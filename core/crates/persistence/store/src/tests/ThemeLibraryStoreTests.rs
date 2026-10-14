#![allow(non_snake_case)]

use super::*;
use std::collections::BTreeMap;
use std::sync::Mutex;
use operit_host_api::{HostError, HostResult, RuntimeStorageEntry};

#[derive(Default)]
struct MemoryStorage {
    files: Mutex<BTreeMap<String, Vec<u8>>>,
}

impl RuntimeStorageHost for MemoryStorage {
    fn runtimeRootDir(&self) -> Option<std::path::PathBuf> { None }
    fn workspaceRootDir(&self) -> Option<std::path::PathBuf> { None }
    fn readBytes(&self, path: &str) -> HostResult<Vec<u8>> {
        self.files.lock().unwrap().get(path).cloned()
            .ok_or_else(|| HostError::new(format!("missing {path}")))
    }
    fn readBytesRange(&self, path: &str, offset: u64, length: usize) -> HostResult<Vec<u8>> {
        let bytes = self.readBytes(path)?;
        let start = (offset as usize).min(bytes.len());
        Ok(bytes[start..start.saturating_add(length).min(bytes.len())].to_vec())
    }
    fn writeBytes(&self, path: &str, bytes: &[u8]) -> HostResult<()> {
        self.files.lock().unwrap().insert(path.into(), bytes.into()); Ok(())
    }
    fn appendBytes(&self, path: &str, bytes: &[u8]) -> HostResult<()> {
        self.files.lock().unwrap().entry(path.into()).or_default().extend_from_slice(bytes); Ok(())
    }
    fn delete(&self, path: &str, recursive: bool) -> HostResult<()> {
        self.files.lock().unwrap().retain(|key, _|
            key != path && !(recursive && key.starts_with(&format!("{path}/")))); Ok(())
    }
    fn exists(&self, path: &str) -> HostResult<bool> {
        Ok(self.files.lock().unwrap().keys().any(|key|
            key == path || key.starts_with(&format!("{path}/"))))
    }
    fn list(&self, path: &str) -> HostResult<Vec<RuntimeStorageEntry>> {
        let mut entries = BTreeMap::new();
        for (key, bytes) in self.files.lock().unwrap().iter() {
            if let Some(relative) = key.strip_prefix(&format!("{path}/")) {
                let name = relative.split('/').next().unwrap();
                let full = format!("{path}/{name}");
                let directory = relative.contains('/');
                entries.insert(full.clone(), RuntimeStorageEntry {
                    path: full, isDirectory: directory, size: if directory { 0 } else { bytes.len() as i64 },
                });
            }
        }
        Ok(entries.into_values().collect())
    }
}

fn setup() -> (Arc<MemoryStorage>, ThemeLibraryStore) {
    let storage = Arc::new(MemoryStorage::default());
    let store = ThemeLibraryStore::new(storage.clone());
    // Minimal private-state fixture; this is not a public package/Snapshot validator.
    store.initializeAppearance(r#"{"themeMode":"system","fontScale":1.0,"useBackgroundImage":false,"bubbleShowAvatar":true}"#).unwrap();
    (storage, store)
}

/// Stores a native JSON selection whose package has not reached this node.
fn selectMissing(storage: &Arc<MemoryStorage>) -> ThemeSelection {
    let selection = ThemeSelection {
        schema_version: 1, theme_id: "com.example.missing:default".into(),
        version: "1.0.0".into(), archive_sha256: "a".repeat(64),
    };
    selectionStore(storage.clone()).edit(|preferences|
        preferences.set(&stringPreferencesKey(THEME_SELECTION_KEY), serde_json::to_value(&selection).unwrap())).unwrap();
    selection
}

#[test]
fn nodeAppearanceDoesNotReadOrMigrateRolePreferences() {
    let (storage, store) = setup();
    let legacy = b"{\"character_card_theme_example_theme_mode\":\"dark\"}";
    storage.writeBytes("runtime/config/preferences/user_preferences.preferences.json", legacy).unwrap();
    store.initializeAppearance(r#"{"themeMode":"light"}"#).unwrap();
    let appearance: serde_json::Value = serde_json::from_str(&store.nodeState().unwrap().private_appearance_json.unwrap()).unwrap();
    assert_eq!(appearance["themeMode"], "system");
    assert_eq!(storage.readBytes("runtime/config/preferences/user_preferences.preferences.json").unwrap(), legacy);
    assert!(storage.files.lock().unwrap().keys().all(|path| !path.contains("migration")));
}

/// Verifies native JSON persistence while the public appearance transport remains JSON text.
#[test]
fn themeStatePersistsNativeJsonObjects() {
    let (storage, store) = setup();
    let selected = selectMissing(&storage);
    store.updateAppearance(BTreeMap::from([("fontScale".into(), "1.2".into())])).unwrap();
    let persisted: serde_json::Value = serde_json::from_slice(
        &storage.readBytes(RUNTIME_THEME_NODE_SELECTION_PATH).unwrap(),
    ).unwrap();
    assert_eq!(persisted[THEME_SELECTION_KEY], serde_json::to_value(&selected).unwrap());
    assert!(persisted[PRIVATE_APPEARANCE_KEY].is_object());
    assert_eq!(persisted[PRIVATE_APPEARANCE_KEY]["fontScale"], 1.2);
    let state = store.nodeState().unwrap();
    assert_eq!(state.selection, Some(selected));
    let appearance: serde_json::Value = serde_json::from_str(&state.private_appearance_json.unwrap()).unwrap();
    assert_eq!(appearance, persisted[PRIVATE_APPEARANCE_KEY]);
}

/// Rejects malformed stored appearance without changing the committed state.
#[test]
fn invalidStoredAppearanceIsRejectedWithoutMutation() {
    let (storage, store) = setup();
    selectionStore(storage.clone()).edit(|preferences| {
        preferences.set(&stringPreferencesKey(PRIVATE_APPEARANCE_KEY), true);
    }).unwrap();
    let before = storage.readBytes(RUNTIME_THEME_NODE_SELECTION_PATH).unwrap();
    assert!(store.nodeState().is_err());
    assert!(store.initializeAppearance(r#"{"themeMode":"light"}"#).is_err());
    assert_eq!(storage.readBytes(RUNTIME_THEME_NODE_SELECTION_PATH).unwrap(), before);
}

#[test]
fn partialEditsAreAtomicAndNodeOwned() {
    let (storage, store) = setup();
    store.updateAppearance(BTreeMap::from([("fontScale".into(), "1.2".into())])).unwrap();
    let before = store.nodeState().unwrap();
    assert!(store.updateAppearance(BTreeMap::from([
        ("themeMode".into(), "\"dark\"".into()), ("unknownField".into(), "true".into()),
    ])).is_err());
    assert_eq!(store.nodeState().unwrap(), before);
    assert!(storage.files.lock().unwrap().keys().all(|path| path == RUNTIME_THEME_NODE_SELECTION_PATH));
}

#[test]
fn selectedMissingPackageIsObservableAndNeverAutoReplaced() {
    let (storage, store) = setup();
    let selection = selectMissing(&storage);
    assert!(store.currentSelection().is_err());
    assert_eq!(store.nodeState().unwrap().selection, Some(selection));
    store.selectPersonalAppearance().unwrap();
    assert!(store.nodeState().unwrap().selection.is_none());
}

#[test]
fn packageVisualsAreImmutableWhileNodeSettingsRemainEditable() {
    let (storage, store) = setup();
    let selected = selectMissing(&storage);
    assert!(store.updateAppearance(BTreeMap::from([("useBackgroundImage".into(), "true".into())])).is_err());
    store.updateAppearance(BTreeMap::from([
        ("themeMode".into(), "\"dark\"".into()),
        ("bubbleShowAvatar".into(), "false".into()),
    ])).unwrap();
    let state = store.nodeState().unwrap();
    assert_eq!(state.selection, Some(selected));
    let appearance: serde_json::Value = serde_json::from_str(&state.private_appearance_json.unwrap()).unwrap();
    assert_eq!(appearance["useBackgroundImage"], false);
    assert_eq!(appearance["themeMode"], "dark");
    assert_eq!(appearance["bubbleShowAvatar"], false);
}

/// Keeps an explicitly selected missing ToolPkg visible in node state without fabricating library entries.
#[test]
fn missingToolPkgDoesNotCreateAnIndependentThemeArchiveStore() {
    let (storage, store) = setup();
    let selected = selectMissing(&storage);
    assert!(store.listLibrary().unwrap().is_empty());
    assert!(store.theme(&selected.theme_id, &selected.archive_sha256).is_err());
    assert_eq!(store.nodeState().unwrap().selection, Some(selected));
    assert!(storage.files.lock().unwrap().keys().all(|path| !path.starts_with("runtime/themes/")));
}

/// Rejects removal of the selected ToolPkg before any shared catalog mutation.
#[test]
fn selectedToolPkgCannotBeRemovedThroughTheThemePage() {
    let (storage, store) = setup();
    let selected = selectMissing(&storage);
    assert!(store.ensureThemeRemovable(&selected.theme_id, &selected.archive_sha256).is_err());
    assert_eq!(store.nodeState().unwrap().selection, Some(selected));
}

/// Builds a real declarative theme ToolPkg fixture without platform or application-version metadata.
fn toolpkgWithTheme(id: &str) -> Vec<u8> {
    let mut png = std::io::Cursor::new(Vec::new());
    image::RgbaImage::from_pixel(1280, 800, image::Rgba([40, 60, 80, 255]))
        .write_to(&mut png, image::ImageFormat::Png).unwrap();
    operit_plugin_theme::export_toolpkg_from_theme(&operit_plugin_theme::ThemeExportDraft {
        toolpkg_id: id.into(), version: "custom-version".into(), display_name: "Example".into(),
        author_name: "Author".into(), theme_json: b"{}".to_vec(), preview_png: png.into_inner(),
        agreement_text: "Custom usage agreement".into(), description: None, assets: Vec::new(),
    }).unwrap()
}

/// Stores one archive in the ordinary package extension catalog for the given owner scope.
fn registerToolPkg(storage: &Arc<MemoryStorage>, id: &str, bytes: &[u8]) {
    let store = ExtensionStore::new(storage.clone());
    storage.writeBytes(&format!("{}/{id}.toolpkg", ExtensionStore::root("package", "device").unwrap()), bytes).unwrap();
    store.registerDevice("package", id, &format!("{id}.toolpkg"), serde_json::json!({
        "builtin": false, "members": [id], "enabledNames": [], "disabledNames": [],
        "subpackageStates": {}, "order": 0, "installationId": "a".repeat(32),
    })).unwrap();
}

/// Resolves themes from installed ToolPkgs and follows package scope moves and package removal.
#[test]
fn themes_follow_shared_toolpkg_installation_scope_and_uninstall() {
    let (storage, store) = setup();
    let id = "dev.operit.installed";
    let bytes = toolpkgWithTheme(id);
    let digest = sha256_hex(&bytes);
    registerToolPkg(&storage, id, &bytes);
    let before = store.theme(&format!("{id}:default"), &digest).unwrap();
    assert_eq!(store.packageBytes(&before).unwrap(), bytes);
    assert!(before.package_path.starts_with("runtime/extensions/device/packages/"));
    let extensions = ExtensionStore::new(storage.clone());
    extensions.moveScope("package", id, "space").unwrap();
    let moved = store.theme(&format!("{id}:default"), &digest).unwrap();
    assert!(moved.package_path.starts_with("runtime/extensions/space/packages/"));
    assert_eq!(store.packageBytes(&moved).unwrap(), bytes);
    store.applyValidated(&operit_plugin_theme::validate_toolpkg_theme_templates(&bytes).map(|mut templates| templates.remove(0)).unwrap()).unwrap();
    assert!(store.ensureThemeRemovable(&format!("{id}:default"), &digest).is_err());
    store.selectPersonalAppearance().unwrap();
    store.ensureThemeRemovable(&format!("{id}:default"), &digest).unwrap();
    extensions.delete("package", id).unwrap();
    assert!(store.listLibrary().unwrap().is_empty());
    assert!(storage.files.lock().unwrap().keys().all(|path| !path.starts_with("runtime/themes/")));
}
