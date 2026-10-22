use operit_host_api::RuntimeStorageHost;
use operit_host_native_storage::NativeRuntimeStorageHost;
use operit_store::{
    PreferenceMerge::{applyOperations, PreferenceMergeSchema, PreferenceMergeStore},
    PreferencesDataStore::{
        stringPreferencesKey, PreferencesDataStore, PreferencesDataStoreError,
        PreferencesSyncedEntry,
    },
    SyncOperationStore::SyncOperationStore,
};
use serde_json::json;
use std::{path::PathBuf, sync::Arc};
struct Fixture {
    root: PathBuf,
    host: Arc<NativeRuntimeStorageHost>,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "operit-preferences-upgrade-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let host = Arc::new(NativeRuntimeStorageHost::new(
            root.join("runtime"),
            root.join("workspaces"),
        ));
        Self { root, host }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
#[test]
fn replay_keeps_existing_unjournaled_baseline() {
    let a = Fixture::new();
    let b = Fixture::new();
    let path = "runtime/config/preferences/review-counter.preferences.json";
    a.host
        .writeBytesAtomically(path, br#"{"counter":"10"}"#)
        .unwrap();
    let local = PreferencesDataStore::newWithStorage(a.host.clone(), path).withSchema(1, |_, p| {
        let key = stringPreferencesKey("counter");
        if let Some(encoded) = p
            .get(&key)
            .and_then(|value| value.as_str())
            .map(str::to_owned)
        {
            p.set(&key, serde_json::from_str::<i64>(&encoded)?);
        }
        Ok(())
    });
    let key = stringPreferencesKey("counter");
    local
        .try_edit_result(|p| p.incrementInteger(&key, 2))
        .unwrap();
    assert_eq!(local.data().unwrap().get(&key), Some(&json!(12)));
    let remote = PreferencesDataStore::newWithStorage(b.host.clone(), path);
    remote
        .try_edit_result(|p| p.incrementInteger(&key, 3))
        .unwrap();
    let incoming = SyncOperationStore::new(b.host.clone(), "runtime/sync")
        .preferenceHistory()
        .unwrap();
    applyOperations(
        a.host.clone(),
        &PreferenceMergeSchema::default(),
        &[],
        &incoming,
        |_, _| true,
    )
    .unwrap();
    assert_eq!(local.data().unwrap().get(&key), Some(&json!(15)));
}
fn decode_old(entry: &mut PreferencesSyncedEntry) -> Result<(), PreferencesDataStoreError> {
    entry.decodeHistoricalValues(|version, p| {
        if version == 0 {
            let existing = p.entries();
            for (key, value) in existing {
                if let Some(encoded) = value.as_str() {
                    p.set(
                        &stringPreferencesKey(&key),
                        serde_json::from_str::<serde_json::Value>(encoded)?,
                    );
                }
            }
        }
        Ok(())
    })
}
#[test]
fn syncing_new_record_migrates_existing_local_records_before_promoting_schema() {
    let a = Fixture::new();
    let b = Fixture::new();
    let path = "runtime/config/preferences/review-representation.preferences.json";
    a.host
        .writeBytesAtomically(path, br#"{"old_record":"{\"enabled\":true}"}"#)
        .unwrap();
    let remote = PreferencesDataStore::newWithStorage(b.host.clone(), path);
    remote
        .edit(|p| {
            p.set(
                &stringPreferencesKey("new_record"),
                json!({"enabled":false}),
            )
        })
        .unwrap();
    let incoming = SyncOperationStore::new(b.host.clone(), "runtime/sync")
        .preferenceHistory()
        .unwrap();
    let schema = PreferenceMergeSchema {
        stores: vec![PreferenceMergeStore::values(path, false, 1, decode_old)],
    };
    applyOperations(a.host.clone(), &schema, &[], &incoming, |_, _| true).unwrap();
    let local = PreferencesDataStore::newWithStorage(a.host.clone(), path).withSchema(1, |_, p| {
        if let Some(encoded) = p
            .get(&stringPreferencesKey("old_record"))
            .and_then(|v| v.as_str())
            .map(str::to_owned)
        {
            p.set(
                &stringPreferencesKey("old_record"),
                serde_json::from_str::<serde_json::Value>(&encoded)?,
            );
        }
        Ok(())
    });
    assert_eq!(local.schemaVersion().unwrap(), 1);
    assert_eq!(
        local
            .data()
            .unwrap()
            .get(&stringPreferencesKey("old_record")),
        Some(&json!({"enabled":true}))
    );
}

const COUNTER: &str = "runtime/config/preferences/upgrade-counter.preferences.json";

fn counter_schema() -> PreferenceMergeSchema {
    PreferenceMergeSchema {
        stores: vec![PreferenceMergeStore::values(COUNTER, false, 1, decode_old)],
    }
}

fn history(fixture: &Fixture) -> Vec<operit_store::SyncOperationStore::SyncOperation> {
    SyncOperationStore::new(fixture.host.clone(), "runtime/sync")
        .preferenceHistory()
        .unwrap()
}

fn counter(fixture: &Fixture) -> i64 {
    PreferencesDataStore::newWithStorage(fixture.host.clone(), COUNTER)
        .data()
        .unwrap()
        .get(&stringPreferencesKey("counter"))
        .unwrap()
        .as_i64()
        .unwrap()
}

fn apply(fixture: &Fixture, operations: &[operit_store::SyncOperationStore::SyncOperation]) {
    applyOperations(
        fixture.host.clone(),
        &counter_schema(),
        &[],
        operations,
        |_, _| true,
    )
    .unwrap();
}

#[test]
fn upgraded_devices_and_empty_receiver_converge_with_reversed_and_duplicate_delivery() {
    let a = Fixture::new();
    let b = Fixture::new();
    let c = Fixture::new();
    for (fixture, delta) in [(&a, 2), (&b, 3)] {
        fixture
            .host
            .writeBytesAtomically(COUNTER, br#"{"counter":"10"}"#)
            .unwrap();
        // Preparation imports untouched old data using the same owner path as sync.
        apply(fixture, &[]);
        PreferencesDataStore::newWithStorage(fixture.host.clone(), COUNTER)
            .try_edit_result(|p| p.incrementInteger(&stringPreferencesKey("counter"), delta))
            .unwrap();
    }
    let all = history(&a)
        .into_iter()
        .chain(history(&b))
        .collect::<Vec<_>>();
    for fixture in [&a, &b, &c] {
        for operation in all.iter().rev().chain(all.iter()) {
            apply(fixture, std::slice::from_ref(operation));
        }
        assert_eq!(counter(fixture), 15);
        let originals = history(fixture);
        apply(fixture, &all);
        assert_eq!(history(fixture), originals);
    }
}

/// Local storage versions can advance independently of the stable typed sync contract.
#[test]
fn same_version_upgrade_keeps_pending_operations_and_does_not_publish_schema_changes() {
    let a = Fixture::new();
    let b = Fixture::new();
    let c = Fixture::new();
    for (fixture, before, after) in [(&a, 2, 4), (&b, 3, 5)] {
        fixture.host.writeBytesAtomically(COUNTER, br#"{"counter":"10"}"#).unwrap();
        apply(fixture, &[]);
        let key = stringPreferencesKey("counter");
        PreferencesDataStore::newWithStorage(fixture.host.clone(), COUNTER)
            .withSchema(1, |_, _| Ok(()))
            .try_edit_result(|values| values.incrementInteger(&key, before)).unwrap();
        let pending = history(fixture);
        let upgraded = PreferencesDataStore::newWithStorage(fixture.host.clone(), COUNTER)
            .withSchema(42, |_, _| Ok(()));
        assert_eq!(upgraded.schemaVersion().unwrap(), 42);
        assert_eq!(history(fixture), pending, "storage migration must not replace business history");
        upgraded.try_edit_result(|values| values.incrementInteger(&key, after)).unwrap();
        assert!(history(fixture).iter().all(|operation| operation.payload["contractVersion"] == 1
            && operation.payload.get("schemaVersion").is_none()),
            "business operations must not depend on the local storage version");
    }
    let operations = history(&a).into_iter().chain(history(&b)).collect::<Vec<_>>();
    let schema = PreferenceMergeSchema {
        stores: vec![PreferenceMergeStore::values(COUNTER, false, 42, decode_old)],
    };
    for fixture in [&a, &b, &c] {
        for operation in operations.iter().rev().chain(operations.iter()) {
            applyOperations(fixture.host.clone(), &schema, &[], std::slice::from_ref(operation), |_, _| true).unwrap();
        }
        assert_eq!(counter(fixture), 24);
        let original = history(fixture);
        applyOperations(fixture.host.clone(), &schema, &[], &operations, |_, _| true).unwrap();
        assert_eq!(history(fixture), original);
        assert!(operations.iter().all(|operation| original.contains(operation)));
    }
}

#[test]
fn incoming_delta_migrates_the_old_local_value_before_importing_it() {
    let a = Fixture::new();
    let b = Fixture::new();
    a.host
        .writeBytesAtomically(COUNTER, br#"{"counter":"10"}"#)
        .unwrap();
    PreferencesDataStore::newWithStorage(b.host.clone(), COUNTER)
        .try_edit_result(|p| p.incrementInteger(&stringPreferencesKey("counter"), 3))
        .unwrap();
    apply(&a, &history(&b));
    assert_eq!(counter(&a), 13);
}

#[test]
fn existing_delta_history_derives_initial_value_without_counting_deltas_twice() {
    let a = Fixture::new();
    let b = Fixture::new();
    PreferencesDataStore::newWithStorage(b.host.clone(), COUNTER)
        .try_edit_result(|p| p.incrementInteger(&stringPreferencesKey("counter"), 2))
        .unwrap();
    let operations = history(&b);
    // Simulates a pre-upgrade dataset: committed value includes a recorded delta, but no initial set.
    a.host
        .writeBytesAtomically(COUNTER, br#"{"counter":12}"#)
        .unwrap();
    let journal = SyncOperationStore::new(a.host.clone(), "runtime/sync");
    journal.appendOperations(&operations).unwrap();
    apply(&a, &[]);
    assert_eq!(counter(&a), 12);
    let imported = history(&a);
    apply(&b, &imported);
    assert_eq!(counter(&b), 12);
    apply(&a, &imported);
    assert_eq!(history(&a), imported);
}

#[test]
fn failed_migration_preserves_data_schema_and_history_and_can_retry() {
    fn reject(_: &mut PreferencesSyncedEntry) -> Result<(), PreferencesDataStoreError> {
        Err(PreferencesDataStoreError::Message(
            "injected migration failure".into(),
        ))
    }
    let a = Fixture::new();
    let before = br#"{"counter":"10"}"#;
    a.host.writeBytesAtomically(COUNTER, before).unwrap();
    let invalid = PreferenceMergeSchema {
        stores: vec![PreferenceMergeStore::values(COUNTER, false, 1, reject)],
    };
    assert!(applyOperations(a.host.clone(), &invalid, &[], &[], |_, _| true).is_err());
    assert_eq!(a.host.readBytes(COUNTER).unwrap(), before);
    assert!(history(&a).is_empty());
    apply(&a, &[]);
    assert_eq!(counter(&a), 10);
    assert_eq!(history(&a).len(), 1);
}

#[test]
fn existing_typed_roots_are_not_reimported_or_decoded_as_legacy_strings() {
    let a = Fixture::new();
    let b = Fixture::new();
    let path = "runtime/config/preferences/mixed-upgrade.preferences.json";
    let typed = stringPreferencesKey("typed");
    PreferencesDataStore::newWithStorage(b.host.clone(), path)
        .edit(|p| p.set(&typed, "a plain string"))
        .unwrap();
    let originals = history(&b);
    let journal = SyncOperationStore::new(a.host.clone(), "runtime/sync");
    journal.appendOperations(&originals).unwrap();
    a.host
        .writeBytesAtomically(
            path,
            br#"{"old":"{\"enabled\":true}","typed":"a plain string"}"#,
        )
        .unwrap();
    let schema = PreferenceMergeSchema {
        stores: vec![PreferenceMergeStore::values(path, false, 1, decode_old)],
    };
    applyOperations(a.host.clone(), &schema, &[], &[], |_, _| true).unwrap();
    let data = PreferencesDataStore::newWithStorage(a.host.clone(), path)
        .data()
        .unwrap();
    assert_eq!(data.get(&typed), Some(&json!("a plain string")));
    assert_eq!(
        data.get(&stringPreferencesKey("old")),
        Some(&json!({"enabled":true}))
    );
    let imported = history(&a);
    assert_eq!(imported.len(), originals.len() + 1);
    assert!(imported.contains(&originals[0]));
    applyOperations(a.host.clone(), &schema, &[], &[], |_, _| true).unwrap();
    assert_eq!(history(&a), imported);
}
