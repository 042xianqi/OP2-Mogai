#![allow(non_snake_case)]

use std::sync::Arc;

use operit_host_api::RuntimeStorageHost;
use operit_plugin_theme::{
    sha256_hex, ThemeTemplateTrust, ThemeValidationReceipt, ValidatedThemeTemplate,
};
use operit_util::RuntimeStorageLayout::{
    RUNTIME_THEME_NODE_SELECTION_PATH,
};
use serde::{Deserialize, Serialize};

use crate::PreferencesDataStore::{
    stringPreferencesKey, CoreNodeStateStore, Flow, Preferences, PreferencesDataStoreError,
};
use crate::ExtensionStore::{ExtensionStore, ExtensionRecord};
use crate::RuntimeStorageHost::defaultRuntimeStorageHost;

const THEME_LIBRARY_SCHEMA_VERSION: u32 = 1;
const THEME_SELECTION_KEY: &str = "selection";
const MAX_THEME_LIBRARY_ENTRIES: usize = 512;
const PRIVATE_APPEARANCE_KEY: &str = "privateAppearanceJson";
const MAX_PRIVATE_APPEARANCE_BYTES: usize = 256 * 1024;

/// Raw node state stays readable even when the selected Space package is missing.
/// Availability is checked separately; an observer must never replace a broken selection.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeNodeState {
    pub schema_version: u32,
    pub selection: Option<ThemeSelection>,
    pub private_appearance_json: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeLibraryAvailability {
    Ready,
    WaitingForSync,
    Invalid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeLibraryItem {
    pub theme_id: String,
    pub archive_sha256: String,
    pub entry: Option<ThemeLibraryEntry>,
    pub availability: ThemeLibraryAvailability,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeLibraryEntry {
    pub schema_version: u32,
    pub theme_id: String,
    pub package_id: String,
    pub template_id: String,
    pub version: String,
    pub display_name: String,
    pub author_name: String,
    pub license: Option<String>,
    pub archive_sha256: String,
    pub archive_bytes: usize,
    pub expanded_bytes: usize,
    pub manifest_sha256: String,
    pub theme_sha256: String,
    pub package_path: String,
    pub trust: ThemeTemplateTrust,
    pub validation: ThemeValidationReceipt,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeSelection {
    pub schema_version: u32,
    pub theme_id: String,
    pub version: String,
    pub archive_sha256: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemeInstallStatus {
    Imported,
    AlreadyPresent,
    Applied,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeInstallReceipt {
    pub schema_version: u32,
    pub status: ThemeInstallStatus,
    pub entry: ThemeLibraryEntry,
    pub selection: Option<ThemeSelection>,
    pub selection_changed: bool,
    pub validation: ThemeValidationReceipt,
}

#[derive(Clone)]
pub struct ThemeLibraryStore {
    storage: Arc<dyn RuntimeStorageHost>,
}

impl ThemeLibraryStore {
    /// Opens installed ToolPkg themes through the shared extension storage.
    pub fn new(storage: Arc<dyn RuntimeStorageHost>) -> Self {
        Self {
            storage,
        }
    }

    /// Opens the registered storage host for installed ToolPkg themes.
    pub fn native() -> Self {
        Self::new(defaultRuntimeStorageHost())
    }

    /// Lists available template contributions from the shared ToolPkg catalog.
    #[allow(non_snake_case)]
    pub fn listThemes(&self) -> Result<Vec<ThemeLibraryEntry>, String> {
        let mut entries = self.listLibrary()?.into_iter().map(|item| {
            if item.availability != ThemeLibraryAvailability::Ready {
                return Err(format!("theme library item is not ready: {} {}", item.theme_id, item.archive_sha256));
            }
            item.entry.ok_or_else(|| "ready theme has no metadata".to_string())
        }).collect::<Result<Vec<_>, String>>()?;
        entries.sort_by(|left, right| {
            (
                left.theme_id.as_str(),
                left.version.as_str(),
                left.archive_sha256.as_str(),
            )
                .cmp(&(
                    right.theme_id.as_str(),
                    right.version.as_str(),
                    right.archive_sha256.as_str(),
                ))
        });
        Ok(entries)
    }

    /// Resolves one exact theme identity from the installed ToolPkg catalog.
    #[allow(non_snake_case)]
    pub fn theme(&self, themeId: &str, archiveSha256: &str) -> Result<ThemeLibraryEntry, String> {
        validateThemeIdentity(themeId, archiveSha256)?;
        self.listThemes()?.into_iter()
            .find(|entry| entry.theme_id == themeId && entry.archive_sha256 == archiveSha256)
            .ok_or_else(|| format!("Theme template is not installed: {themeId} {archiveSha256}"))
    }

    #[allow(non_snake_case)]
    /// Reads ordinary ToolPkg bytes for a selected template without duplicating its storage.
    pub fn packageBytes(&self, entry: &ThemeLibraryEntry) -> Result<Vec<u8>, String> {
        let stored = self.theme(&entry.theme_id, &entry.archive_sha256)?;
        if stored != *entry {
            return Err("theme library entry changed during package read".to_string());
        }
        self.readPackageBytes(&stored)
    }

    /// Applies only a library entry and writes only the local CoreNode choice.
    /// The package bytes remain Space-owned and are never copied into selection.
    #[allow(non_snake_case)]
    pub fn applyValidated(
        &self,
        package: &ValidatedThemeTemplate,
    ) -> Result<ThemeInstallReceipt, String> {
        let entry = self.theme(&package.metadata().theme_id, package.archive_sha256())?;
        let expected = entryFromPackage(package, &entry.package_path)?;
        if entry != expected {
            return Err("theme library changed after full package validation".to_string());
        }
        let next = ThemeSelection {
            schema_version: THEME_LIBRARY_SCHEMA_VERSION,
            theme_id: entry.theme_id.clone(),
            version: entry.version.clone(),
            archive_sha256: entry.archive_sha256.clone(),
        };
        let selection_store = selectionStore(self.storage.clone());
        let value = serde_json::to_value(&next).map_err(|error| error.to_string())?;
        let changed = selection_store.try_edit_result(|preferences| {
            let previous = selectionFromPreferences(preferences)
                .map_err(PreferencesDataStoreError::Message)?;
            let changed = previous.as_ref() != Some(&next);
            if changed { preferences.set(&stringPreferencesKey(THEME_SELECTION_KEY), value); }
            Ok::<bool, PreferencesDataStoreError>(changed)
        }).map_err(|error| error.to_string())?;
        Ok(ThemeInstallReceipt {
            schema_version: THEME_LIBRARY_SCHEMA_VERSION,
            status: ThemeInstallStatus::Applied,
            validation: package.receipt().clone(),
            selection: Some(next),
            selection_changed: changed,
            entry,
        })
    }

    #[allow(non_snake_case)]
    pub fn currentSelection(&self) -> Result<Option<ThemeSelection>, String> {
        let selection = readSelection(&selectionStore(self.storage.clone()))?;
        let Some(selection) = selection else {
            return Ok(None);
        };
        let entry = self.theme(&selection.theme_id, &selection.archive_sha256)?;
        if entry.version != selection.version {
            return Err("theme selection version differs from its Space library entry".to_string());
        }
        let _ = self.readPackageBytes(&entry)?;
        Ok(Some(selection))
    }

    pub fn nodeState(&self) -> Result<ThemeNodeState, String> {
        nodeStateFromPreferences(
            &selectionStore(self.storage.clone()).data().map_err(|error| error.to_string())?,
        )
    }

    /// All engines observe the same node-local store; no UI polling or prompt subscription.
    pub fn nodeStateFlow(&self) -> Flow<ThemeNodeState> {
        selectionStore(self.storage.clone()).dataFlow().mapResult(|preferences| {
            nodeStateFromPreferences(&preferences).map_err(PreferencesDataStoreError::Message)
        })
    }

    /// Fresh internal-test state only. Old character/group keys are not read or migrated.
    pub fn initializeAppearance(&self, snapshotJson: &str) -> Result<ThemeNodeState, String> {
        checkPrivateSnapshot(snapshotJson)?;
        let defaults: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(snapshotJson).map_err(|error| error.to_string())?;
        let store = selectionStore(self.storage.clone());
        store.try_edit_result(|preferences| {
            let key = stringPreferencesKey(PRIVATE_APPEARANCE_KEY);
            match preferences.getTyped::<serde_json::Map<String, serde_json::Value>>(&key)? {
                None => {
                    preferences.set(&key, defaults.clone());
                }
                Some(mut current) => {
                    let mut changed = false;
                    for (field, default_value) in &defaults {
                        if !current.contains_key(field) {
                            current.insert(field.clone(), default_value.clone());
                            changed = true;
                        }
                    }
                    if changed {
                        let serialized = serde_json::to_string(&current)?;
                        checkPrivateSnapshot(&serialized)
                            .map_err(PreferencesDataStoreError::Message)?;
                        preferences.set(&key, current);
                    }
                }
            }
            Ok::<(), PreferencesDataStoreError>(())
        }).map_err(|error| error.to_string())?;
        self.nodeState()
    }

    /// Atomic partial edits avoid replacing changes made by another Flutter engine.
    /// An immutable package allows only node/user settings, not visual edits.
    pub fn updateAppearance(
        &self,
        updates: std::collections::BTreeMap<String, String>,
    ) -> Result<ThemeNodeState, String> {
        let store = selectionStore(self.storage.clone());
        store.try_edit_result(|preferences| {
            let state = nodeStateFromPreferences(preferences)
                .map_err(PreferencesDataStoreError::Message)?;
            let source = state.private_appearance_json.ok_or_else(||
                PreferencesDataStoreError::Message("node appearance has not been initialized".to_string()))?;
            let mut snapshot: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&source)?;
            for (key, value) in &updates {
                if !snapshot.contains_key(key) {
                    return Err(PreferencesDataStoreError::Message(format!("unknown appearance field: {key}")));
                }
                if state.selection.is_some() && !isNodeThemeSetting(key) {
                    return Err(PreferencesDataStoreError::Message(
                        "package visuals are immutable; select personal appearance to edit them".to_string()));
                }
                snapshot.insert(key.clone(), serde_json::from_str(value)?);
            }
            let value = serde_json::to_string(&snapshot)?;
            checkPrivateSnapshot(&value).map_err(PreferencesDataStoreError::Message)?;
            preferences.set(&stringPreferencesKey(PRIVATE_APPEARANCE_KEY), snapshot);
            Ok(())
        }).map_err(|error| error.to_string())?;
        self.nodeState()
    }

    /// Explicit user action, never invoked in response to a missing package.
    pub fn selectPersonalAppearance(&self) -> Result<ThemeNodeState, String> {
        let store = selectionStore(self.storage.clone());
        store.try_edit_result(|preferences| {
            if preferences.get(&stringPreferencesKey(PRIVATE_APPEARANCE_KEY)).is_none() {
                return Err(PreferencesDataStoreError::Message("node appearance has not been initialized".to_string()));
            }
            preferences.remove(&stringPreferencesKey(THEME_SELECTION_KEY));
            Ok(())
        }).map_err(|error| error.to_string())?;
        self.nodeState()
    }

    /// Rejects removing a ToolPkg currently selected on this CoreNode.
    #[allow(non_snake_case)]
    pub fn ensureThemeRemovable(&self, themeId: &str, archiveSha256: &str) -> Result<(), String> {
        validateThemeIdentity(themeId, archiveSha256)?;
        if readSelection(&selectionStore(self.storage.clone()))?.as_ref()
            .is_some_and(|selection| selection.theme_id.split_once(':').map(|(owner, _)| owner) == themeId.split_once(':').map(|(owner, _)| owner)) {
            return Err("Switch to personal appearance before removing the ToolPkg contributing the active template".to_string());
        }
        self.theme(themeId, archiveSha256)?;
        Ok(())
    }

    /// Lists themes from the shared extension catalog without keeping a second archive store.
    #[allow(non_snake_case)]
    pub fn listLibrary(&self) -> Result<Vec<ThemeLibraryItem>, String> {
        let records = ExtensionStore::new(self.storage.clone()).records("package")?;
        let mut items = Vec::new();
        for record in records {
            if !record.sourceName.ends_with(".toolpkg") { continue; }
            let path = installedPackagePath(&record)?;
            if !self.storage.exists(&path).map_err(|error| error.to_string())? { continue; }
            let bytes = self.storage.readBytes(&path)
                .map_err(|error| error.to_string())?;
            let templates = operit_plugin_sdk::toolpkg::ToolPkgParser::ToolPkgArchiveParser::readThemeTemplates(&bytes)?;
            for template in templates {
                if template.metadata().package_id != record.id {
                    return Err("Template owner differs from installed ToolPkg identity".to_string());
                }
                let entry = entryFromPackage(&template, &path)?;
                items.push(ThemeLibraryItem {
                    theme_id: entry.theme_id.clone(), archive_sha256: entry.archive_sha256.clone(),
                    entry: Some(entry), availability: ThemeLibraryAvailability::Ready, message: None,
                });
                if items.len() > MAX_THEME_LIBRARY_ENTRIES {
                    return Err("Too many installed theme templates".to_string());
                }
            }
        }
        items.sort_by(|left, right| left.theme_id.cmp(&right.theme_id));
        Ok(items)
    }

    /// Reads the ordinary installed ToolPkg and verifies its confirmed archive identity.
    fn readPackageBytes(&self, entry: &ThemeLibraryEntry) -> Result<Vec<u8>, String> {
        let bytes = self
            .storage
            .readBytesRange(&entry.package_path, 0, entry.archive_bytes + 1)
            .map_err(|error| error.to_string())?;
        if bytes.len() != entry.archive_bytes || sha256_hex(&bytes) != entry.archive_sha256 {
            return Err("ToolPkg bytes differ from the library entry".to_string());
        }
        Ok(bytes)
    }


}

/// Projects one validated template into a shared package capability listing.
fn entryFromPackage(package: &ValidatedThemeTemplate, packagePath: &str) -> Result<ThemeLibraryEntry, String> {
    let manifest = package.metadata();
    validateThemeIdentity(&manifest.theme_id, package.archive_sha256())?;
    Ok(ThemeLibraryEntry {
        schema_version: THEME_LIBRARY_SCHEMA_VERSION,
        theme_id: manifest.theme_id.clone(),
        package_id: manifest.package_id.clone(), template_id: manifest.template_id.clone(),
        version: manifest.version.clone(),
        display_name: manifest.display_name.clone(),
        author_name: manifest.author.name.clone(),
        license: manifest.license.clone(),
        archive_sha256: package.archive_sha256().to_string(),
        archive_bytes: package.archive_bytes(),
        expanded_bytes: package.expanded_bytes(),
        manifest_sha256: package.receipt().manifest_sha256.clone(),
        theme_sha256: package.receipt().theme_sha256.clone(),
        package_path: packagePath.to_string(),
        trust: package.receipt().trust,
        validation: package.receipt().clone(),
    })
}

/// Resolves the archive through the owning extension scope.
fn installedPackagePath(record: &ExtensionRecord) -> Result<String, String> {
    Ok(format!("{}/{}", ExtensionStore::root("package", &record.scope)?, record.sourceName))
}

fn validateThemeIdentity(themeId: &str, archiveSha256: &str) -> Result<(), String> {
    let (package_id, template_id) = themeId.split_once(':').ok_or("Theme identity must include its ToolPkg owner and template id")?;
    if package_id.is_empty() || template_id.is_empty() || themeId.len() > 256 ||
        !package_id.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_')) ||
        template_id.contains(':') {
        return Err("Theme template identity is invalid".to_string());
    }
    if archiveSha256.len() != 64
        || !archiveSha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("theme archive SHA-256 is invalid".to_string());
    }
    Ok(())
}

fn selectionStore(storage: Arc<dyn RuntimeStorageHost>) -> CoreNodeStateStore {
    CoreNodeStateStore::newWithStorage(storage, RUNTIME_THEME_NODE_SELECTION_PATH)
}

fn readSelection(store: &CoreNodeStateStore) -> Result<Option<ThemeSelection>, String> {
    let preferences = store.data().map_err(|error| error.to_string())?;
    selectionFromPreferences(&preferences)
}

/// Decodes the selection from its native JSON object and validates its identity.
fn selectionFromPreferences(preferences: &Preferences) -> Result<Option<ThemeSelection>, String> {
    let Some(selection) = preferences
        .getTyped::<ThemeSelection>(&stringPreferencesKey(THEME_SELECTION_KEY))
        .map_err(|error| error.to_string())? else {
        return Ok(None);
    };
    if selection.schema_version != THEME_LIBRARY_SCHEMA_VERSION {
        return Err("unsupported theme selection schema".to_string());
    }
    validateThemeIdentity(&selection.theme_id, &selection.archive_sha256)?;
    Ok(Some(selection))
}

/// Serializes the native appearance object for the public theme state transport.
fn nodeStateFromPreferences(preferences: &Preferences) -> Result<ThemeNodeState, String> {
    let appearance = preferences
        .getTyped::<serde_json::Map<String, serde_json::Value>>(
            &stringPreferencesKey(PRIVATE_APPEARANCE_KEY),
        )
        .map_err(|error| error.to_string())?
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| error.to_string())?;
    if let Some(value) = &appearance { checkPrivateSnapshot(value)?; }
    Ok(ThemeNodeState {
        schema_version: THEME_LIBRARY_SCHEMA_VERSION,
        selection: selectionFromPreferences(preferences)?,
        private_appearance_json: appearance,
    })
}

fn checkPrivateSnapshot(value: &str) -> Result<(), String> {
    if value.len() > MAX_PRIVATE_APPEARANCE_BYTES {
        return Err("node appearance byte budget exceeded".to_string());
    }
    let _: serde_json::Map<String, serde_json::Value> =
        serde_json::from_str(value).map_err(|error| error.to_string())?;
    Ok(())
}

fn isNodeThemeSetting(key: &str) -> bool {
    matches!(
        key,
        "themeMode"
            | "fontScale"
            | "customUserAvatarUri"
            | "customAiAvatarUri"
            | "bubbleShowAvatar"
            | "avatarShape"
            | "avatarCornerRadius"
            | "chatStyle"
            | "bubbleWideLayoutEnabled"
            | "inputStyle"
            | "chatInputFloating"
            | "inputBackgroundColor"
            | "inputRoundedCornersEnabled"
            | "inputUseImage"
            | "inputImageUri"
            | "inputImageRenderMode"
            | "inputImageCropLeft"
            | "inputImageCropTop"
            | "inputImageCropRight"
            | "inputImageCropBottom"
            | "inputImageRepeatStart"
            | "inputImageRepeatEnd"
            | "inputImageRepeatYStart"
            | "inputImageRepeatYEnd"
            | "inputImageScale"
            | "inputSettingsAsset"
            | "inputAttachmentAsset"
            | "inputMicAsset"
            | "inputSendAsset"
            | "inputStopAsset"
            | "inputSettingsAssetSize"
            | "inputAttachmentAssetSize"
            | "inputMicAssetSize"
            | "inputSendAssetSize"
            | "inputStopAssetSize"
            | "transparentSurfaceEnabled"
            | "backgroundImageOpacity"
            | "useBackgroundBlur"
            | "backgroundBlurRadius"
            | "videoBackgroundMuted"
            | "videoBackgroundLoop"
            | "bubbleUserRoundedCornersEnabled"
            | "bubbleAiRoundedCornersEnabled"
            | "bubbleUserContentPaddingLeft"
            | "bubbleUserContentPaddingRight"
            | "bubbleAiContentPaddingLeft"
            | "bubbleAiContentPaddingRight"
            | "cursorUserBubbleColor"
            | "bubbleUserBubbleColor"
            | "bubbleAiBubbleColor"
            | "bubbleUserTextColor"
            | "bubbleAiTextColor"
            | "bubbleUserImageRenderMode"
            | "bubbleUserImageCropLeft"
            | "bubbleUserImageCropTop"
            | "bubbleUserImageCropRight"
            | "bubbleUserImageCropBottom"
            | "bubbleUserImageRepeatStart"
            | "bubbleUserImageRepeatEnd"
            | "bubbleUserImageRepeatYStart"
            | "bubbleUserImageRepeatYEnd"
            | "bubbleUserImageScale"
            | "bubbleAiImageRenderMode"
            | "bubbleAiImageCropLeft"
            | "bubbleAiImageCropTop"
            | "bubbleAiImageCropRight"
            | "bubbleAiImageCropBottom"
            | "bubbleAiImageRepeatStart"
            | "bubbleAiImageRepeatEnd"
            | "bubbleAiImageRepeatYStart"
            | "bubbleAiImageRepeatYEnd"
            | "bubbleAiImageScale"
            | "showThinkingProcess"
            | "showModelProvider"
            | "showModelName"
            | "showRoleName"
            | "showUserName"
            | "showMessageTokenStats"
            | "showMessageTimingStats"
            | "showMessageTimestamp"
            | "showInputProcessingStatus"
    )
}

#[cfg(test)]
#[path = "tests/ThemeLibraryStoreTests.rs"]
mod tests;
