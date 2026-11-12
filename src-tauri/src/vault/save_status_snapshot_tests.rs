//! Native persistence-status regressions; all files and values are synthetic.
use super::*;
use tempfile::TempDir;

const SYNTHETIC_PASSWORD: &str = "synthetic-status-only";

fn fixture() -> (TempDir, VaultSession, PathBuf) {
    let dir = TempDir::new().unwrap();
    let parent = dir.path().join("owned");
    std::fs::create_dir(&parent).unwrap();
    let path = parent.join("synthetic.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, SYNTHETIC_PASSWORD, "Aes", "Aes256", "None", None)
        .unwrap();
    (dir, session, path)
}

fn edit(session: &mut VaultSession) {
    session
        .add_group(&GroupInput {
            parent_uuid: Some(ROOT_GROUP_UUID.into()),
            icon: None,
            name: "Synthetic changed group".into(),
        })
        .unwrap();
}

fn assert_status_published_later(before: &VaultState, after: &VaultState) {
    assert_eq!(
        after.revision, before.revision,
        "status must not invalidate editing/history identity"
    );
    assert!(
        after.status_revision > before.status_revision,
        "persistence status must have newer publication identity"
    );
}

#[test]
fn native_status_clean_save_orders_after_dirty_snapshot() {
    let (_dir, mut session, path) = fixture();
    edit(&mut session);
    let before = session.state().unwrap().unwrap();
    let after = session.save().unwrap();
    assert!(before.dirty && !after.dirty);
    assert_status_published_later(&before, &after);
    let mut reopened = VaultSession::default();
    assert_eq!(
        reopened
            .open(&path, SYNTHETIC_PASSWORD, None)
            .unwrap()
            .root
            .children
            .len(),
        1
    );
    reopened.close();
    session.close();
}

#[test]
fn native_status_real_failures_order_read_only_after_writable_snapshot() {
    let (dir, mut session, path) = fixture();
    let before = session.state().unwrap().unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let moved = dir.path().join("moved");
    // Move only this test-owned parent; the existing KDBX is preserved.
    std::fs::rename(path.parent().unwrap(), &moved).unwrap();
    for _ in 0..3 {
        assert!(session.save().is_err());
    }
    let after = session.state().unwrap().unwrap();
    assert!(!before.read_only && after.read_only);
    assert!(session.save().unwrap_err().contains("只读模式"));
    assert_eq!(std::fs::read(moved.join("synthetic.kdbx")).unwrap(), bytes);
    assert_status_published_later(&before, &after);
    session.close();
}

#[test]
fn native_status_concurrent_edit_and_warning_clearance_are_ordered() {
    let (_dir, mut session, path) = fixture();
    let job = session.prepare_save(false).unwrap();
    let captured = job.revision;
    edit(&mut session);
    // The warning boundary is injected; persistence and KDBX are real.
    session.mirror_warning = Some("Synthetic previous warning".into());
    let before = session.state().unwrap().unwrap();
    let outcome = persist_save(job).unwrap();
    let after = session.complete_save(captured, outcome).unwrap();
    assert!(before.dirty && after.dirty);
    assert_eq!(before.modified_at, after.modified_at);
    assert!(before.mirror_warning.is_some() && after.mirror_warning.is_none());
    assert_eq!(after.root.children.len(), 1);
    assert_status_published_later(&before, &after);
    session.save().unwrap();
    session.close();
    let mut reopened = VaultSession::default();
    assert_eq!(
        reopened
            .open(&path, SYNTHETIC_PASSWORD, None)
            .unwrap()
            .root
            .children
            .len(),
        1
    );
    reopened.close();
}

#[test]
fn native_status_master_change_orders_clean_completion() {
    let (_dir, mut session, path) = fixture();
    edit(&mut session);
    let before = session.state().unwrap().unwrap();
    let after = session
        .change_master_key("synthetic-new-status-only", None)
        .unwrap();
    assert!(before.dirty && !after.dirty);
    assert_status_published_later(&before, &after);
    session.close();
    let mut reopened = VaultSession::default();
    assert!(reopened.open(&path, SYNTHETIC_PASSWORD, None).is_err());
    assert_eq!(
        reopened
            .open(&path, "synthetic-new-status-only", None)
            .unwrap()
            .root
            .children
            .len(),
        1
    );
    reopened.close();
}

#[test]
fn native_status_ordinary_save_retains_valid_history_selection() {
    let (_dir, mut session, _) = fixture();
    let input: EntryInput = serde_json::from_value(serde_json::json!({
        "groupUuid": "root", "title": "Synthetic A", "username": "", "password": "", "url": "", "notes": ""
    })).unwrap();
    let id = session.add_entry(&input).unwrap().root.entries[0]
        .uuid
        .clone();
    session
        .update_entries(
            std::slice::from_ref(&id),
            &EntryPatch {
                title: Some("Synthetic B".into()),
                ..Default::default()
            },
        )
        .unwrap();
    let selected = session.get_entry_history(&id).unwrap()[0].clone();
    session.save().unwrap();
    let restored = session
        .restore_entry_version(&id, selected.index, &selected.selection_token)
        .unwrap();
    assert_eq!(restored.root.entries[0].title, "Synthetic A");
    session.close();
}

#[test]
fn native_status_refused_write_preserves_state_and_existing_bytes() {
    let (_dir, mut session, path) = fixture();
    let bytes = std::fs::read(path).unwrap();
    for _ in 0..3 {
        session.note_save_failure();
    }
    let before = serde_json::to_value(session.state().unwrap().unwrap()).unwrap();
    assert!(session.save().unwrap_err().contains("只读模式"));
    assert!(session
        .change_master_key("synthetic-refused-only", None)
        .unwrap_err()
        .contains("只读模式"));
    let after = serde_json::to_value(session.state().unwrap().unwrap()).unwrap();
    assert_eq!(before, after);
    assert_eq!(
        std::fs::read(session.path.as_ref().unwrap()).unwrap(),
        bytes
    );
    session.close();
}

#[test]
fn native_status_repeated_reads_preserve_snapshot_identity() {
    let (_dir, mut session, _) = fixture();
    let before = serde_json::to_value(session.state().unwrap().unwrap()).unwrap();
    let after = serde_json::to_value(session.state().unwrap().unwrap()).unwrap();
    assert_eq!(before, after);
    session.close();
}

#[test]
fn native_status_delta_wire_matches_full_snapshot_after_save_and_read_only() {
    let (_dir, mut session, _) = fixture();
    edit(&mut session);
    let group = session.state().unwrap().unwrap().root.children[0]
        .uuid
        .clone();
    let input: EntryInput = serde_json::from_value(serde_json::json!({
        "groupUuid": "root", "title": "Synthetic delta", "username": "", "password": "", "url": "", "notes": ""
    })).unwrap();
    let id = session.add_entry(&input).unwrap().root.entries[0]
        .uuid
        .clone();
    session.save().unwrap();
    for phase in 0..3 {
        let delta = match phase {
            0 => session
                .set_groups_expanded_delta(std::slice::from_ref(&group), false)
                .unwrap(),
            1 => {
                for _ in 0..3 {
                    session.note_save_failure();
                }
                session.set_groups_expanded_delta(&[], false).unwrap()
            }
            _ => session.toggle_favorite_delta(&id).unwrap(),
        };
        let delta = serde_json::to_value(delta).unwrap();
        let state = serde_json::to_value(session.state().unwrap().unwrap()).unwrap();
        for key in [
            "revision",
            "statusRevision",
            "dirty",
            "readOnly",
            "mirrorWarning",
            "modifiedAt",
        ] {
            assert_eq!(
                delta.get(key),
                state.get(key),
                "delta status differs at {key}, phase {phase}"
            );
        }
        assert!(delta.get("status_revision").is_none());
        assert_eq!(delta["readOnly"], phase > 0);
        assert_eq!(delta["dirty"], phase == 2);
    }
    session.close();
}
