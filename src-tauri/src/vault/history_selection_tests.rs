use super::*;
use tempfile::TempDir;

// Legacy native fixtures select and mutate synchronously. Production callers
// must instead retain the actual token returned with their history list.
impl VaultSession {
    pub(crate) fn restore_history_fixture(
        &mut self,
        uuid: &str,
        index: usize,
    ) -> Result<VaultState, String> {
        let id = helpers::parse_entry_id(uuid)?;
        let token = self.history_selection_token(id, index);
        self.restore_entry_version(uuid, index, &token)
    }

    pub(crate) fn delete_history_fixture(
        &mut self,
        uuid: &str,
        index: usize,
    ) -> Result<VaultState, String> {
        let id = helpers::parse_entry_id(uuid)?;
        let token = self.history_selection_token(id, index);
        self.delete_entry_history(uuid, index, &token)
    }
}

fn fixture() -> (TempDir, VaultSession, String) {
    let dir = TempDir::new().unwrap();
    let mut session = VaultSession::default();
    session
        .create(
            &dir.path().join("synthetic.kdbx"),
            "synthetic-master",
            "Aes",
            "Aes256",
            "None",
            None,
        )
        .unwrap();
    let input: EntryInput = serde_json::from_value(serde_json::json!({
        "groupUuid":"root", "title":"A", "username":"", "password":"",
        "url":"", "notes":""
    }))
    .unwrap();
    let uuid = session.add_entry(&input).unwrap().root.entries[0]
        .uuid
        .clone();
    rename(&mut session, &uuid, "B");
    rename(&mut session, &uuid, "C");
    (dir, session, uuid)
}

fn rename(session: &mut VaultSession, uuid: &str, title: &str) {
    session
        .update_entries(
            &[uuid.to_owned()],
            &EntryPatch {
                title: Some(title.to_owned()),
                ..Default::default()
            },
        )
        .unwrap();
}

#[test]
fn history_selection_restore_after_insert_rejects_without_mutation() {
    let (_dir, mut session, uuid) = fixture();
    let selected = session.get_entry_history(&uuid).unwrap()[1].clone();
    assert_eq!(selected.title, "A");
    rename(&mut session, &uuid, "D");
    let before = serde_json::to_value(session.state().unwrap()).unwrap();
    let history_before = serde_json::to_value(session.get_entry_history(&uuid).unwrap()).unwrap();
    let error = session
        .restore_entry_version(&uuid, selected.index, &selected.selection_token)
        .unwrap_err();
    assert!(error.starts_with("HISTORY_SELECTION_STALE:"));
    assert_eq!(
        serde_json::to_value(session.state().unwrap()).unwrap(),
        before
    );
    assert_eq!(
        serde_json::to_value(session.get_entry_history(&uuid).unwrap()).unwrap(),
        history_before
    );
    let refreshed = session.get_entry_history(&uuid).unwrap()[2].clone();
    let state = session
        .restore_entry_version(&uuid, refreshed.index, &refreshed.selection_token)
        .unwrap();
    assert_eq!(state.root.entries[0].title, "A");
    assert!(session
        .delete_entry_history(&uuid, refreshed.index, &refreshed.selection_token)
        .is_err());
}

#[test]
fn history_selection_delete_after_removal_rejects_without_mutation() {
    let (_dir, mut session, uuid) = fixture();
    rename(&mut session, &uuid, "D");
    let selected = session.get_entry_history(&uuid).unwrap()[1].clone();
    assert_eq!(selected.title, "B");
    session.delete_history_fixture(&uuid, 0).unwrap();
    let before = serde_json::to_value(session.state().unwrap()).unwrap();
    let history_before = serde_json::to_value(session.get_entry_history(&uuid).unwrap()).unwrap();
    let error = session
        .delete_entry_history(&uuid, selected.index, &selected.selection_token)
        .unwrap_err();
    assert!(error.starts_with("HISTORY_SELECTION_STALE:"));
    assert_eq!(
        serde_json::to_value(session.state().unwrap()).unwrap(),
        before
    );
    assert_eq!(
        serde_json::to_value(session.get_entry_history(&uuid).unwrap()).unwrap(),
        history_before
    );
    let refreshed = session.get_entry_history(&uuid).unwrap()[0].clone();
    session
        .delete_entry_history(&uuid, refreshed.index, &refreshed.selection_token)
        .unwrap();
    let remaining = session.get_entry_history(&uuid).unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].title, "A");
}

#[test]
fn history_selection_reopen_same_path_uuid_and_revision_rejects_old_token() {
    let (dir, mut session, uuid) = fixture();
    session.save().unwrap();
    let selected = session.get_entry_history(&uuid).unwrap()[0].clone();
    let revision = session.revision;
    let path = dir.path().join("synthetic.kdbx");
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    assert_eq!(session.revision, revision);
    let fresh = session.get_entry_history(&uuid).unwrap()[0].clone();
    assert_eq!(selected.index, fresh.index);
    assert_eq!(selected.modified, fresh.modified);
    assert_eq!(selected.title, fresh.title);
    assert_ne!(selected.selection_token, fresh.selection_token);
    let before = serde_json::to_value(session.state().unwrap()).unwrap();
    for restore in [true, false] {
        let result = if restore {
            session.restore_entry_version(&uuid, selected.index, &selected.selection_token)
        } else {
            session.delete_entry_history(&uuid, selected.index, &selected.selection_token)
        };
        assert!(result.unwrap_err().starts_with("HISTORY_SELECTION_STALE:"));
        assert_eq!(
            serde_json::to_value(session.state().unwrap()).unwrap(),
            before
        );
    }
    session
        .restore_entry_version(&uuid, fresh.index, &fresh.selection_token)
        .unwrap();
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    assert_eq!(session.state().unwrap().unwrap().root.entries[0].title, "B");
}

#[test]
fn history_selection_cross_session_index_and_entry_do_not_match() {
    let (dir, mut session, uuid) = fixture();
    session.save().unwrap();
    let selected = session.get_entry_history(&uuid).unwrap()[0].clone();
    let mut other = VaultSession::default();
    other
        .open(&dir.path().join("synthetic.kdbx"), "synthetic-master", None)
        .unwrap();
    // Explicitly equalize revision to rule out a revision-only guard.
    other.revision = session.revision;
    let before = serde_json::to_value(other.state().unwrap()).unwrap();
    assert!(other
        .restore_entry_version(&uuid, selected.index, &selected.selection_token)
        .is_err());
    assert!(other
        .delete_entry_history(&uuid, selected.index, &selected.selection_token)
        .is_err());
    assert_eq!(
        serde_json::to_value(other.state().unwrap()).unwrap(),
        before
    );
    let selected = other.get_entry_history(&uuid).unwrap()[0].clone();
    assert!(other
        .delete_entry_history(&uuid, 1, &selected.selection_token)
        .is_err());
    let foreign_uuid = uuid::Uuid::new_v4().to_string();
    assert!(other
        .restore_entry_version(&foreign_uuid, 0, &selected.selection_token)
        .is_err());
    assert!(other.restore_entry_version(&uuid, 0, "").is_err());
    assert_eq!(
        serde_json::to_value(other.state().unwrap()).unwrap(),
        before
    );
    // Saving without edits does not change the selected version's identity.
    other.save().unwrap();
    other
        .delete_entry_history(&uuid, selected.index, &selected.selection_token)
        .unwrap();
}

#[test]
fn history_selection_remote_replacements_invalidate_the_old_expectation() {
    for merge in [false, true] {
        let (_dir, mut session, uuid) = fixture();
        let selected = session.get_entry_history(&uuid).unwrap()[0].clone();
        let db = session.require_db().unwrap().clone();
        let revision = session.revision;
        // Exercise production adoption without network or provider access.
        if merge {
            session
                .complete_remote_merge(
                    revision,
                    persist::RemoteMergeResult {
                        db,
                        base_hash: [0; 32],
                        mirror_warning: None,
                    },
                )
                .unwrap();
        } else {
            session
                .complete_remote_refresh(
                    revision,
                    persist::RemoteRefreshResult {
                        db,
                        base_hash: [0; 32],
                    },
                )
                .unwrap();
        }
        let before = serde_json::to_value(session.state().unwrap()).unwrap();
        assert!(session
            .restore_entry_version(&uuid, selected.index, &selected.selection_token)
            .is_err());
        assert!(session
            .delete_entry_history(&uuid, selected.index, &selected.selection_token)
            .is_err());
        assert_eq!(
            serde_json::to_value(session.state().unwrap()).unwrap(),
            before
        );
    }
}

#[test]
fn history_selection_registry_routes_parked_owner_and_rejects_other_or_closed_owner() {
    use crate::commands::with_vault_session;
    use std::sync::Mutex;
    let (dir, mut source, uuid) = fixture();
    source.save().unwrap();
    let path = dir.path().join("synthetic.kdbx");
    let active = Mutex::new(VaultSession::default());
    let vaults = VaultSessions::default();
    let a = vaults
        .open(&mut active.lock().unwrap(), |target| {
            target.open(&path, "synthetic-master", None)
        })
        .unwrap()
        .session_id;
    let b = vaults
        .open(&mut active.lock().unwrap(), |target| {
            target.open(&path, "synthetic-master", None)
        })
        .unwrap()
        .session_id;
    let selected = with_vault_session(&vaults, &active, Some(&a), |target| {
        target.get_entry_history(&uuid)
    })
    .unwrap()[0]
        .clone();
    let before_b = serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap();
    assert!(
        with_vault_session(&vaults, &active, Some(&b), |target| target
            .delete_entry_history(&uuid, selected.index, &selected.selection_token))
        .is_err()
    );
    let state_a = with_vault_session(&vaults, &active, Some(&a), |target| {
        target.restore_entry_version(&uuid, selected.index, &selected.selection_token)
    })
    .unwrap();
    assert_eq!(state_a.root.entries[0].title, "B");
    assert_eq!(
        serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap(),
        before_b
    );
    vaults.close(&mut active.lock().unwrap(), Some(&a)).unwrap();
    assert!(
        with_vault_session(&vaults, &active, Some(&a), |target| target
            .restore_entry_version(&uuid, selected.index, &selected.selection_token))
        .is_err()
    );
    assert_eq!(
        serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap(),
        before_b
    );
}
