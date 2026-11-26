//! Synthetic group relocation/order tests; never access a user vault.
use super::*;
use keepass::db::{GroupId, Times, Value};
use tempfile::TempDir;

fn setup() -> (TempDir, PathBuf, VaultSession, GroupId, GroupId, GroupId) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-group-move.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-test-only", "Aes", "Aes256", "None", None)
        .unwrap();
    let db = session.require_db_mut().unwrap();
    let a = db.root_mut().add_group().id();
    let b = db.root_mut().add_group().id();
    let child = db.group_mut(a).unwrap().add_group().id();
    db.group_mut(a).unwrap().notes = Some("Synthetic note".into());
    db.group_mut(a).unwrap().enable_searching = Some(false);
    db.group_mut(child).unwrap().name = "Synthetic child".into();
    session.mark_dirty();
    session.save().unwrap();
    (dir, path, session, a, b, child)
}

#[test]
fn group_move_reorders_and_persists_without_rewriting_sibling_provenance() {
    let (_dir, path, mut session, a, b, child) = setup();
    let original = session.require_db().unwrap().clone();
    let state = session
        .move_group(&b.to_string(), ROOT_GROUP_UUID, Some(&a.to_string()))
        .unwrap();
    assert!(state.dirty);
    assert_eq!(
        state
            .root
            .children
            .iter()
            .map(|g| g.uuid.clone())
            .collect::<Vec<_>>(),
        vec![b.to_string(), a.to_string()]
    );
    let db = session.require_db().unwrap();
    assert_eq!(*db.group(a).unwrap(), *original.group(a).unwrap());
    assert_eq!(*db.group(child).unwrap(), *original.group(child).unwrap());
    assert_eq!(
        db.group(b).unwrap().previous_parent().map(|g| g.id()),
        original.group(b).unwrap().previous_parent().map(|g| g.id())
    );
    assert!(db.group(b).unwrap().times.location_changed.is_some());
    session.save().unwrap();
    session.close();
    let state = session.open(&path, "synthetic-test-only", None).unwrap();
    assert_eq!(
        state
            .root
            .children
            .iter()
            .map(|g| g.uuid.clone())
            .collect::<Vec<_>>(),
        vec![b.to_string(), a.to_string()]
    );
    session.close();
}

#[test]
fn group_move_reparents_whole_subtree_and_returns_to_root_after_reopen() {
    let (_dir, path, mut session, a, b, child) = setup();
    let db = session.require_db_mut().unwrap();
    let entry = db.group_mut(child).unwrap().add_entry().id();
    {
        let mut e = db.entry_mut(entry).unwrap();
        e.fields
            .insert("Title".into(), Value::Unprotected("Synthetic entry".into()));
        e.fields.insert(
            "Password".into(),
            Value::Protected(Box::new("synthetic-value".to_owned()).into()),
        );
        e.add_attachment(
            "synthetic.txt",
            Value::protected(b"synthetic attachment".to_vec()),
        );
        e.track_changes().fields.insert(
            "Notes".into(),
            Value::Unprotected("Synthetic change".into()),
        );
    }
    let original = session.require_db().unwrap().clone();
    session
        .move_group(&a.to_string(), &b.to_string(), None)
        .unwrap();
    let db = session.require_db().unwrap();
    assert_eq!(db.group(a).unwrap().parent().unwrap().id(), b);
    assert_eq!(
        db.group(a).unwrap().previous_parent().unwrap().id(),
        db.root().id()
    );
    assert_eq!(*db.group(child).unwrap(), *original.group(child).unwrap());
    assert_eq!(*db.entry(entry).unwrap(), *original.entry(entry).unwrap());
    assert_eq!(db.group(b).unwrap().times, original.group(b).unwrap().times);
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-test-only", None).unwrap();
    assert_eq!(
        session
            .require_db()
            .unwrap()
            .group(a)
            .unwrap()
            .parent()
            .unwrap()
            .id(),
        b
    );
    session
        .move_group(&a.to_string(), ROOT_GROUP_UUID, Some(&b.to_string()))
        .unwrap();
    session.save().unwrap();
    session.close();
    let state = session.open(&path, "synthetic-test-only", None).unwrap();
    assert_eq!(state.root.children[0].uuid, a.to_string());
    assert_eq!(
        session
            .require_db()
            .unwrap()
            .entry(entry)
            .unwrap()
            .history
            .as_ref()
            .unwrap()
            .get_entries()
            .len(),
        1
    );
    session.close();
}

#[test]
fn group_move_rejects_cycles_missing_targets_bin_and_read_only_atomically() {
    let (_dir, _path, mut session, a, b, child) = setup();
    let original = session.require_db().unwrap().clone();
    let state = serde_json::to_value(session.snapshot().unwrap()).unwrap();
    for (source, target, before) in [
        (a.to_string(), child.to_string(), None),
        (a.to_string(), a.to_string(), None),
        (ROOT_GROUP_UUID.into(), b.to_string(), None),
        (a.to_string(), "missing".into(), None),
        (a.to_string(), b.to_string(), Some(child.to_string())),
        (a.to_string(), ROOT_GROUP_UUID.into(), Some(a.to_string())),
    ] {
        assert!(session
            .move_group(&source, &target, before.as_deref())
            .is_err());
        assert_eq!(*session.require_db().unwrap(), original);
        assert_eq!(
            serde_json::to_value(session.snapshot().unwrap()).unwrap(),
            state
        );
    }
    session.require_db_mut().unwrap().meta.recyclebin_uuid = Some(b.uuid());
    assert!(session
        .move_group(&a.to_string(), &b.to_string(), None)
        .is_err());
    assert!(session
        .move_group(&b.to_string(), ROOT_GROUP_UUID, None)
        .is_err());
    while !session.is_read_only() {
        session.note_save_failure();
    }
    assert!(session
        .move_group(&a.to_string(), ROOT_GROUP_UUID, None)
        .is_err());
    session.close();
    assert!(session
        .move_group(&a.to_string(), ROOT_GROUP_UUID, None)
        .is_err());
}

#[test]
fn group_move_equivalent_order_is_clean_and_preserves_location_time() {
    let (_dir, _path, mut session, a, b, _child) = setup();
    let original = session.require_db().unwrap().clone();
    let revision = session.revision;
    let state = session
        .move_group(&a.to_string(), ROOT_GROUP_UUID, Some(&b.to_string()))
        .unwrap();
    assert!(!state.dirty);
    assert_eq!(state.revision, revision);
    assert_eq!(*session.require_db().unwrap(), original);
    // A real same-parent order change must not fabricate previous-parent provenance.
    session
        .require_db_mut()
        .unwrap()
        .group_mut(a)
        .unwrap()
        .times
        .location_changed = Some(Times::now());
    session
        .move_group(&a.to_string(), ROOT_GROUP_UUID, None)
        .unwrap();
    assert_eq!(
        session
            .require_db()
            .unwrap()
            .group(a)
            .unwrap()
            .previous_parent()
            .map(|g| g.id()),
        original.group(a).unwrap().previous_parent().map(|g| g.id())
    );
    session.close();
}
