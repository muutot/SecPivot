//! Real disposable KDBX and offline MemoryStorage; no GUI or provider access.
use super::*;
use crate::remote::{MemoryStorage, RemoteStorage};
use keepass::{config::KdfConfig, DatabaseKey};
use std::sync::Arc;
use tempfile::TempDir;

const PASSWORD: &str = "synthetic-force-once-only";

fn bytes(flag: Option<bool>) -> Vec<u8> {
    let mut db = Database::new();
    db.config.kdf_config = KdfConfig::Aes { rounds: 1 };
    db.meta.master_key_change_force_once = flag;
    db.meta.master_key_change_rec = Some(-1);
    db.meta.master_key_change_force = Some(42);
    let mut bytes = Vec::new();
    db.save(&mut bytes, DatabaseKey::new().with_password(PASSWORD))
        .unwrap();
    bytes
}

fn assert_flag(db: &Database, flag: Option<bool>) {
    assert_eq!(db.meta.master_key_change_force_once, flag);
    assert_eq!(db.meta.master_key_change_rec, Some(-1));
    assert_eq!(db.meta.master_key_change_force, Some(42));
}

#[test]
fn master_key_force_once_local_save_save_as_and_kdf_updates_retain_all_states() {
    let dir = TempDir::new().unwrap();
    for (index, flag) in [None, Some(false), Some(true)].into_iter().enumerate() {
        let path = dir.path().join(format!("synthetic-{index}.kdbx"));
        std::fs::write(&path, bytes(flag)).unwrap();
        let mut session = VaultSession::default();
        session.open(&path, PASSWORD, None).unwrap();
        assert_flag(session.require_db().unwrap(), flag);
        session.save().unwrap();
        session
            .update_database_settings(&DatabaseSettingsPatch {
                kdf_parameters: Some(KdfParameters::Aes { rounds: "2".into() }),
                compression: Some("Gzip".into()),
                ..Default::default()
            })
            .unwrap();
        assert_flag(session.require_db().unwrap(), flag);
        let copied = dir.path().join(format!("synthetic-copy-{index}.kdbx"));
        session.save_as(&copied).unwrap();
        session.close();
        for file in [&path, &copied] {
            session.open(file, PASSWORD, None).unwrap();
            assert_flag(session.require_db().unwrap(), flag);
            session.close();
        }
    }
}

#[test]
fn master_key_force_once_offline_remote_save_reopen_retains_all_states() {
    let dir = TempDir::new().unwrap();
    let storage = Arc::new(MemoryStorage::default());
    for (index, flag) in [None, Some(false), Some(true)].into_iter().enumerate() {
        let key = format!("synthetic-{index}.kdbx");
        storage.put(&key, &bytes(flag)).unwrap();
        let mut session = VaultSession::default();
        session
            .open_remote(
                storage.clone(),
                &key,
                PASSWORD,
                None,
                RemoteMode::InMemory,
                dir.path(),
                0,
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap();
        session.save().unwrap();
        let stored = Database::parse(
            &storage.get(&key).unwrap(),
            DatabaseKey::new().with_password(PASSWORD),
        )
        .unwrap();
        assert_flag(&stored, flag);
        session.close();
        session
            .open_remote(
                storage.clone(),
                &key,
                PASSWORD,
                None,
                RemoteMode::InMemory,
                dir.path(),
                0,
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap();
        assert_flag(session.require_db().unwrap(), flag);
        session.close();
    }
}

#[test]
fn master_key_force_once_read_only_refusal_retains_flag_and_original_bytes() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-read-only.kdbx");
    let original = bytes(Some(true));
    std::fs::write(&path, &original).unwrap();
    let mut session = VaultSession::default();
    session.open(&path, PASSWORD, None).unwrap();
    for _ in 0..3 {
        session.note_save_failure();
    }
    assert!(session.save().is_err());
    assert_flag(session.require_db().unwrap(), Some(true));
    assert_eq!(std::fs::read(&path).unwrap(), original);
    session.close();
}
