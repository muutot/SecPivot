//! Master-key dates use real temporary KDBX files and offline storage only.
use super::*;
use crate::remote::{MemoryStorage, RemoteStorage};
use chrono::{NaiveDateTime, SubsecRound, Utc};
use keepass::{config::KdfConfig, DatabaseKey};
use std::sync::Arc;
use tempfile::TempDir;

const OLD_PASSWORD: &str = "synthetic-old-key-time-only";
const NEW_PASSWORD: &str = "synthetic-new-key-time-only";

fn old_date() -> NaiveDateTime {
    chrono::DateTime::from_timestamp(946_684_800, 0)
        .unwrap()
        .naive_utc()
}

fn bytes() -> Vec<u8> {
    let mut db = Database::new();
    db.config.kdf_config = KdfConfig::Aes { rounds: 1 };
    db.meta.master_key_changed = Some(old_date());
    db.meta.master_key_change_rec = Some(30);
    db.meta.master_key_change_force = Some(60);
    let mut bytes = Vec::new();
    db.save(&mut bytes, DatabaseKey::new().with_password(OLD_PASSWORD))
        .unwrap();
    bytes
}

fn fixture(dir: &TempDir) -> (VaultSession, PathBuf) {
    let path = dir.path().join("synthetic-key-time.kdbx");
    std::fs::write(&path, bytes()).unwrap();
    let mut session = VaultSession::default();
    session.open(&path, OLD_PASSWORD, None).unwrap();
    (session, path)
}

fn date(session: &VaultSession) -> NaiveDateTime {
    session
        .require_db()
        .unwrap()
        .meta
        .master_key_changed
        .unwrap()
}

fn assert_current(value: NaiveDateTime, earliest: NaiveDateTime) {
    assert!(value >= earliest);
    assert!(value <= Utc::now().naive_utc());
    assert_eq!(value, value.trunc_subsecs(0));
}

#[test]
fn master_key_time_creation_initializes_local_and_remote_committed_dates() {
    let dir = TempDir::new().unwrap();
    let earliest = Utc::now().naive_utc().trunc_subsecs(0);
    let path = dir.path().join("new-synthetic.kdbx");
    let mut local = VaultSession::default();
    local
        .create(&path, OLD_PASSWORD, "Aes", "Aes256", "None", None)
        .unwrap();
    let created = date(&local);
    assert_current(created, earliest);
    local.save().unwrap();
    local.close();
    local.open(&path, OLD_PASSWORD, None).unwrap();
    assert_eq!(date(&local), created);
    let storage = Arc::new(MemoryStorage::default());
    let mut remote = VaultSession::default();
    remote
        .create_remote(
            storage.clone(),
            "new.kdbx",
            OLD_PASSWORD,
            "Aes",
            "Aes256",
            "None",
            None,
            RemoteMode::InMemory,
            dir.path(),
            0,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    let created = date(&remote);
    assert_current(created, earliest);
    let stored = Database::parse(
        &storage.get("new.kdbx").unwrap(),
        DatabaseKey::new().with_password(OLD_PASSWORD),
    )
    .unwrap();
    assert_eq!(stored.meta.master_key_changed, Some(created));
    remote.close();
    local.close();
}

#[test]
fn master_key_time_ordinary_save_save_as_and_kdf_changes_preserve_existing_date() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = fixture(&dir);
    session.save().unwrap();
    assert_eq!(date(&session), old_date());
    session
        .update_database_settings(&DatabaseSettingsPatch {
            compression: Some("Gzip".into()),
            kdf_parameters: Some(KdfParameters::Aes { rounds: "2".into() }),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(date(&session), old_date());
    let path = dir.path().join("copied-synthetic.kdbx");
    session.save_as(&path).unwrap();
    session.close();
    session.open(&path, OLD_PASSWORD, None).unwrap();
    assert_eq!(date(&session), old_date());
    assert_eq!(
        session.require_db().unwrap().meta.master_key_change_rec,
        Some(30)
    );
    assert_eq!(
        session.require_db().unwrap().meta.master_key_change_force,
        Some(60)
    );
    session.close();
}

#[test]
fn master_key_time_completion_preserves_concurrent_edits_and_exact_committed_date() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = fixture(&dir);
    let (db, target, revision) = session.prepare_change().unwrap();
    let earliest = Utc::now().naive_utc().trunc_subsecs(0);
    let (outcome, changed_at) = persist_change(db, NEW_PASSWORD, None, &target).unwrap();
    assert_current(changed_at, earliest);
    assert_eq!(
        date(&session),
        old_date(),
        "preparation/persistence must not prematurely adopt"
    );
    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.into()),
            icon: None,
            name: "Concurrent synthetic edit".into(),
        })
        .unwrap();
    let after = session
        .complete_change(NEW_PASSWORD, None, revision, outcome, changed_at)
        .unwrap();
    assert!(after.dirty);
    assert_eq!(after.root.children.len(), 1);
    assert_eq!(date(&session), changed_at);
    let stored = Database::parse(
        &std::fs::read(&path).unwrap(),
        DatabaseKey::new().with_password(NEW_PASSWORD),
    )
    .unwrap();
    assert_eq!(stored.meta.master_key_changed, Some(changed_at));
    session.save().unwrap();
    session.close();
    assert!(session.open(&path, OLD_PASSWORD, None).is_err());
    session.open(&path, NEW_PASSWORD, None).unwrap();
    assert_eq!(date(&session), changed_at);
    assert_eq!(session.state().unwrap().unwrap().root.children.len(), 1);
    assert_eq!(
        session.require_db().unwrap().meta.master_key_change_force,
        Some(60)
    );
    session.close();
}

#[test]
fn master_key_time_refused_changes_preserve_metadata_credentials_and_original_file() {
    let dir = TempDir::new().unwrap();
    let (mut session, path) = fixture(&dir);
    let original = std::fs::read(&path).unwrap();
    assert!(session
        .change_master_key(NEW_PASSWORD, Some(&dir.path().join("missing.key")))
        .is_err());
    assert_eq!(date(&session), old_date());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(session.change_master_key(NEW_PASSWORD, None).is_err());
    assert_eq!(date(&session), old_date());
    assert!(session.password.as_deref() == Some(OLD_PASSWORD));
    std::fs::remove_dir(&path).unwrap();
    std::fs::write(&path, &original).unwrap();
    for _ in 0..3 {
        session.note_save_failure();
    }
    assert!(session.change_master_key(NEW_PASSWORD, None).is_err());
    assert_eq!(date(&session), old_date());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    session.close();
    let mut closed = VaultSession::default();
    assert!(closed
        .complete_change(NEW_PASSWORD, None, 0, [0; 32], old_date())
        .is_err());
    assert!(closed.password.is_none());
}

#[test]
fn master_key_time_keyfile_only_and_remote_changes_commit_the_new_date() {
    let dir = TempDir::new().unwrap();
    let (mut local, path) = fixture(&dir);
    let keyfile = dir.path().join("synthetic.key");
    std::fs::write(&keyfile, b"0123456789abcdef0123456789abcdef").unwrap();
    let earliest = Utc::now().naive_utc().trunc_subsecs(0);
    local.change_master_key("", Some(&keyfile)).unwrap();
    let changed_at = date(&local);
    assert_current(changed_at, earliest);
    local.close();
    local.open(&path, "", Some(&keyfile)).unwrap();
    assert_eq!(date(&local), changed_at);
    local.close();
    let storage = Arc::new(MemoryStorage::default());
    storage.put("synthetic.kdbx", &bytes()).unwrap();
    let mut remote = VaultSession::default();
    remote
        .open_remote(
            storage.clone(),
            "synthetic.kdbx",
            OLD_PASSWORD,
            None,
            RemoteMode::InMemory,
            dir.path(),
            0,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    remote.change_master_key(NEW_PASSWORD, None).unwrap();
    let changed_at = date(&remote);
    assert_current(changed_at, earliest);
    remote.save().unwrap();
    remote.close();
    remote
        .open_remote(
            storage,
            "synthetic.kdbx",
            NEW_PASSWORD,
            None,
            RemoteMode::InMemory,
            dir.path(),
            0,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    assert_eq!(date(&remote), changed_at);
    remote.close();
}
