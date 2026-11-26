//! Disposable synthetic KDBX files / offline MemoryStorage only.
use super::*;
use crate::remote::{MemoryStorage, RemoteStorage};
use crate::vault::{helpers::default_kdf_config, RemoteMode, DEFAULT_BACKUP_TEMPLATE};
use keepass::{config::KdfConfig, db::Value, Database, DatabaseKey};
use std::{path::Path, sync::Arc};
use tempfile::TempDir;

fn weak(algorithm: &str, version: u32) -> KdfParameters {
    if algorithm == "Aes" {
        KdfParameters::Aes { rounds: "1".into() }
    } else {
        let wire = serde_json::json!({
            "algorithm": algorithm, "iterations": "1", "memoryBytes": "1048576",
            "parallelism": 1, "version": version,
        });
        serde_json::from_value(wire).unwrap()
    }
}

fn bytes(parameters: &KdfParameters) -> Vec<u8> {
    let mut db = Database::new();
    db.config.kdf_config = parameters.to_config().unwrap();
    let group = db.root_mut().add_group().id();
    let entry = db.group_mut(group).unwrap().add_entry().id();
    {
        let mut entry = db.entry_mut(entry).unwrap();
        entry
            .fields
            .insert("Password".into(), Value::protected("synthetic-entry-only"));
        entry.add_attachment(
            "synthetic.txt",
            Value::protected(b"synthetic attachment".to_vec()),
        );
        entry
            .track_changes()
            .fields
            .insert("Notes".into(), Value::Unprotected("synthetic note".into()));
    }
    let mut bytes = Vec::new();
    db.save(
        &mut bytes,
        DatabaseKey::new().with_password("synthetic-master-only"),
    )
    .unwrap();
    bytes
}

fn open(path: &Path, parameters: &KdfParameters) -> VaultSession {
    std::fs::write(path, bytes(parameters)).unwrap();
    let mut session = VaultSession::default();
    session.open(path, "synthetic-master-only", None).unwrap();
    session
}

#[test]
fn upgrades_preserve_algorithm_and_contents_and_only_save_changes_the_file() {
    for (algorithm, version) in [
        ("Aes", 19),
        ("Argon2", 16),
        ("Argon2", 19),
        ("Argon2id", 19),
    ] {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("synthetic-upgrade.kdbx");
        let mut session = open(&path, &weak(algorithm, version));
        let original_bytes = std::fs::read(&path).unwrap();
        let original = session.require_db().unwrap().clone();
        let revision = session.revision;
        let offer = session.kdf_upgrade_offer().unwrap();
        assert!(offer.parameters.is_weak().unwrap());
        let recommended = offer.recommended.clone().unwrap();
        assert!(!recommended.is_weak().unwrap());
        assert_eq!(recommended.algorithm(), algorithm);
        assert_eq!(
            recommended.to_config().unwrap(),
            default_kdf_config(algorithm).unwrap()
        );
        let wire = serde_json::to_string(&offer).unwrap();
        assert!(!wire.contains("synthetic-master-only"));
        assert!(!wire.contains("synthetic-entry-only"));
        let state = session
            .upgrade_weak_kdf(&offer.token, &offer.parameters)
            .unwrap();
        assert!(state.dirty);
        assert_eq!(state.revision, revision + 1);
        assert_eq!(std::fs::read(&path).unwrap(), original_bytes);
        let mut expected = original;
        expected.config.kdf_config = recommended.to_config().unwrap();
        assert_eq!(session.require_db().unwrap(), &expected);
        session.save().unwrap();
        session.close();
        session.open(&path, "synthetic-master-only", None).unwrap();
        assert_eq!(
            session.database_settings().unwrap().unwrap().kdf_parameters,
            recommended
        );
        assert_eq!(*session.require_db().unwrap().root(), *expected.root());
        session.close();
    }
}

#[test]
fn weak_threshold_matches_official_boundaries_and_large_integer_products() {
    for (rounds, expected) in [
        (0, true),
        (599999, true),
        (600000, false),
        (u64::MAX, false),
    ] {
        assert_eq!(
            KdfParameters::Aes {
                rounds: rounds.to_string()
            }
            .is_weak()
            .unwrap(),
            expected
        );
    }
    for algorithm in ["Argon2", "Argon2id"] {
        for (iterations, memory, expected) in [
            (2, 67108863, true),
            (2, 67108864, false),
            (4, 33554432, false),
            (u32::MAX as u64, 4398046511103u64, false),
        ] {
            let wire = serde_json::json!({ "algorithm": algorithm, "iterations": iterations.to_string(), "memoryBytes": memory.to_string(), "parallelism": 1, "version": 19 });
            let parameters: KdfParameters = serde_json::from_value(wire).unwrap();
            assert_eq!(parameters.is_weak().unwrap(), expected);
        }
    }
}

#[test]
fn stale_parameters_and_same_path_replacements_reject_without_mutation() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-stale.kdbx");
    let mut session = open(&path, &weak("Aes", 19));
    let offer = session.kdf_upgrade_offer().unwrap();
    session.require_db_mut().unwrap().config.kdf_config = KdfConfig::Aes { rounds: 2 };
    let before = session.require_db().unwrap().clone();
    let revision = session.revision;
    assert_eq!(
        session
            .upgrade_weak_kdf(&offer.token, &offer.parameters)
            .err()
            .unwrap(),
        CHANGED
    );
    assert_eq!(session.require_db().unwrap(), &before);
    assert_eq!(session.revision, revision);
    session.close();
    session.open(&path, "synthetic-master-only", None).unwrap();
    assert_eq!(
        session
            .upgrade_weak_kdf(&offer.token, &offer.parameters)
            .err()
            .unwrap(),
        CHANGED
    );
    assert!(!session.dirty);
    session.close();
}

#[test]
fn read_only_closed_and_already_strong_sessions_cannot_upgrade() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-readonly.kdbx");
    let mut session = open(&path, &weak("Aes", 19));
    let offer = session.kdf_upgrade_offer().unwrap();
    while !session.is_read_only() {
        session.note_save_failure();
    }
    assert!(session.kdf_upgrade_offer().unwrap().read_only);
    assert!(session
        .upgrade_weak_kdf(&offer.token, &offer.parameters)
        .is_err());
    assert!(!session.dirty);
    session.close();
    assert!(session
        .upgrade_weak_kdf(&offer.token, &offer.parameters)
        .is_err());
    session.open(&path, "synthetic-master-only", None).unwrap();
    session.require_db_mut().unwrap().config.kdf_config = default_kdf_config("Aes").unwrap();
    let strong = session.kdf_upgrade_offer().unwrap();
    assert!(strong.recommended.is_none());
    assert!(session
        .upgrade_weak_kdf(&strong.token, &strong.parameters)
        .is_err());
    session.close();
}

#[tokio::test]
async fn persistence_permit_waits_without_holding_vault_lock_and_targets_the_parked_origin() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-serialized.kdbx");
    std::fs::write(&path, bytes(&weak("Aes", 19))).unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let a = vaults
        .open(&mut active.lock().unwrap(), |fresh| {
            fresh.open(&path, "synthetic-master-only", None)
        })
        .unwrap()
        .session_id;
    let offer = active.lock().unwrap().kdf_upgrade_offer().unwrap();
    let permit = vaults.acquire_persistence().unwrap();
    let old_job = active.lock().unwrap().prepare_save(false).unwrap();
    let old_revision = old_job.revision;
    let mut run = Box::pin(stage_weak_kdf(
        &vaults,
        &active,
        &a,
        &offer.token,
        &offer.parameters,
    ));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), &mut run)
            .await
            .is_err()
    );
    let b = vaults
        .open(&mut active.try_lock().unwrap(), |fresh| {
            fresh.open(&path, "synthetic-master-only", None)
        })
        .unwrap()
        .session_id;
    let old_outcome = crate::vault::persist_save(old_job).unwrap();
    vaults
        .with_session_mut(&mut active.lock().unwrap(), Some(&a), |session| {
            session.complete_save(old_revision, old_outcome)
        })
        .unwrap();
    let saved_before_upgrade = std::fs::read(&path).unwrap();
    drop(permit);
    let result = run.await.unwrap();
    assert!(result.dirty);
    assert_eq!(vaults.active_id(), Some(b));
    assert!(!active.lock().unwrap().dirty);
    assert_eq!(std::fs::read(&path).unwrap(), saved_before_upgrade);
    assert!(vaults
        .with_session_mut(&mut active.lock().unwrap(), Some(&a), |session| {
            Ok(session.kdf_upgrade_offer()?.recommended.is_none())
        })
        .unwrap());
    assert!(vaults
        .with_session_mut(&mut active.lock().unwrap(), Some(&a), |session| Ok(
            session.dirty
        ))
        .unwrap());
    vaults
        .close_all(&mut active.lock().unwrap(), false)
        .unwrap();
}

#[test]
fn offline_remote_upgrade_does_not_upload_until_save() {
    let dir = TempDir::new().unwrap();
    let storage = Arc::new(MemoryStorage::default());
    let original = bytes(&weak("Aes", 19));
    storage.put("synthetic.kdbx", &original).unwrap();
    let mut session = VaultSession::default();
    session
        .open_remote(
            storage.clone(),
            "synthetic.kdbx",
            "synthetic-master-only",
            None,
            RemoteMode::InMemory,
            dir.path(),
            0,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    let offer = session.kdf_upgrade_offer().unwrap();
    session
        .upgrade_weak_kdf(&offer.token, &offer.parameters)
        .unwrap();
    assert_eq!(storage.get("synthetic.kdbx").unwrap(), original);
    session.save().unwrap();
    session.close();
    session
        .open_remote(
            storage,
            "synthetic.kdbx",
            "synthetic-master-only",
            None,
            RemoteMode::InMemory,
            dir.path(),
            0,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    assert!(session.kdf_upgrade_offer().unwrap().recommended.is_none());
    session.close();
}
