//! Only disposable synthetic KDBX files and offline MemoryStorage are used.
use super::*;
use crate::{
    remote::{MemoryStorage, RemoteStorage},
    vault::{RemoteMode, DEFAULT_BACKUP_TEMPLATE},
};
use std::{path::PathBuf, sync::Arc};
use tempfile::TempDir;

struct Fixture {
    dir: TempDir,
    vaults: VaultSessions,
    active: Mutex<VaultSession>,
}
impl Fixture {
    fn new() -> Self {
        Self {
            dir: TempDir::new().unwrap(),
            vaults: VaultSessions::default(),
            active: Mutex::new(VaultSession::default()),
        }
    }
    fn open(&self, name: &str) -> String {
        self.vaults
            .open(&mut self.active.lock().unwrap(), |fresh| {
                fresh.create(
                    &self.path(name),
                    "synthetic-only",
                    "Aes",
                    "Aes256",
                    "None",
                    None,
                )
            })
            .unwrap()
            .session_id
    }
    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
    fn edit(&self, id: &str, name: &str) {
        self.vaults
            .with_session_mut(&mut self.active.lock().unwrap(), Some(id), |session| {
                session.require_db_mut()?.root_mut().name = name.into();
                session.mark_dirty();
                Ok(())
            })
            .unwrap();
    }
    fn ids(&self) -> Vec<String> {
        self.vaults.close_session_ids().unwrap()
    }
    fn assert_wiped(&self) {
        assert!(self.ids().is_empty());
        let session = self.active.lock().unwrap();
        assert!(
            session.db.is_none()
                && session.password.is_none()
                && session.keyfile.is_none()
                && session.remote.is_none()
        );
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self
            .vaults
            .close_all(&mut self.active.lock().unwrap(), false);
    }
}
fn disk_name(path: &std::path::Path) -> String {
    let mut session = VaultSession::default();
    session.open(path, "synthetic-only", None).unwrap();
    let name = session.require_db().unwrap().root().name.clone();
    session.close();
    name
}
async fn no_prompt(_: ClosePrompt) -> Result<CloseDecision, String> {
    panic!("clean/automatic close must not prompt")
}
async fn save(_: ClosePrompt) -> Result<CloseDecision, String> {
    Ok(CloseDecision::Save)
}
async fn discard(_: ClosePrompt) -> Result<CloseDecision, String> {
    Ok(CloseDecision::Discard)
}
fn all() -> CloseTarget {
    CloseTarget::All { keep_rpc: false }
}

#[tokio::test]
async fn clean_and_empty_workspaces_never_prompt_or_write() {
    for auto_save in [false, true] {
        let f = Fixture::new();
        f.open("clean.kdbx");
        let before = std::fs::read(f.path("clean.kdbx")).unwrap();
        close_with_writer(
            &f.vaults,
            &f.active,
            all(),
            auto_save,
            no_prompt,
            |_| async { panic!("clean vault must not be saved") },
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(f.path("clean.kdbx")).unwrap(), before);
        f.assert_wiped();
        close_with_save(&f.vaults, &f.active, all(), auto_save, no_prompt)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn cancelling_later_database_keeps_every_session_and_does_not_start_saves() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    let b = f.open("b.kdbx");
    f.edit(&a, "unsaved A");
    f.edit(&b, "unsaved B");
    let before = std::fs::read(f.path("a.kdbx")).unwrap();
    let mut prompts = 0;
    let error = close_with_writer(
        &f.vaults,
        &f.active,
        all(),
        false,
        |request| {
            prompts += 1;
            assert!(request.file_name.ends_with(".kdbx"));
            std::future::ready(Ok(if prompts == 1 {
                CloseDecision::Save
            } else {
                CloseDecision::Cancel
            }))
        },
        |_| async { panic!("cancel must precede writes") },
    )
    .await
    .err()
    .unwrap();
    assert_eq!(error, CANCELLED);
    assert_eq!(prompts, 2);
    assert_eq!(f.ids().len(), 2);
    assert_eq!(std::fs::read(f.path("a.kdbx")).unwrap(), before);
    assert!(f.active.lock().unwrap().dirty);
    assert!(!f.vaults.closing.load(Ordering::Acquire));
}

#[tokio::test]
async fn explicit_save_of_parked_tab_preserves_active_tab_and_reopens_saved_kdbx() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    let b = f.open("b.kdbx");
    f.edit(&a, "saved A");
    f.edit(&b, "unsaved B");
    let closed = close_with_save(
        &f.vaults,
        &f.active,
        CloseTarget::One(Some(a.clone())),
        false,
        save,
    )
    .await
    .unwrap();
    assert_eq!(closed.ids, vec![a]);
    assert!(closed.any_open);
    assert_eq!(f.ids(), vec![b.clone()]);
    assert_eq!(f.vaults.active_id().as_deref(), Some(b.as_str()));
    assert!(f.active.lock().unwrap().dirty);
    assert_eq!(disk_name(&f.path("a.kdbx")), "saved A");
}

#[tokio::test]
async fn explicit_discard_closes_without_changing_disk() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    f.edit(&a, "discarded");
    let before = std::fs::read(f.path("a.kdbx")).unwrap();
    close_with_save(&f.vaults, &f.active, all(), false, discard)
        .await
        .unwrap();
    assert_eq!(std::fs::read(f.path("a.kdbx")).unwrap(), before);
    f.assert_wiped();
}

#[tokio::test]
async fn automatic_save_persists_every_database_before_wiping_secrets() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    let b = f.open("b.kdbx");
    f.edit(&a, "saved A");
    f.edit(&b, "saved B");
    close_with_save(&f.vaults, &f.active, all(), true, no_prompt)
        .await
        .unwrap();
    f.assert_wiped();
    assert_eq!(disk_name(&f.path("a.kdbx")), "saved A");
    assert_eq!(disk_name(&f.path("b.kdbx")), "saved B");
}

#[tokio::test]
async fn actual_local_write_failures_keep_dirty_sessions_and_reach_read_only() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    f.edit(&a, "retained");
    // Existing directory is an invalid file destination, without touching the
    // originally persisted KDBX or relying on host-specific permission flags.
    f.active.lock().unwrap().path = Some(f.dir.path().to_string_lossy().into_owned());
    for expected in 1..=3 {
        assert!(
            close_with_save(&f.vaults, &f.active, all(), true, no_prompt)
                .await
                .is_err()
        );
        let session = f.active.lock().unwrap();
        assert_eq!(session.save_failures, expected);
        assert!(session.dirty && session.password.is_some());
        assert_eq!(session.is_read_only(), expected == 3);
    }
    assert!(
        close_with_save(&f.vaults, &f.active, all(), true, no_prompt)
            .await
            .err()
            .unwrap()
            .contains("只读")
    );
    assert_eq!(f.active.lock().unwrap().save_failures, 3);
    close_with_save(&f.vaults, &f.active, all(), false, discard)
        .await
        .unwrap();
    f.assert_wiped();
}

#[tokio::test]
async fn later_save_failure_leaves_all_sessions_open_and_earlier_save_durable() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    let b = f.open("b.kdbx");
    f.edit(&a, "saved A");
    f.edit(&b, "retained B");
    f.active.lock().unwrap().path = Some(f.dir.path().to_string_lossy().into_owned());
    assert!(
        close_with_save(&f.vaults, &f.active, all(), true, no_prompt)
            .await
            .is_err()
    );
    assert_eq!(f.ids().len(), 2);
    assert_eq!(disk_name(&f.path("a.kdbx")), "saved A");
    f.vaults
        .with_session_mut(&mut f.active.lock().unwrap(), Some(&a), |session| {
            assert!(!session.dirty);
            Ok(())
        })
        .unwrap();
    assert!(f.active.lock().unwrap().dirty);
}

#[tokio::test]
async fn tab_switch_during_save_never_redirects_completion_or_close() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    let b = f.open("b.kdbx");
    f.edit(&a, "saved A");
    close_with_writer(
        &f.vaults,
        &f.active,
        CloseTarget::One(Some(a.clone())),
        true,
        no_prompt,
        |job| async {
            assert!(f.active.try_lock().is_ok());
            f.vaults
                .switch_active(&mut f.active.lock().unwrap(), &a)
                .unwrap();
            persist_save(job)
        },
    )
    .await
    .unwrap();
    assert_eq!(f.ids(), vec![b]);
    assert_eq!(disk_name(&f.path("a.kdbx")), "saved A");
}

#[tokio::test]
async fn concurrent_edit_during_save_is_retained_and_aborts_close() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    f.edit(&a, "saved version");
    let result = close_with_writer(&f.vaults, &f.active, all(), true, no_prompt, |job| async {
        f.edit(&a, "late version");
        persist_save(job)
    })
    .await;
    assert_eq!(result.err().unwrap(), CHANGED);
    assert_eq!(f.ids(), vec![a]);
    assert_eq!(disk_name(&f.path("a.kdbx")), "saved version");
    assert!(f.active.lock().unwrap().dirty);
    assert_eq!(
        f.active.lock().unwrap().require_db().unwrap().root().name,
        "late version"
    );
}

#[tokio::test]
async fn new_tab_during_save_aborts_whole_workspace_close() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    f.edit(&a, "saved version");
    let result = close_with_writer(&f.vaults, &f.active, all(), true, no_prompt, |job| async {
        f.open("new.kdbx");
        persist_save(job)
    })
    .await;
    assert_eq!(result.err().unwrap(), CHANGED);
    assert_eq!(f.ids().len(), 2);
}

#[tokio::test]
async fn replaced_lifetime_never_adopts_previous_save_hash_or_status() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    f.edit(&a, "saved version");
    let result = close_with_writer(&f.vaults, &f.active, all(), true, no_prompt, |job| async {
        let outcome = persist_save(job)?;
        f.active
            .lock()
            .unwrap()
            .open(&f.path("a.kdbx"), "synthetic-only", None)
            .unwrap();
        f.active.lock().unwrap().save_failures = 2;
        Ok(outcome)
    })
    .await;
    assert_eq!(result.err().unwrap(), CHANGED);
    assert_eq!(f.ids(), vec![a]);
    assert_eq!(f.active.lock().unwrap().save_failures, 2);
}

#[tokio::test]
async fn edits_during_discard_prompt_and_clean_tab_edits_abort_without_wiping() {
    for clean_target in [false, true] {
        let f = Fixture::new();
        let a = f.open("a.kdbx");
        let b = f.open("b.kdbx");
        f.edit(&a, "initial");
        let result = close_with_save(&f.vaults, &f.active, all(), false, |_| {
            f.edit(if clean_target { &b } else { &a }, "late");
            std::future::ready(Ok(CloseDecision::Discard))
        })
        .await;
        assert_eq!(result.err().unwrap(), CHANGED);
        assert_eq!(f.ids().len(), 2);
    }
}

#[tokio::test]
async fn concurrent_close_attempt_is_rejected_and_cancel_releases_permit() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    f.edit(&a, "initial");
    let result = close_with_save(&f.vaults, &f.active, all(), false, |_| async {
        let concurrent = close_with_save(&f.vaults, &f.active, all(), true, no_prompt).await;
        assert_eq!(concurrent.err().unwrap(), BUSY);
        Ok(CloseDecision::Cancel)
    })
    .await;
    assert_eq!(result.err().unwrap(), CANCELLED);
    close_with_save(&f.vaults, &f.active, all(), false, discard)
        .await
        .unwrap();
    f.assert_wiped();
}

#[tokio::test]
async fn aborted_prompt_future_releases_close_permit_and_keeps_session() {
    let f = Fixture::new();
    let a = f.open("a.kdbx");
    f.edit(&a, "initial");
    {
        let attempt = close_with_save(&f.vaults, &f.active, all(), false, |_| {
            std::future::pending::<Result<CloseDecision, String>>()
        });
        tokio::pin!(attempt);
        tokio::select! { _ = &mut attempt => panic!("prompt unexpectedly returned"), _ = tokio::task::yield_now() => {} }
        assert!(f.vaults.closing.load(Ordering::Acquire));
    }
    assert!(!f.vaults.closing.load(Ordering::Acquire));
    assert_eq!(f.ids(), vec![a]);
    close_with_save(&f.vaults, &f.active, all(), false, discard)
        .await
        .unwrap();
    f.assert_wiped();
}

#[tokio::test]
async fn offline_remote_save_reopens_and_conflict_keeps_unsaved_changes() {
    for conflict in [false, true] {
        let f = Fixture::new();
        let storage = Arc::new(MemoryStorage::default());
        let a = f
            .vaults
            .open(&mut f.active.lock().unwrap(), |fresh| {
                fresh.create_remote(
                    storage.clone(),
                    "synthetic.kdbx",
                    "synthetic-only",
                    "Aes",
                    "Aes256",
                    "None",
                    None,
                    RemoteMode::InMemory,
                    f.dir.path(),
                    0,
                    DEFAULT_BACKUP_TEMPLATE,
                )
            })
            .unwrap()
            .session_id;
        f.edit(&a, "remote saved");
        if conflict {
            storage.seed("synthetic.kdbx", vec![1, 2, 3]);
        }
        let result = close_with_save(&f.vaults, &f.active, all(), true, no_prompt).await;
        if conflict {
            assert!(result
                .err()
                .unwrap()
                .starts_with(crate::remote::REMOTE_CONFLICT_MARKER));
            assert_eq!(f.ids(), vec![a]);
            assert!(f.active.lock().unwrap().dirty);
            assert_eq!(f.active.lock().unwrap().save_failures, 0);
            assert_eq!(storage.get("synthetic.kdbx").unwrap(), vec![1, 2, 3]);
        } else {
            result.unwrap();
            f.assert_wiped();
            let mut reopened = VaultSession::default();
            reopened
                .open_remote(
                    storage,
                    "synthetic.kdbx",
                    "synthetic-only",
                    None,
                    RemoteMode::InMemory,
                    f.dir.path(),
                    0,
                    DEFAULT_BACKUP_TEMPLATE,
                )
                .unwrap();
            assert_eq!(reopened.require_db().unwrap().root().name, "remote saved");
            reopened.close();
        }
    }
}

#[tokio::test]
async fn authoritative_remote_success_with_failed_optional_mirror_still_closes() {
    let f = Fixture::new();
    let storage = Arc::new(MemoryStorage::default());
    let id = f
        .vaults
        .open(&mut f.active.lock().unwrap(), |fresh| {
            fresh.create_remote(
                storage.clone(),
                "synthetic.kdbx",
                "synthetic-only",
                "Aes",
                "Aes256",
                "None",
                None,
                RemoteMode::InMemory,
                f.dir.path(),
                0,
                DEFAULT_BACKUP_TEMPLATE,
            )
        })
        .unwrap()
        .session_id;
    f.edit(&id, "remote durable");
    std::fs::write(f.path("mirror-blocker"), [0]).unwrap();
    {
        let mut session = f.active.lock().unwrap();
        let remote = session.remote.as_mut().unwrap();
        remote.mode = RemoteMode::SaveLocal;
        remote.local_dir = f.path("mirror-blocker");
    }
    close_with_save(&f.vaults, &f.active, all(), true, no_prompt)
        .await
        .unwrap();
    f.assert_wiped();
    let mut reopened = VaultSession::default();
    reopened
        .open_remote(
            storage,
            "synthetic.kdbx",
            "synthetic-only",
            None,
            RemoteMode::InMemory,
            f.dir.path(),
            0,
            DEFAULT_BACKUP_TEMPLATE,
        )
        .unwrap();
    assert_eq!(reopened.require_db().unwrap().root().name, "remote durable");
    reopened.close();
}

#[tokio::test]
async fn prior_save_completing_while_close_waits_is_not_saved_twice() {
    let f = Fixture::new();
    let id = f.open("a.kdbx");
    f.edit(&id, "prior save");
    let permit = f.vaults.acquire_persistence().unwrap();
    let job = f.active.lock().unwrap().prepare_save(false).unwrap();
    let revision = job.revision;
    let close = close_with_writer(&f.vaults, &f.active, all(), true, no_prompt, |_| async {
        panic!("the prior save already persisted this revision")
    });
    let finish_prior = async {
        tokio::task::yield_now().await;
        let outcome = persist_save(job).unwrap();
        f.active
            .lock()
            .unwrap()
            .complete_save(revision, outcome)
            .unwrap();
        drop(permit);
    };
    let (closed, ()) = tokio::join!(close, finish_prior);
    closed.unwrap();
    f.assert_wiped();
    assert_eq!(disk_name(&f.path("a.kdbx")), "prior save");
}

#[tokio::test]
async fn dirty_keyfile_vault_saves_and_lock_retains_only_opted_in_rpc_keys() {
    for keep_rpc in [false, true] {
        let f = Fixture::new();
        let keyfile = f.path("synthetic.key");
        std::fs::write(&keyfile, [7; 32]).unwrap();
        let id = f
            .vaults
            .open(&mut f.active.lock().unwrap(), |fresh| {
                fresh.create(
                    &f.path("a.kdbx"),
                    "synthetic-only",
                    "Aes",
                    "Aes256",
                    "None",
                    Some(&keyfile),
                )
            })
            .unwrap()
            .session_id;
        f.edit(&id, "keyfile saved");
        {
            let mut session = f.active.lock().unwrap();
            session.rpc_keys.insert("synthetic-rpc".into(), vec![3; 32]);
            session
                .bridge_keys
                .insert("synthetic-bridge".into(), vec![4; 32]);
        }
        close_with_save(
            &f.vaults,
            &f.active,
            CloseTarget::All { keep_rpc },
            true,
            no_prompt,
        )
        .await
        .unwrap();
        {
            let session = f.active.lock().unwrap();
            assert!(
                session.db.is_none() && session.password.is_none() && session.keyfile.is_none()
            );
            assert!(session.bridge_keys.is_empty());
            assert_eq!(!session.rpc_keys.is_empty(), keep_rpc);
        }
        let mut reopened = VaultSession::default();
        reopened
            .open(&f.path("a.kdbx"), "synthetic-only", Some(&keyfile))
            .unwrap();
        assert_eq!(reopened.require_db().unwrap().root().name, "keyfile saved");
        reopened.close();
    }
}
