use super::*;
use crate::vault::{GroupInput, RemoteMode, VaultSessions, ROOT_GROUP_UUID};
use std::path::Path;
use tempfile::TempDir;

fn fixture() -> (TempDir, PathBuf, VaultSession) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-key", "Aes", "Aes256", "None", None)
        .unwrap();
    (dir, path, session)
}
fn add(session: &mut VaultSession, name: &str) {
    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.into()),
            icon: None,
            name: name.into(),
        })
        .unwrap();
}
fn external(path: &Path, name: &str) {
    let mut other = VaultSession::default();
    other.open(path, "synthetic-key", None).unwrap();
    add(&mut other, name);
    other.save().unwrap();
    other.close();
}
fn offer(session: &mut VaultSession) -> FileReloadResult {
    let ticket = session.prepare_file_check().unwrap();
    let changed = check_file(&ticket).unwrap().unwrap();
    let job = session.prepare_file_reload(&ticket, changed).unwrap();
    session
        .stage_file_reload(ticket, reload_file(job).unwrap())
        .unwrap()
}

#[test]
fn unchanged_files_skip_reload_and_open_save_and_save_as_track_exact_bytes() {
    let (dir, path, mut session) = fixture();
    assert!(check_file(&session.prepare_file_check().unwrap())
        .unwrap()
        .is_none());
    add(&mut session, "saved");
    session.save().unwrap();
    assert_eq!(
        session.local_base_hash,
        Some(crate::crypto::sha256_bytes(&std::fs::read(&path).unwrap()))
    );
    assert!(check_file(&session.prepare_file_check().unwrap())
        .unwrap()
        .is_none());
    let target = dir.path().join("saved-as.kdbx");
    session.save_as(&target).unwrap();
    assert!(check_file(&session.prepare_file_check().unwrap())
        .unwrap()
        .is_none());
    session.close();
    assert!(session.local_base_hash.is_none());
    session.open(&target, "synthetic-key", None).unwrap();
    assert!(check_file(&session.prepare_file_check().unwrap())
        .unwrap()
        .is_none());
    session.close();
}

#[test]
fn clean_reload_is_staged_then_adopted_once_without_writing_or_changing_credentials() {
    let (_dir, path, mut session) = fixture();
    external(&path, "external");
    let bytes = std::fs::read(&path).unwrap();
    let old_lifetime = session.history_lifetime;
    let ready = offer(&mut session);
    assert_eq!(ready.status, ReloadStatus::Ready);
    assert!(ready.state.is_none());
    assert!(session.state().unwrap().unwrap().root.children.is_empty());
    let token = ready.token.unwrap();
    let result = session.apply_file_reload(&token).unwrap();
    assert_eq!(result.status, ReloadStatus::Reloaded);
    assert!(!result.state.unwrap().dirty);
    assert_eq!(
        session.state().unwrap().unwrap().root.children[0].name,
        "external"
    );
    assert_ne!(session.history_lifetime, old_lifetime);
    assert_eq!(session.password.as_deref(), Some("synthetic-key"));
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(session.apply_file_reload(&token).is_err());
    session.close();
}

#[test]
fn dirty_reload_merges_both_versions_in_memory_and_keeps_unsaved_status() {
    let (_dir, path, mut session) = fixture();
    add(&mut session, "unsaved local");
    external(&path, "external");
    let bytes = std::fs::read(&path).unwrap();
    let token = offer(&mut session).token.unwrap();
    let result = session.apply_file_reload(&token).unwrap();
    assert_eq!(result.status, ReloadStatus::Merged);
    let state = result.state.unwrap();
    assert!(state.dirty);
    assert_eq!(state.root.children.len(), 2);
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert!(check_file(&session.prepare_file_check().unwrap())
        .unwrap()
        .is_none());
    session.save().unwrap();
    session.close();
    assert_eq!(
        session
            .open(&path, "synthetic-key", None)
            .unwrap()
            .root
            .children
            .len(),
        2
    );
    session.close();
}

#[test]
fn equal_timestamp_conflicts_keep_unsaved_content_and_never_publish_a_candidate() {
    let (_dir, path, mut session) = fixture();
    add(&mut session, "original");
    session.save().unwrap();
    let id = session.state().unwrap().unwrap().root.children[0]
        .uuid
        .clone();
    let mut other = VaultSession::default();
    other.open(&path, "synthetic-key", None).unwrap();
    session.rename_group(&id, "local change").unwrap();
    other.rename_group(&id, "external change").unwrap();
    let at = chrono::DateTime::from_timestamp(1_700_000_000, 0)
        .unwrap()
        .naive_utc();
    let group = crate::vault::helpers::parse_group_id(&id).unwrap();
    session
        .db
        .as_mut()
        .unwrap()
        .group_mut(group)
        .unwrap()
        .times
        .last_modification = Some(at);
    other
        .db
        .as_mut()
        .unwrap()
        .group_mut(group)
        .unwrap()
        .times
        .last_modification = Some(at);
    other.save().unwrap();
    other.close();
    let ticket = session.prepare_file_check().unwrap();
    let changed = check_file(&ticket).unwrap().unwrap();
    let result = reload_file(session.prepare_file_reload(&ticket, changed).unwrap());
    assert_eq!(result.err().unwrap(), CONFLICT);
    assert!(session.pending_file_reload.is_none());
    assert!(session.dirty);
    assert_eq!(
        session.state().unwrap().unwrap().root.children[0].name,
        "local change"
    );
    session.close();
}

#[test]
fn damaged_or_new_key_files_never_replace_the_live_database_or_degrade_saving() {
    let (_dir, path, mut session) = fixture();
    add(&mut session, "unsaved");
    let revision = session.revision;
    for new_key in [false, true] {
        if new_key {
            let mut replacement = VaultSession::default();
            replacement
                .create(
                    &path,
                    "different-synthetic-key",
                    "Aes",
                    "Aes256",
                    "None",
                    None,
                )
                .unwrap();
            replacement.close();
        } else {
            std::fs::write(&path, b"damaged synthetic file").unwrap();
        }
        let ticket = session.prepare_file_check().unwrap();
        let changed = check_file(&ticket).unwrap().unwrap();
        assert_eq!(
            reload_file(session.prepare_file_reload(&ticket, changed).unwrap())
                .err()
                .unwrap(),
            INVALID
        );
        assert_eq!(session.revision, revision);
        assert_eq!(session.state().unwrap().unwrap().root.children.len(), 1);
        assert_eq!(session.save_failures, 0);
    }
    session.close();
}

#[test]
fn edits_replacement_expiry_and_wrong_tokens_refuse_candidates() {
    let (_dir, path, mut session) = fixture();
    external(&path, "external");
    let ticket = session.prepare_file_check().unwrap();
    let changed = check_file(&ticket).unwrap().unwrap();
    let result = reload_file(session.prepare_file_reload(&ticket, changed).unwrap()).unwrap();
    add(&mut session, "concurrent edit");
    assert!(session.stage_file_reload(ticket, result).is_err());
    let token = offer(&mut session).token.unwrap();
    assert!(session.apply_file_reload("wrong-token").is_err());
    assert!(session.pending_file_reload.is_some());
    session.pending_file_reload.as_mut().unwrap().created -= std::time::Duration::from_secs(11);
    assert!(session.apply_file_reload(&token).is_err());
    let token = offer(&mut session).token.unwrap();
    add(&mut session, "later edit");
    assert!(session.pending_file_reload.is_none());
    assert!(session.apply_file_reload(&token).is_err());
    let token = offer(&mut session).token.unwrap();
    session.close();
    assert!(session.pending_file_reload.is_none());
    session.open(&path, "synthetic-key", None).unwrap();
    assert!(session.apply_file_reload(&token).is_err());
    session.close();
}

#[test]
fn offline_remote_checks_merge_without_uploading_or_writing_the_mirror() {
    let (dir, path, mut session) = fixture();
    let storage = crate::remote::MemoryStorage::default();
    storage.seed("synthetic.kdbx", std::fs::read(&path).unwrap());
    session.close();
    session
        .open_remote(
            Arc::new(storage.clone()),
            "synthetic.kdbx",
            "synthetic-key",
            None,
            RemoteMode::SaveLocal,
            &dir.path().join("mirror"),
            2,
            "{name}.{timestamp}.{ext}.bak",
        )
        .unwrap();
    add(&mut session, "local");
    external(&path, "remote");
    let changed = std::fs::read(&path).unwrap();
    storage.seed("synthetic.kdbx", changed.clone());
    let token = offer(&mut session).token.unwrap();
    let result = session.apply_file_reload(&token);
    let result = result.unwrap();
    assert_eq!(result.status, ReloadStatus::Merged);
    assert_eq!(result.state.unwrap().root.children.len(), 2);
    assert_eq!(storage.get("synthetic.kdbx").unwrap(), changed);
    session.close();
}

#[test]
fn persistence_gate_skips_busy_operations_and_releases_the_lease() {
    let registry = VaultSessions::default();
    let permit = registry.try_acquire_persistence().unwrap();
    assert!(registry.try_acquire_persistence().is_none());
    drop(permit);
    assert!(registry.try_acquire_persistence().is_some());
}

#[tokio::test]
async fn abandoned_running_checks_keep_the_single_worker_budget_until_work_finishes() {
    let permit = try_worker().unwrap();
    let started = Arc::new(tokio::sync::Notify::new());
    let notice = started.clone();
    let (release, wait) = std::sync::mpsc::channel();
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        notice.notify_one();
        wait.recv().unwrap();
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), started.notified())
        .await
        .unwrap();
    task.abort();
    drop(task);
    assert!(try_worker().is_none());
    release.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if let Some(permit) = try_worker() {
                drop(permit);
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
