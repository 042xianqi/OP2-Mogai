//! Composes preference merge declarations and prepares source-owned migrations.
use std::sync::Arc;

use operit_host_api::RuntimeStorageHost;
use operit_store::PreferenceMerge::PreferenceMergeSchema;
use operit_util::RuntimeStorageLayout;

use super::FunctionalConfigManager::FunctionalConfigManager;
use super::ModelConfigManager::ModelConfigManager;
use super::SttConfigManager::SttConfigManager;
use super::ThemeConfigManager::ThemeConfigManager;
use super::TtsConfigManager::TtsConfigManager;
use super::UserPreferencesManager::UserPreferencesManager;

/// Collects merge declarations without opening preference stores.
pub(crate) fn schema() -> PreferenceMergeSchema {
    PreferenceMergeSchema {
        stores: vec![
            ModelConfigManager::spaceMergeStore(),
            TtsConfigManager::spaceMergeStore(),
            SttConfigManager::spaceMergeStore(),
            ThemeConfigManager::spaceMergeStore(),
            FunctionalConfigManager::spaceMergeStore(),
        ],
    }
}

/// Runs each owner's migrations and reports failures by storage path.
pub(crate) fn prepare(storage: Arc<dyn RuntimeStorageHost>) -> Vec<String> {
    let stores = [
        ModelConfigManager::preferenceStore(
            storage.clone(),
            RuntimeStorageLayout::MODEL_CONFIGS_PREFERENCES_PATH,
        ),
        TtsConfigManager::preferenceStore(
            storage.clone(),
            RuntimeStorageLayout::TTS_CONFIGS_PREFERENCES_PATH,
        ),
        SttConfigManager::preferenceStore(
            storage.clone(),
            RuntimeStorageLayout::STT_CONFIGS_PREFERENCES_PATH,
        ),
        UserPreferencesManager::store(storage.clone()),
        FunctionalConfigManager::preferenceStore(
            storage,
            RuntimeStorageLayout::FUNCTIONAL_CONFIGS_PREFERENCES_PATH,
        ),
    ];

    let mut errors = Vec::new();
    for store in stores {
        if let Err(error) = store.data() {
            errors.push(format!(
                "preference migration path={}: {error}",
                store.path().display(),
            ));
        }
    }
    errors
}
