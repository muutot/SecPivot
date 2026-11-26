use super::*;
use crate::vault::{GroupInput, VaultSession, VaultSessions, ROOT_GROUP_UUID};
use tempfile::TempDir;

fn session_fixture() -> (TempDir, VaultSession, PathBuf) {
    let root = TempDir::new().unwrap();
    let path = root.path().join("synthetic.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-old-key", "Aes", "Aes256", "None", None)
        .unwrap();
    session.local_backup = enabled(2);
    (root, session, path)
}
fn edit(session: &mut VaultSession, name: &str) {
    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.into()),
            icon: None,
            name: name.into(),
        })
        .unwrap();
}
fn open_groups(path: &Path, password: &str) -> usize {
    let mut session = VaultSession::default();
    let state = session.open(path, password, None).unwrap();
    session.close();
    state.root.children.len()
}

fn enabled(count: i32) -> LocalBackupSettings {
    LocalBackupSettings {
        enabled: true,
        count,
        ..Default::default()
    }
}
fn names(path: &Path) -> Vec<String> {
    load_index(&directory(path).unwrap()).unwrap().files
}

#[test]
fn enabled_writes_preserve_previous_bytes_and_prune_only_indexed_backups() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("seed.kdbx");
    write(&path, b"encrypted-first", &enabled(2)).unwrap();
    assert!(!directory(&path).unwrap().exists());
    write(&path, b"encrypted-second", &enabled(2)).unwrap();
    let dir = directory(&path).unwrap();
    let first = names(&path)[0].clone();
    assert_eq!(fs::read(dir.join(&first)).unwrap(), b"encrypted-first");
    let untracked = dir.join("seed.20000101000000.000.kdbx.bak");
    fs::write(&untracked, b"untracked synthetic data").unwrap();
    write(&path, b"encrypted-third", &enabled(2)).unwrap();
    write(&path, b"encrypted-fourth", &enabled(2)).unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"encrypted-fourth");
    assert_eq!(names(&path).len(), 2);
    assert!(!dir.join(first).exists());
    assert_eq!(fs::read(untracked).unwrap(), b"untracked synthetic data");
    let stored: Vec<_> = names(&path)
        .iter()
        .map(|name| fs::read(dir.join(name)).unwrap())
        .collect();
    assert_eq!(
        stored,
        vec![b"encrypted-second".to_vec(), b"encrypted-third".to_vec()]
    );
}

#[test]
fn failed_primary_writes_retain_the_old_file_and_every_existing_backup() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("seed.kdbx");
    fs::write(&path, b"first encrypted version").unwrap();
    write(&path, b"second encrypted version", &enabled(1)).unwrap();
    let dir = directory(&path).unwrap();
    let first = names(&path)[0].clone();
    let error = write_with(&path, b"not committed", &enabled(1), |_, _| {
        Err("synthetic write refused".into())
    })
    .unwrap_err();
    assert!(error.contains("refused"));
    assert_eq!(fs::read(&path).unwrap(), b"second encrypted version");
    assert_eq!(
        fs::read(dir.join(first)).unwrap(),
        b"first encrypted version"
    );
    assert_eq!(
        names(&path).len(),
        2,
        "no retention before a successful primary commit"
    );
    let newest = names(&path).pop().unwrap();
    assert_eq!(
        fs::read(dir.join(newest)).unwrap(),
        b"second encrypted version"
    );
}

#[test]
fn invalid_templates_or_indexes_never_modify_the_primary_or_unrelated_files() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("seed.kdbx");
    fs::write(&path, b"old encrypted file").unwrap();
    for template in [
        "../{name}.{timestamp}.bak",
        "{name}.bak",
        "{timestamp}.{unknown}",
        "{timestamp}.{timestamp}",
    ] {
        let settings = LocalBackupSettings {
            template: template.into(),
            ..enabled(3)
        };
        assert!(write(&path, b"new", &settings).is_err());
        assert!(!directory(&path).unwrap().exists());
        assert_eq!(fs::read(&path).unwrap(), b"old encrypted file");
    }
    let dir = directory(&path).unwrap();
    fs::create_dir(&dir).unwrap();
    let index = br#"{"version":1,"files":["../unrelated.kdbx"]}"#;
    fs::write(dir.join(INDEX_NAME), index).unwrap();
    assert!(write(&path, b"new", &enabled(3)).is_err());
    assert_eq!(fs::read(&path).unwrap(), b"old encrypted file");
    assert_eq!(fs::read(dir.join(INDEX_NAME)).unwrap(), index);
}

#[test]
fn disabled_backups_create_no_metadata_and_fixed_stamp_collisions_preserve_existing_files() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("seed.kdbx");
    write(&path, b"one", &LocalBackupSettings::default()).unwrap();
    write(&path, b"two", &LocalBackupSettings::default()).unwrap();
    assert!(!directory(&path).unwrap().exists());
    let template = "{name}.{timestamp}.{ext}.bak";
    let first = create_backup(
        root.path(),
        template,
        "seed",
        "kdbx",
        b"first",
        "20261127120000.000",
    )
    .unwrap();
    let second = create_backup(
        root.path(),
        template,
        "seed",
        "kdbx",
        b"second",
        "20261127120000.000",
    )
    .unwrap();
    assert_ne!(first, second);
    assert_eq!(fs::read(root.path().join(first)).unwrap(), b"first");
    assert_eq!(fs::read(root.path().join(second)).unwrap(), b"second");
}

#[test]
fn retention_failure_is_a_post_commit_warning_and_preserves_nonfiles() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("seed.kdbx");
    fs::write(&path, b"one").unwrap();
    write(&path, b"two", &enabled(1)).unwrap();
    let old = directory(&path).unwrap().join(&names(&path)[0]);
    fs::remove_file(&old).unwrap();
    fs::create_dir(&old).unwrap();
    let warning = write(&path, b"three", &enabled(1)).unwrap().unwrap();
    assert!(warning.contains("本地已保存"));
    assert_eq!(fs::read(&path).unwrap(), b"three");
    assert!(old.is_dir());
    assert_eq!(names(&path).len(), 2);
}

#[test]
fn backup_writer_lock_is_exclusive_and_released_when_the_handle_drops() {
    let root = TempDir::new().unwrap();
    let dir = root.path().join("backups");
    let first = lock_directory(&dir).unwrap();
    assert!(lock_directory(&dir).is_err());
    drop(first);
    assert!(lock_directory(&dir).is_ok());
}

#[test]
fn native_kdbx_saves_reopen_previous_states_and_preserve_concurrent_edits() {
    let (_root, mut session, path) = session_fixture();
    let original = fs::read(&path).unwrap();
    edit(&mut session, "first synthetic edit");
    let job = session.prepare_save(false).unwrap();
    let revision = job.revision;
    edit(&mut session, "later synthetic edit");
    let outcome = crate::vault::persist_save(job).unwrap();
    let state = session.complete_save(revision, outcome).unwrap();
    assert!(state.dirty);
    assert_eq!(state.root.children.len(), 2);
    assert_eq!(open_groups(&path, "synthetic-old-key"), 1);
    let dir = directory(&path).unwrap();
    let first = names(&path)[0].clone();
    assert_eq!(fs::read(dir.join(&first)).unwrap(), original);
    assert_eq!(open_groups(&dir.join(first), "synthetic-old-key"), 0);
    assert!(!session.save().unwrap().dirty);
    assert_eq!(open_groups(&path, "synthetic-old-key"), 2);
    let last = names(&path).pop().unwrap();
    assert_eq!(open_groups(&dir.join(last), "synthetic-old-key"), 1);
    session.close();
}

#[test]
fn native_master_key_changes_backup_old_encryption_and_commit_new_encryption() {
    let (_root, mut session, path) = session_fixture();
    let original = fs::read(&path).unwrap();
    session
        .change_master_key("synthetic-new-key", None)
        .unwrap();
    let backup = directory(&path).unwrap().join(&names(&path)[0]);
    assert_eq!(fs::read(&backup).unwrap(), original);
    assert_eq!(open_groups(&backup, "synthetic-old-key"), 0);
    assert_eq!(open_groups(&path, "synthetic-new-key"), 0);
    let mut rejected = VaultSession::default();
    assert!(rejected.open(&path, "synthetic-old-key", None).is_err());
    session.close();
}

#[test]
fn native_creation_overwrites_only_after_backing_up_the_existing_encrypted_database() {
    let (_root, mut original, path) = session_fixture();
    edit(&mut original, "previous synthetic contents");
    original.save().unwrap();
    let previous = fs::read(&path).unwrap();
    original.close();
    let mut replacement = VaultSession {
        local_backup: enabled(2),
        ..Default::default()
    };
    replacement
        .create(&path, "synthetic-new-key", "Aes", "Aes256", "None", None)
        .unwrap();
    let backup = directory(&path).unwrap().join(names(&path).last().unwrap());
    assert_eq!(fs::read(&backup).unwrap(), previous);
    assert_eq!(open_groups(&backup, "synthetic-old-key"), 1);
    assert_eq!(open_groups(&path, "synthetic-new-key"), 0);
    let mut rejected = VaultSession::default();
    assert!(rejected.open(&path, "synthetic-old-key", None).is_err());
    replacement.close();
}

#[test]
fn native_save_as_backs_up_the_existing_destination_and_preserves_the_source() {
    let (root, mut source, source_path) = session_fixture();
    let original_source = fs::read(&source_path).unwrap();
    let target = root.path().join("destination.kdbx");
    let mut existing = VaultSession::default();
    existing
        .create(
            &target,
            "synthetic-destination-key",
            "Aes",
            "Aes256",
            "None",
            None,
        )
        .unwrap();
    let original_target = fs::read(&target).unwrap();
    existing.close();
    edit(&mut source, "new synthetic contents");
    let saved = source.save_as(&target).unwrap();
    assert!(!saved.dirty);
    assert_eq!(saved.path, target.to_string_lossy());
    assert_eq!(fs::read(&source_path).unwrap(), original_source);
    let backup = directory(&target).unwrap().join(&names(&target)[0]);
    assert_eq!(fs::read(&backup).unwrap(), original_target);
    assert_eq!(open_groups(&backup, "synthetic-destination-key"), 0);
    assert_eq!(open_groups(&target, "synthetic-old-key"), 1);
    source.close();
}

#[test]
fn native_registry_updates_active_parked_future_sessions_but_not_captured_jobs() {
    let root = TempDir::new().unwrap();
    let registry = VaultSessions::default();
    let mut active = VaultSession::default();
    registry.set_local_backup(&mut active, &enabled(3)).unwrap();
    let first_path = root.path().join("first.kdbx");
    let first = registry
        .open(&mut active, |session| {
            session.create(&first_path, "synthetic", "Aes", "Aes256", "None", None)
        })
        .unwrap();
    let captured = active.prepare_save(false).unwrap();
    let second_path = root.path().join("second.kdbx");
    registry
        .open(&mut active, |session| {
            session.create(&second_path, "synthetic", "Aes", "Aes256", "None", None)
        })
        .unwrap();
    registry.set_local_backup(&mut active, &enabled(1)).unwrap();
    assert_eq!(active.local_backup.count, 1);
    registry
        .with_session_mut(&mut active, Some(&first.session_id), |session| {
            assert_eq!(session.local_backup.count, 1);
            Ok(())
        })
        .unwrap();
    let third_path = root.path().join("third.kdbx");
    registry
        .open(&mut active, |session| {
            session.create(&third_path, "synthetic", "Aes", "Aes256", "None", None)
        })
        .unwrap();
    assert_eq!(active.local_backup.count, 1);
    assert!(
        matches!(captured.target, crate::vault::persist::SaveTarget::LocalBackup { ref settings, .. } if settings.count == 3)
    );
    registry
        .set_local_backup(&mut active, &LocalBackupSettings::default())
        .unwrap();
    assert!(matches!(
        active.prepare_save(false).unwrap().target,
        crate::vault::persist::SaveTarget::Local(_)
    ));
    registry.close_all(&mut active, false).unwrap();
}

#[tokio::test]
async fn native_auto_save_on_close_runs_the_same_backup_transaction() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("close.kdbx");
    let registry = VaultSessions::default();
    let mut session = VaultSession::default();
    registry
        .set_local_backup(&mut session, &enabled(2))
        .unwrap();
    registry
        .open(&mut session, |session| {
            session.create(&path, "synthetic-old-key", "Aes", "Aes256", "None", None)
        })
        .unwrap();
    edit(&mut session, "saved on synthetic close");
    let active = std::sync::Mutex::new(session);
    let closed = crate::vault::close::close_with_save(
        &registry,
        &active,
        crate::vault::close::CloseTarget::All { keep_rpc: false },
        true,
        |_| async { panic!("auto-save must not ask a native prompt") },
    )
    .await
    .unwrap();
    assert!(!closed.any_open);
    assert!(!active.lock().unwrap().is_open());
    assert_eq!(open_groups(&path, "synthetic-old-key"), 1);
    let backup = directory(&path).unwrap().join(&names(&path)[0]);
    assert_eq!(open_groups(&backup, "synthetic-old-key"), 0);
}

#[test]
fn native_busy_backup_lock_does_not_degrade_the_session_and_uri_targets_keep_existing_io() {
    let (_root, mut session, path) = session_fixture();
    let dir = directory(&path).unwrap();
    let lock = lock_directory(&dir).unwrap();
    for _ in 0..4 {
        assert_eq!(session.save().unwrap_err(), BUSY);
    }
    assert_eq!(session.save_failures, 0);
    drop(lock);
    assert!(session.save().is_ok());
    for uri in ["content://synthetic/document", "file://synthetic/document"] {
        assert!(matches!(
            session.prepare_save_as(Path::new(uri)).unwrap().target,
            crate::vault::persist::SaveTarget::Local(_)
        ));
    }
    session.close();
}
