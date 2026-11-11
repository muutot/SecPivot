use super::*;
use serde_json::{json, Value};

fn input(title: &str) -> Value {
    json!({"groupUuid":"root", "title":title, "username":"", "password":"synthetic-password",
        "url":"https://synthetic.example", "notes":""})
}
fn request(uuid: Option<&str>, title: &str, flags: Value) -> EntrySaveRequest {
    serde_json::from_value(json!({
        "target": uuid.map(|id| json!({"mode":"edit","uuid":id})).unwrap_or(json!({"mode":"create"})),
        "input":input(title), "autoType":{"enabled":false,"defaultSequence":" {USERNAME} ",
        "associations":[{"window":" Synthetic ","sequence":"{PASSWORD}"}]}, "flags":flags
    })).unwrap()
}
fn fixture() -> (tempfile::TempDir, VaultSession, String) {
    let dir = tempfile::tempdir().unwrap();
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
    let state = session
        .add_entry(&serde_json::from_value(input("Before")).unwrap())
        .unwrap();
    session.save().unwrap();
    (dir, session, state.root.entries[0].uuid.clone())
}

fn with_rules(uuid: Option<&str>, policy: Value, flags: Value) -> EntrySaveRequest {
    let mut body = json!({
        "target":uuid.map(|id|json!({"mode":"edit","uuid":id})).unwrap_or(json!({"mode":"create"})),
        "input":input("After"), "flags":flags,
    });
    body["websiteRules"] = policy;
    serde_json::from_value(body).unwrap()
}
fn exact_policy() -> Value {
    json!({"version":1,"rules":[{"url":"https://synthetic.example:443/A?x=%2f#b","mode":"exact"}]})
}

#[test]
fn editor_policy_set_clear_history_masking_and_reopen_are_atomic() {
    let (dir, mut session, uuid) = fixture();
    let revision = session.revision;
    let policy = exact_policy();
    let saved = session
        .save_entry(&with_rules(
            Some(&uuid),
            json!({"mode":"set","rules":policy}),
            json!({"qualityCheck":false}),
        ))
        .unwrap();
    assert_eq!(saved.state.revision, revision + 1);
    assert_eq!(session.get_entry_history(&uuid).unwrap().len(), 1);
    assert!(saved.state.root.entries[0].has_website_rules);
    assert!(!serde_json::to_string(&saved.state)
        .unwrap()
        .contains("/A?x=%2f#b"));
    session.save().unwrap();
    let mut reopened = VaultSession::default();
    reopened
        .open(&dir.path().join("synthetic.kdbx"), "synthetic-master", None)
        .unwrap();
    assert_eq!(
        serde_json::to_value(reopened.get_entry_website_rules(&uuid).unwrap()).unwrap(),
        policy
    );
    let prior = reopened.revision;
    reopened
        .save_entry(&with_rules(
            Some(&uuid),
            json!({"mode":"clear"}),
            json!({"overrideUrl":"https://override.example"}),
        ))
        .unwrap();
    assert_eq!(reopened.revision, prior + 1);
    assert!(reopened.get_entry_website_rules(&uuid).unwrap().is_none());
    let db = reopened.require_db().unwrap();
    let entry = db
        .entry(super::helpers::parse_entry_id(&uuid).unwrap())
        .unwrap();
    assert_eq!(
        entry.override_url.as_deref(),
        Some("https://override.example")
    );
    assert!(entry
        .historical(0)
        .unwrap()
        .fields
        .contains_key(FIELD_WEBSITE_RULES));
    assert!(
        !serde_json::to_string(&reopened.get_entry_history(&uuid).unwrap())
            .unwrap()
            .contains(FIELD_WEBSITE_RULES)
    );
    reopened.save().unwrap();
    let mut final_db = VaultSession::default();
    final_db
        .open(&dir.path().join("synthetic.kdbx"), "synthetic-master", None)
        .unwrap();
    assert!(final_db.get_entry_website_rules(&uuid).unwrap().is_none());
}

#[test]
fn editor_policy_null_omission_and_empty_set_have_distinct_intent() {
    let (_dir, mut session, uuid) = fixture();
    session
        .save_entry(&with_rules(
            Some(&uuid),
            json!({"mode":"set","rules":exact_policy()}),
            Value::Null,
        ))
        .unwrap();
    session
        .save_entry(&with_rules(Some(&uuid), Value::Null, Value::Null))
        .unwrap();
    session
        .save_entry(&request(Some(&uuid), "Omitted", Value::Null))
        .unwrap();
    assert_eq!(
        serde_json::to_value(session.get_entry_website_rules(&uuid).unwrap()).unwrap(),
        exact_policy()
    );
    session
        .save_entry(&with_rules(
            Some(&uuid),
            json!({"mode":"set","rules":{"version":1,"rules":[]}}),
            Value::Null,
        ))
        .unwrap();
    assert!(session
        .get_entry_website_rules(&uuid)
        .unwrap()
        .unwrap()
        .rules
        .is_empty());
    assert!(session.state().unwrap().unwrap().root.entries[0].has_website_rules);
}

#[test]
fn editor_policy_invalid_values_do_not_publish_content_or_creation() {
    let (_dir, mut session, uuid) = fixture();
    let before = session.require_db().unwrap().clone();
    let revision = session.revision;
    for target in [None, Some(uuid.as_str())] {
        for rules in [
            json!({"version":2,"rules":[]}),
            json!({"version":1,"rules":[{"url":"not a URL","mode":"host"}]}),
        ] {
            assert!(session
                .save_entry(&with_rules(
                    target,
                    json!({"mode":"set","rules":rules}),
                    Value::Null
                ))
                .is_err());
            assert!(session.require_db().unwrap() == &before);
            assert_eq!(session.revision, revision);
            assert!(!session.dirty);
        }
    }
    for invalid in [
        json!({"mode":"clear","rules":exact_policy()}),
        json!({"mode":"set"}),
        json!({"mode":"set","rules":{"version":1,"rules":[{"url":"synthetic-private-marker","mode":"unknown"}]}}),
        json!({"mode":"unknown"}),
    ] {
        let error = serde_json::from_value::<EntrySaveRequest>(
            json!({"target":{"mode":"create"},"input":input("After"),"websiteRules":invalid}),
        )
        .err()
        .unwrap();
        assert!(!error.to_string().contains("synthetic-private-marker"));
    }
}

#[test]
fn editor_policy_checks_inherited_override_and_duplicate_creation_sources() {
    let (_dir, mut session, uuid) = fixture();
    session
        .update_entry_flags(&uuid, Some("https://override.example".into()), None, None)
        .unwrap();
    let before = session.require_db().unwrap().clone();
    assert!(session
        .save_entry(&with_rules(
            Some(&uuid),
            json!({"mode":"set","rules":exact_policy()}),
            Value::Null
        ))
        .is_err());
    assert!(session.require_db().unwrap() == &before);
    session
        .save_entry(&with_rules(
            Some(&uuid),
            json!({"mode":"set","rules":exact_policy()}),
            json!({"overrideUrl":""}),
        ))
        .unwrap();
    assert!(session.get_entry_website_rules(&uuid).unwrap().is_some());
    let mut duplicate = with_rules(None, json!({"mode":"clear"}), Value::Null);
    duplicate.input.custom_fields = serde_json::from_value(
        json!([{"name":FIELD_WEBSITE_RULES,"value":exact_policy().to_string(),"protected":true}]),
    )
    .unwrap();
    let before = session.require_db().unwrap().clone();
    assert!(session.save_entry(&duplicate).is_err());
    assert!(session.require_db().unwrap() == &before);
    let created = session
        .save_entry(&with_rules(
            None,
            json!({"mode":"set","rules":exact_policy()}),
            Value::Null,
        ))
        .unwrap();
    assert!(session
        .get_entry_website_rules(&created.uuid)
        .unwrap()
        .is_some());
    assert!(session.get_entry_history(&created.uuid).unwrap().is_empty());
}

#[test]
fn editor_policy_snapshot_failure_restores_every_publication_field() {
    let (_dir, mut session, uuid) = fixture();
    session.snapshot().unwrap();
    let before = session.require_db().unwrap().clone();
    let revision = session.revision;
    let modified = session.modified_at.clone();
    let cache = serde_json::to_value(&session.cached_snapshot).unwrap();
    let path = session.path.take();
    for target in [None, Some(uuid.as_str())] {
        assert!(session
            .save_entry(&with_rules(
                target,
                json!({"mode":"set","rules":exact_policy()}),
                Value::Null
            ))
            .is_err());
        assert!(session.require_db().unwrap() == &before);
        assert!(!session.dirty);
        assert_eq!(session.revision, revision);
        assert_eq!(session.modified_at, modified);
        assert!(serde_json::to_value(&session.cached_snapshot).unwrap() == cache);
    }
    session.path = path;
}

#[test]
fn editor_save_failure_does_not_publish_or_persist_partial_content() {
    for policy_conflict in [false, true] {
        let (dir, mut session, uuid) = fixture();
        if policy_conflict {
            session
                .set_entry_website_rules(
                    &uuid,
                    Some(serde_json::from_value(json!({"version":1,"rules":[]})).unwrap()),
                )
                .unwrap();
            session.save().unwrap();
        }
        let before = session.require_db().unwrap().clone();
        let revision = session.revision;
        let modified = session.modified_at.clone();
        let flags = if policy_conflict {
            json!({"overrideUrl":"https://synthetic.example","qualityCheck":false})
        } else {
            json!({"foregroundColor":"invalid","qualityCheck":false})
        };
        assert!(session
            .save_entry(&request(Some(&uuid), "After", flags))
            .is_err());
        assert!(
            session.require_db().unwrap() == &before,
            "failed save changed native data/history"
        );
        assert_eq!(session.revision, revision);
        assert_eq!(session.modified_at, modified);
        assert!(!session.dirty);
        session.save().unwrap();
        let mut reopened = VaultSession::default();
        reopened
            .open(&dir.path().join("synthetic.kdbx"), "synthetic-master", None)
            .unwrap();
        assert_eq!(
            reopened.state().unwrap().unwrap().root.entries[0].title,
            "Before"
        );
    }
}

#[test]
fn editor_save_all_fields_have_one_revision_and_complete_history() {
    let (dir, mut session, uuid) = fixture();
    session
        .update_entry_autotype(
            &uuid,
            &serde_json::from_value(json!({"enabled":true,"defaultSequence":"OLD"})).unwrap(),
        )
        .unwrap();
    session
        .update_entry_flags(
            &uuid,
            Some("https://old.example".into()),
            Some(true),
            Some("#AABBCC".into()),
        )
        .unwrap();
    let before = session.revision;
    let saved = session.save_entry(&request(Some(&uuid), "After", json!({
        "overrideUrl":" https://new.example ","qualityCheck":false,"foregroundColor":"#112233"
    }))).unwrap();
    assert_eq!(saved.uuid, uuid);
    assert_eq!(saved.state.revision, before + 1);
    assert_eq!(session.get_entry_history(&uuid).unwrap().len(), 1);
    session.save().unwrap();
    let mut reopened = VaultSession::default();
    reopened
        .open(&dir.path().join("synthetic.kdbx"), "synthetic-master", None)
        .unwrap();
    let entry = reopened
        .require_db()
        .unwrap()
        .entry(super::helpers::parse_entry_id(&uuid).unwrap())
        .unwrap();
    assert_eq!(entry.get_title(), Some("After"));
    assert!(!entry.quality_check);
    assert_eq!(entry.override_url.as_deref(), Some("https://new.example"));
    assert_eq!(entry.foreground_color, Some("#112233".parse().unwrap()));
    assert!(!entry.autotype.as_ref().unwrap().enabled);
    assert_eq!(
        entry.autotype.as_ref().unwrap().default_sequence.as_deref(),
        Some("{USERNAME}")
    );
    assert_eq!(
        entry.autotype.as_ref().unwrap().associations[0].window,
        "Synthetic"
    );
    let old = entry.historical(0).unwrap();
    assert_eq!(old.get_title(), Some("Before"));
    assert!(old.quality_check);
    assert_eq!(old.override_url.as_deref(), Some("https://old.example"));
    assert_eq!(old.foreground_color, Some("#AABBCC".parse().unwrap()));
    assert_eq!(
        old.autotype.as_ref().unwrap().default_sequence.as_deref(),
        Some("OLD")
    );
    assert!(!serde_json::to_string(&saved.state)
        .unwrap()
        .contains("synthetic-password"));
}

#[test]
fn editor_save_creation_is_atomic_retryable_and_returns_exact_identity() {
    let (_dir, mut session, _) = fixture();
    let before = session.require_db().unwrap().clone();
    let revision = session.revision;
    assert!(session
        .save_entry(&request(
            None,
            "Created",
            json!({"foregroundColor":"invalid"})
        ))
        .is_err());
    assert!(session.require_db().unwrap() == &before);
    assert_eq!(session.revision, revision);
    let created = session
        .save_entry(&request(None, "Created", json!({"qualityCheck":false})))
        .unwrap();
    assert_eq!(created.state.revision, revision + 1);
    assert_eq!(created.state.root.entries.len(), 2);
    let entry = created
        .state
        .root
        .entries
        .iter()
        .find(|e| e.uuid == created.uuid)
        .unwrap();
    assert_eq!(entry.title, "Created");
    assert!(!entry.quality_check);
    assert!(!entry.autotype.as_ref().unwrap().enabled);
    assert!(session.get_entry_history(&created.uuid).unwrap().is_empty());
    let another = session
        .save_entry(&request(None, "Created", Value::Null))
        .unwrap();
    assert_ne!(created.uuid, another.uuid);
}

#[test]
fn editor_save_omitted_extras_keep_native_policy_and_metadata() {
    let (_dir, mut session, uuid) = fixture();
    session
        .set_entry_website_rules(
            &uuid,
            Some(serde_json::from_value(json!({"version":1,"rules":[]})).unwrap()),
        )
        .unwrap();
    session
        .update_entry_autotype(
            &uuid,
            &serde_json::from_value(json!({"enabled":false,"defaultSequence":"OLD"})).unwrap(),
        )
        .unwrap();
    session
        .update_entry_flags(&uuid, None, Some(false), Some("#AABBCC".into()))
        .unwrap();
    let request: EntrySaveRequest = serde_json::from_value(
        json!({"target":{"mode":"edit","uuid":uuid},"input":input("After")}),
    )
    .unwrap();
    let state = session.save_entry(&request).unwrap().state;
    let entry = &state.root.entries[0];
    assert!(entry.has_website_rules);
    assert!(!entry.quality_check);
    assert!(!entry.autotype.as_ref().unwrap().enabled);
    assert_eq!(entry.foreground_color.as_deref(), Some("#AABBCC"));
    assert!(entry
        .custom_fields
        .iter()
        .all(|field| field.name != FIELD_WEBSITE_RULES));
}

#[test]
fn editor_save_invalid_targets_and_payloads_leave_original_state() {
    let (_dir, mut session, uuid) = fixture();
    let before = session.require_db().unwrap().clone();
    for bad in [
        json!({"target":{"mode":"edit"},"input":input("After")}),
        json!({"target":{"mode":"edit","uuid":null},"input":input("After")}),
        json!({"target":{"mode":"create","uuid":uuid},"input":input("After")}),
        json!({"input":input("After")}),
        json!({"target":{"mode":"unknown"},"input":input("After")}),
        json!({"target":{"mode":"create"},"input":input("After"),"extra":true}),
    ] {
        assert!(serde_json::from_value::<EntrySaveRequest>(bad).is_err());
    }
    let mut bad = request(Some(&uuid), "After", Value::Null);
    bad.input.group_uuid = "missing".into();
    assert!(session.save_entry(&bad).is_err());
    bad.input.group_uuid = "root".into();
    bad.input.attachments = serde_json::from_value(json!([{"name":"test","data":"!!!"}])).unwrap();
    assert!(session.save_entry(&bad).is_err());
    assert!(session
        .save_entry(&request(Some("missing"), "After", Value::Null))
        .is_err());
    assert!(session.require_db().unwrap() == &before);
}

#[test]
fn editor_save_snapshot_failure_restores_database_and_publication_metadata() {
    let (_dir, mut session, uuid) = fixture();
    session.snapshot().unwrap();
    let before = session.require_db().unwrap().clone();
    let dirty = session.dirty;
    let revision = session.revision;
    let modified = session.modified_at.clone();
    let cache = serde_json::to_value(&session.cached_snapshot).unwrap();
    let path = session.path.take();
    for target in [None, Some(uuid.as_str())] {
        assert!(session
            .save_entry(&request(target, "After", json!({"qualityCheck":false})))
            .is_err());
        assert!(session.require_db().unwrap() == &before);
        assert_eq!(session.dirty, dirty);
        assert_eq!(session.revision, revision);
        assert_eq!(session.modified_at, modified);
        assert!(serde_json::to_value(&session.cached_snapshot).unwrap() == cache);
    }
    session.path = path;
}

#[test]
fn editor_save_attachments_and_history_retention_survive_reopen() {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(b"synthetic binary");
    for cap in [0, 10] {
        let (dir, mut session, uuid) = fixture();
        session.require_db_mut().unwrap().meta.history_max_items = Some(cap);
        let mut with_binary = request(Some(&uuid), "With binary", Value::Null);
        with_binary.input.attachments =
            serde_json::from_value(json!([{"name":"test.bin","data":encoded}])).unwrap();
        session.save_entry(&with_binary).unwrap();
        let prior = session.revision;
        session
            .save_entry(&request(
                Some(&uuid),
                "Removed",
                json!({"qualityCheck":false}),
            ))
            .unwrap();
        assert_eq!(session.revision, prior + 1);
        session.save().unwrap();
        let mut reopened = VaultSession::default();
        reopened
            .open(&dir.path().join("synthetic.kdbx"), "synthetic-master", None)
            .unwrap();
        assert!(reopened.attachment_data(&uuid, "test.bin").is_err());
        let history = reopened.get_entry_history(&uuid).unwrap();
        if cap == 0 {
            assert!(history.is_empty());
        } else {
            let db = reopened.require_db().unwrap();
            let entry = db
                .entry(super::helpers::parse_entry_id(&uuid).unwrap())
                .unwrap();
            let old = entry.historical(0).unwrap();
            assert!(
                old.attachment_by_name("test.bin").unwrap().get().as_slice() == b"synthetic binary"
            );
        }
    }
}
