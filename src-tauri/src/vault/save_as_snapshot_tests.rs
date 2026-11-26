//! Native synthetic vaults with explicit prepare/edit/persist/complete ordering.
//! No Tauri IPC, actual remote provider, shared clipboard or user files.
use super::*;
use crate::remote::MemoryStorage;
use keepass::DatabaseKey;
use tempfile::TempDir;

const SYNTHETIC_PASSWORD: &str = "synthetic-save-as-only";

fn local(dir: &TempDir) -> (VaultSession, PathBuf) {
    let source = dir.path().join("synthetic-source.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&source, SYNTHETIC_PASSWORD, "Aes", "Aes256", "None", None)
        .unwrap();
    (session, source)
}

fn edit(session: &mut VaultSession) {
    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.into()),
            icon: None,
            name: "Synthetic concurrent edit".into(),
        })
        .unwrap();
}

fn complete_cached_edit(session: &mut VaultSession, target: PathBuf) -> (VaultState, VaultState) {
    let job = session.prepare_save_as(&target).unwrap();
    let revision = job.revision;
    edit(session);
    // A full read warms the CURRENT revision's cache while persistence retains
    // the old database clone. Merely returning a light mutation skips the bug.
    let before = session.state().unwrap().unwrap();
    assert_eq!(session.cached_snapshot.as_ref().unwrap().0, before.revision);
    persist_save(job).unwrap();
    let completed = session.complete_save_as(target, revision, None).unwrap();
    (before, completed)
}

fn remote(dir: &TempDir) -> (VaultSession, MemoryStorage, Vec<u8>) {
    let (mut seed, source) = local(dir);
    let bytes = std::fs::read(&source).unwrap();
    seed.close();
    let storage = MemoryStorage::default();
    storage.put("synthetic.kdbx", &bytes).unwrap();
    let mut session = VaultSession::default();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "synthetic.kdbx",
            SYNTHETIC_PASSWORD,
            None,
            RemoteMode::InMemory,
            &dir.path().join("unused-mirror"),
            0,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    (session, storage, bytes)
}

#[test]
fn native_save_as_cached_concurrent_edit_returns_new_target() {
    let dir = TempDir::new().unwrap();
    let (mut session, source) = local(&dir);
    let target = dir.path().join("synthetic-copy.kdbx");
    let (before, completed) = complete_cached_edit(&mut session, target.clone());
    assert_eq!(session.path.as_deref(), target.to_str());
    assert_eq!(before.path, source.to_string_lossy());
    assert_eq!(completed.path, target.to_string_lossy());
    assert_eq!(completed.file_name, "synthetic-copy.kdbx");
    assert!(completed.dirty);
    assert_eq!(completed.root.children.len(), 1);
    assert_eq!(completed.modified_at, before.modified_at);
    assert_eq!(session.state().unwrap().unwrap().path, completed.path);
    session.close();
}

#[test]
fn native_save_as_cached_concurrent_edit_resets_read_only_snapshot() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = local(&dir);
    // Inject the counter state using its production update API. This tests
    // recovery publication, not how a provider causes three write failures.
    for _ in 0..3 {
        session.note_save_failure();
    }
    assert!(session.is_read_only());
    let (before, completed) =
        complete_cached_edit(&mut session, dir.path().join("synthetic-recovered.kdbx"));
    assert!(before.read_only);
    assert!(!session.is_read_only());
    assert!(!completed.read_only);
    assert!(!session.state().unwrap().unwrap().read_only);
    assert!(completed.dirty);
    session.close();
}

#[test]
fn native_save_as_remote_cached_edit_returns_local_target() {
    let dir = TempDir::new().unwrap();
    let (mut session, storage, bytes) = remote(&dir);
    let target = dir.path().join("synthetic-local.kdbx");
    let (before, completed) = complete_cached_edit(&mut session, target.clone());
    assert!(before.path.starts_with("s3://"));
    assert!(session.remote.is_none());
    assert_eq!(completed.path, target.to_string_lossy());
    assert_eq!(completed.file_name, "synthetic-local.kdbx");
    assert_eq!(storage.get("synthetic.kdbx").unwrap(), bytes);
    session.close();
}

#[test]
fn native_save_as_remote_cached_edit_clears_warning_snapshot() {
    let dir = TempDir::new().unwrap();
    let (mut session, _, _) = remote(&dir);
    // No failing real provider is contacted: the stored warning is synthetic.
    session.mirror_warning = Some("Synthetic previous mirror warning".into());
    let (before, completed) =
        complete_cached_edit(&mut session, dir.path().join("synthetic-no-warning.kdbx"));
    assert!(before.mirror_warning.is_some());
    assert!(session.mirror_warning.is_none());
    assert!(completed.mirror_warning.is_none());
    assert!(session.state().unwrap().unwrap().mirror_warning.is_none());
    session.close();
}

#[test]
fn native_save_as_successive_targets_have_increasing_snapshot_revisions() {
    let dir = TempDir::new().unwrap();
    let (mut session, _) = local(&dir);
    let before = session.state().unwrap().unwrap();
    let first = session
        .save_as(&dir.path().join("synthetic-first.kdbx"))
        .unwrap();
    let second = session
        .save_as(&dir.path().join("synthetic-second.kdbx"))
        .unwrap();
    assert!(!first.dirty && !second.dirty);
    assert_ne!(first.path, second.path);
    assert!(
        before.revision < first.revision && first.revision < second.revision,
        "save-as snapshot revisions must advance: {:?}",
        [before.revision, first.revision, second.revision]
    );
    session.close();
}

#[test]
fn native_save_as_no_concurrent_edit_returns_clean_new_target() {
    let dir = TempDir::new().unwrap();
    let (mut session, source) = local(&dir);
    let bytes = std::fs::read(&source).unwrap();
    edit(&mut session);
    let target = dir.path().join("synthetic-normal.kdbx");
    let completed = session.save_as(&target).unwrap();
    assert_eq!(completed.path, target.to_string_lossy());
    assert!(!completed.dirty && !completed.read_only);
    session.close();
    let mut reopened = VaultSession::default();
    let state = reopened.open(&target, SYNTHETIC_PASSWORD, None).unwrap();
    assert_eq!(state.root.children.len(), 1);
    reopened.close();
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
}

#[test]
fn native_save_as_concurrent_edit_persists_only_to_new_target_on_next_save() {
    let dir = TempDir::new().unwrap();
    let (mut session, source) = local(&dir);
    let bytes = std::fs::read(&source).unwrap();
    let target = dir.path().join("synthetic-later.kdbx");
    let (_, completed) = complete_cached_edit(&mut session, target.clone());
    assert!(completed.dirty && session.dirty);
    let written = Database::parse(
        &std::fs::read(&target).unwrap(),
        DatabaseKey::new().with_password(SYNTHETIC_PASSWORD),
    )
    .unwrap();
    assert_eq!(written.root().groups().count(), 0);
    session.save().unwrap();
    session.close();
    let mut reopened = VaultSession::default();
    let state = reopened.open(&target, SYNTHETIC_PASSWORD, None).unwrap();
    assert_eq!(state.root.children.len(), 1);
    assert_eq!(state.root.children[0].name, "Synthetic concurrent edit");
    reopened.close();
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
}

#[test]
fn native_save_as_failed_write_preserves_target_cache_and_revision() {
    let dir = TempDir::new().unwrap();
    let (mut session, source) = local(&dir);
    let bytes = std::fs::read(&source).unwrap();
    edit(&mut session);
    let before = session.state().unwrap().unwrap();
    let before_db = session.require_db().unwrap().clone();
    assert!(session
        .save_as(&dir.path().join("missing-directory/synthetic-refused.kdbx"))
        .is_err());
    let after = session.state().unwrap().unwrap();
    assert_eq!(after.path, before.path);
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.dirty, before.dirty);
    assert_eq!(after.read_only, before.read_only);
    assert_eq!(session.require_db().unwrap(), &before_db);
    assert_eq!(std::fs::read(&source).unwrap(), bytes);
    session.close();
}
