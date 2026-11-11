use super::*;
use keepass::db::{AutoType, AutoTypeAssociation, DataTransferObfuscation, EntryId};
use serde_json::{json, Value};

fn content(title: &str) -> Value {
    json!({"groupUuid":"root", "title":title, "username":"", "password":"",
        "url":"", "notes":""})
}

fn old_time() -> chrono::NaiveDateTime {
    chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
}

fn config(flag: &DataTransferObfuscation, changed: bool) -> AutoType {
    AutoType {
        enabled: !changed,
        default_sequence: Some(if changed { "{TITLE}" } else { "{USERNAME}" }.into()),
        data_transfer_obfuscation: flag.clone(),
        associations: vec![AutoTypeAssociation {
            window: if changed {
                "Synthetic changed *"
            } else {
                "Synthetic original *"
            }
            .into(),
            sequence: if changed {
                "{TITLE}{ENTER}"
            } else {
                "{USERNAME}{TAB}"
            }
            .into(),
        }],
    }
}

fn visible(config: &AutoType) -> Value {
    json!({"enabled":config.enabled, "defaultSequence":config.default_sequence,
        "associations":config.associations.iter().map(|a|json!({"window":a.window,"sequence":a.sequence})).collect::<Vec<_>>()})
}

fn fixture(flag: Option<DataTransferObfuscation>) -> (tempfile::TempDir, VaultSession, EntryId) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic-retention.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let state = session
        .add_entry(&serde_json::from_value(content("Before")).unwrap())
        .unwrap();
    let id = helpers::parse_entry_id(&state.root.entries[0].uuid).unwrap();
    {
        let mut entry = session.require_db_mut().unwrap().entry_mut(id).unwrap();
        entry.autotype = flag.as_ref().map(|flag| config(flag, false));
        entry.times.last_modification = Some(old_time());
    }
    session.mark_dirty();
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    let entry = session.require_db().unwrap().entry(id).unwrap();
    assert!(entry.autotype == flag.as_ref().map(|flag| config(flag, false)));
    assert_eq!(entry.times.last_modification, Some(old_time()));
    assert!(entry
        .history
        .as_ref()
        .is_none_or(|history| history.get_entries().is_empty()));
    (dir, session, id)
}

fn reopen(dir: &tempfile::TempDir, session: &mut VaultSession) {
    session.save().unwrap();
    session.close();
    session
        .open(
            &dir.path().join("synthetic-retention.kdbx"),
            "synthetic-master",
            None,
        )
        .unwrap();
}

#[test]
fn entry_autotype_retention_standalone_same_and_changed_keep_hidden_option() {
    let mut failures = Vec::new();
    for (flag_index, flag) in [
        DataTransferObfuscation::None,
        DataTransferObfuscation::UseClipboard,
    ]
    .into_iter()
    .enumerate()
    {
        for changed in [false, true] {
            let (dir, mut session, id) = fixture(Some(flag.clone()));
            let expected = config(&flag, changed);
            let input = serde_json::from_value(visible(&expected)).unwrap();
            session
                .update_entry_autotype(&id.to_string(), &input)
                .unwrap();
            for reopened in [false, true] {
                if reopened {
                    reopen(&dir, &mut session);
                }
                let entry = session.require_db().unwrap().entry(id).unwrap();
                let actual = entry.autotype.as_ref().unwrap();
                assert!(visible(actual) == visible(&expected));
                assert!(entry
                    .history
                    .as_ref()
                    .is_none_or(|history| history.get_entries().is_empty())); // Standalone API retains its existing history policy.
                if actual != &expected
                    || (entry.times.last_modification == Some(old_time())) == changed
                {
                    failures.push((flag_index, changed, reopened));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "configuration or time changed unexpectedly: {failures:?}"
    );
}

#[test]
fn entry_autotype_retention_editor_save_keeps_current_and_historical_option() {
    let mut failures = Vec::new();
    for (flag_index, flag) in [
        DataTransferObfuscation::None,
        DataTransferObfuscation::UseClipboard,
    ]
    .into_iter()
    .enumerate()
    {
        for changed in [false, true] {
            let (dir, mut session, id) = fixture(Some(flag.clone()));
            let expected = config(&flag, changed);
            let request: EntrySaveRequest = serde_json::from_value(json!({
                "target":{"mode":"edit","uuid":id.to_string()},"input":content("After"),"autoType":visible(&expected)
            })).unwrap();
            let revision = session.revision;
            let result = session.save_entry(&request).unwrap();
            assert_eq!(result.uuid, id.to_string());
            assert_eq!(session.revision, revision + 1);
            assert!(session.dirty);
            for reopened in [false, true] {
                if reopened {
                    reopen(&dir, &mut session);
                }
                let entry = session.require_db().unwrap().entry(id).unwrap();
                assert_eq!(entry.get_title(), Some("After"));
                assert!(entry.times.last_modification > Some(old_time()));
                assert!(visible(entry.autotype.as_ref().unwrap()) == visible(&expected));
                assert_eq!(entry.history.as_ref().unwrap().get_entries().len(), 1);
                let old = entry.historical(0).unwrap();
                assert_eq!(old.get_title(), Some("Before"));
                assert!(old.autotype == Some(config(&flag, false)));
                assert_eq!(old.times.last_modification, Some(old_time()));
                if entry.autotype.as_ref() != Some(&expected) {
                    failures.push((flag_index, changed, reopened));
                }
            }
        }
    }
    assert!(
        failures.is_empty(),
        "editor lost hidden configuration: {failures:?}"
    );
}

#[test]
fn entry_autotype_retention_omission_and_new_configuration_keep_defaults() {
    for flag in [None, Some(DataTransferObfuscation::UseClipboard)] {
        let (dir, mut session, id) = fixture(flag.clone());
        for auto_type in [None, Some(Value::Null)] {
            let mut body =
                json!({"target":{"mode":"edit","uuid":id.to_string()},"input":content("After")});
            if let Some(value) = auto_type {
                body["autoType"] = value;
            }
            session
                .save_entry(&serde_json::from_value(body).unwrap())
                .unwrap();
            reopen(&dir, &mut session);
            assert!(
                session.require_db().unwrap().entry(id).unwrap().autotype
                    == flag.as_ref().map(|flag| config(flag, false))
            );
        }
    }
    for standalone in [false, true] {
        let (dir, mut session, id) = fixture(None);
        let expected = config(&DataTransferObfuscation::None, true);
        if standalone {
            session
                .update_entry_autotype(
                    &id.to_string(),
                    &serde_json::from_value(visible(&expected)).unwrap(),
                )
                .unwrap();
        } else {
            session.save_entry(&serde_json::from_value(json!({"target":{"mode":"edit","uuid":id.to_string()},"input":content("After"),"autoType":visible(&expected)})).unwrap()).unwrap();
        }
        let created = session.save_entry(&serde_json::from_value(json!({"target":{"mode":"create"},"input":content("Created"),"autoType":visible(&expected)})).unwrap()).unwrap();
        let created_id = helpers::parse_entry_id(&created.uuid).unwrap();
        reopen(&dir, &mut session);
        for entry_id in [id, created_id] {
            assert!(
                session
                    .require_db()
                    .unwrap()
                    .entry(entry_id)
                    .unwrap()
                    .autotype
                    == Some(expected.clone())
            );
        }
    }
}

#[test]
fn entry_autotype_retention_failed_saves_preserve_data_and_publication() {
    let (dir, mut session, id) = fixture(Some(DataTransferObfuscation::UseClipboard));
    session.snapshot().unwrap();
    let before = session.require_db().unwrap().clone();
    let revision = session.revision;
    let modified = session.modified_at.clone();
    let cache = serde_json::to_value(&session.cached_snapshot).unwrap();
    let mut request: EntrySaveRequest = serde_json::from_value(json!({"target":{"mode":"edit","uuid":id.to_string()},"input":content("After"),"autoType":visible(&config(&DataTransferObfuscation::None, true)),"flags":{"foregroundColor":"invalid"}})).unwrap();
    assert!(session.save_entry(&request).is_err());
    request.flags = None;
    let path = session.path.take(); // Forces final snapshot failure after the candidate mutation.
    assert!(session.save_entry(&request).is_err());
    session.path = path;
    assert!(session
        .update_entry_autotype(
            "invalid",
            &serde_json::from_value(visible(&config(&DataTransferObfuscation::None, true)))
                .unwrap()
        )
        .is_err());
    assert!(session.require_db().unwrap() == &before);
    assert_eq!(session.revision, revision);
    assert_eq!(session.modified_at, modified);
    assert!(!session.dirty);
    assert!(serde_json::to_value(&session.cached_snapshot).unwrap() == cache);
    reopen(&dir, &mut session);
    assert!(
        session.require_db().unwrap().entry(id).unwrap().autotype
            == Some(config(&DataTransferObfuscation::UseClipboard, false))
    );
}
