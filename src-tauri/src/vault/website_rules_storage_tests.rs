use super::*;
use crate::bridge::BridgeHost;
use crate::rpc::{RpcFieldWrite, RpcHost, RpcLoginWrite};
use crate::vault::{CustomField, EntryInput, EntryPatch, ImportEntryInput, FIELD_KPRPC_CONFIG};
use tempfile::TempDir;

fn config(rules: &[(&str, WebsiteMode)]) -> WebsiteRuleSet {
    WebsiteRuleSet {
        version: 1,
        rules: rules
            .iter()
            .map(|(url, mode)| WebsiteRule {
                url: (*url).into(),
                mode: *mode,
            })
            .collect(),
    }
}

fn input() -> EntryInput {
    serde_json::from_value(serde_json::json!({
        "groupUuid":"root", "title":"Synthetic", "username":"synthetic-user",
        "password":"synthetic-fixture", "url":"https://legacy.example", "notes":""
    }))
    .unwrap()
}

fn session() -> (TempDir, VaultSession, String) {
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
    let state = session.add_entry(&input()).unwrap();
    let id = state.root.entries[0].uuid.clone();
    (dir, session, id)
}

fn matches(session: &VaultSession, id: &str, url: &str, expected: bool) {
    assert_eq!(
        session
            .logins_for(url, None)
            .iter()
            .any(|entry| entry.uuid == id),
        expected
    );
    assert_eq!(
        session
            .find_logins(&[url.to_owned()], None, None, None)
            .iter()
            .any(|entry| entry.uuid == id),
        expected
    );
}

fn stored(session: &VaultSession, id: &str) -> String {
    let entry = session
        .require_db()
        .unwrap()
        .entry(parse_entry_id(id).unwrap())
        .unwrap();
    let field = entry.fields.get(FIELD_WEBSITE_RULES).unwrap();
    assert!(field.is_protected());
    field.get().to_owned()
}

#[test]
fn website_policy_history_diff_marks_policy_only_changes_without_leaking_values() {
    let (dir, mut session, id) = session();
    session
        .set_entry_website_rules(
            &id,
            Some(config(&[(
                "https://synthetic.example:443/A?x=%2f#b",
                WebsiteMode::Exact,
            )])),
        )
        .unwrap();
    let history = session.get_entry_history(&id).unwrap();
    let diff = serde_json::to_value(&history[0].diff).unwrap();
    assert_eq!(
        diff.get("websiteRules"),
        Some(&serde_json::Value::Bool(true)),
        "policy-only changes must have an authoritative diff flag"
    );
    assert!(
        diff.as_object()
            .unwrap()
            .iter()
            .all(|(key, value)| key == "websiteRules"
                || value == &serde_json::Value::Bool(false)
                || value == &serde_json::json!([])),
        "no unrelated fields changed"
    );
    let timeline = serde_json::to_value(session.change_timeline().unwrap()).unwrap();
    assert_eq!(timeline[0]["diff"]["websiteRules"], true);
    for payload in [
        serde_json::to_string(&history).unwrap(),
        timeline.to_string(),
    ] {
        assert!(!payload.contains("/A?x=%2f#b"));
        assert!(!payload.contains(FIELD_WEBSITE_RULES));
    }
    session.save().unwrap();
    let mut reopened = VaultSession::default();
    reopened
        .open(&dir.path().join("synthetic.kdbx"), "synthetic-master", None)
        .unwrap();
    assert_eq!(
        serde_json::to_value(reopened.get_entry_history(&id).unwrap()[0].diff.clone()).unwrap()
            ["websiteRules"],
        true
    );
}

#[test]
fn website_policy_history_diff_tracks_bytes_protection_and_presence() {
    let (_dir, mut session, id) = session();
    session
        .set_entry_website_rules(
            &id,
            Some(config(&[("https://synthetic.example", WebsiteMode::Host)])),
        )
        .unwrap();
    let entry_id = parse_entry_id(&id).unwrap();
    let baseline = session.require_db().unwrap().clone();
    let raw = stored(&session, &id);
    for (value, expected) in [
        (Some(keepass::db::Value::protected(raw.clone())), false),
        (Some(keepass::db::Value::unprotected(raw.clone())), true),
        (
            Some(keepass::db::Value::protected(raw.replace("host", "site"))),
            true,
        ),
        (Some(keepass::db::Value::protected(String::new())), true),
        (None, true),
    ] {
        let db = session.require_db_mut().unwrap();
        let mut entry = db.entry_mut(entry_id).unwrap();
        match value {
            Some(value) => {
                entry.fields.insert(FIELD_WEBSITE_RULES.to_owned(), value);
            }
            None => {
                entry.fields.remove(FIELD_WEBSITE_RULES);
            }
        }
        let diff = super::super::entries::history_diff(
            &baseline.entry(entry_id).unwrap(),
            &entry.as_ref(),
        );
        assert_eq!(diff.website_rules, expected);
        assert!(diff.custom_fields.is_empty());
    }
    let absent = session.require_db().unwrap().clone();
    session
        .require_db_mut()
        .unwrap()
        .entry_mut(entry_id)
        .unwrap()
        .set(
            FIELD_WEBSITE_RULES,
            keepass::db::Value::protected(String::new()),
        );
    let diff = super::super::entries::history_diff(
        &absent.entry(entry_id).unwrap(),
        &session.require_db().unwrap().entry(entry_id).unwrap(),
    );
    assert!(
        diff.website_rules,
        "an empty foreign field still differs from no policy"
    );
}

#[test]
fn managed_policy_replaces_legacy_positive_rules_and_survives_native_reopen() {
    let (_dir, mut session, id) = session();
    session.update_custom_field(&id, FIELD_KPRPC_CONFIG, r#"{"altURLs":["https://alt.example"],"regExURLs":["legacy|alt"],"blockedURLs":["https://blocked.site.example"],"regExBlockedURLs":["/deny$"]}"#, true).unwrap();
    let rules = config(&[
        ("https://login.site.example", WebsiteMode::Site),
        ("https://host.example:8443/Path", WebsiteMode::Host),
        ("https://exact.example/测试?Case=YES#A", WebsiteMode::Exact),
        ("https://host.example:8443/Path", WebsiteMode::Never),
        ("https://ignored.example", WebsiteMode::Never),
    ]);
    let before = session.revision;
    let state = session
        .set_entry_website_rules(&id, Some(rules.clone()))
        .unwrap();
    assert_eq!(state.revision, before + 1);
    assert!(state.dirty);
    let wire = serde_json::to_string(&state).unwrap();
    assert!(
        !wire.contains(FIELD_WEBSITE_RULES)
            && !wire.contains("exact.example")
            && !wire.contains("synthetic-fixture")
    );
    let json = stored(&session, &id);
    let path = session.path.clone().unwrap();
    session.save().unwrap();
    session.close();
    assert!(session.get_entry_website_rules(&id).is_err());
    matches(&session, &id, "https://host.example:8443", false);
    session
        .open(std::path::Path::new(&path), "synthetic-master", None)
        .unwrap();
    assert!(session.get_entry_website_rules(&id).unwrap() == Some(rules));
    assert_eq!(stored(&session, &id), json);
    for registrable in [false, true] {
        session.match_registrable_domain = registrable;
        for (url, expected) in [
            ("https://sub.site.example", true),
            ("https://blocked.site.example", false),
            ("https://host.example:8443/Other", true),
            ("https://host.example:8443/deny", false),
            ("https://host.example/Other", false),
            ("https://exact.example/测试?Case=YES#A", true),
            ("https://exact.example/测试?Case=yes#A", false),
            ("https://legacy.example", false),
            ("https://alt.example", false),
            ("https://ignored.example", false),
            ("https://unrelated.example", false),
        ] {
            matches(&session, &id, url, expected);
        }
    }
    assert_eq!(
        session
            .logins_for(
                "https://unrelated.example",
                Some("https://host.example:8443/Other")
            )
            .len(),
        0
    );
    let urls = session.find_logins(&[], Some(&id), None, None)[0]
        .urls
        .clone();
    for (submit, expected) in [
        (Some("https://unrelated.example"), 0),
        (Some("https://host.example:8443/deny"), 0),
        (Some("https://host.example:8443/Other"), 1),
        (Some(""), 1),
        (None, 1),
    ] {
        assert_eq!(
            session
                .logins_for("https://host.example:8443/Page", submit)
                .len(),
            expected
        );
    }
    assert_eq!(urls.len(), 3);
    assert!(!urls
        .iter()
        .any(|u| u.contains("legacy") || u.contains("ignored")));
    assert!(session.autotype_match("Synthetic host.example").is_err());
    assert!(session
        .autotype_match_candidates("Synthetic host.example")
        .is_err());
}

#[test]
fn absent_empty_never_and_explicit_clear_have_distinct_native_results() {
    let (_dir, mut session, id) = session();
    assert!(session.get_entry_website_rules(&id).unwrap().is_none());
    matches(&session, &id, "https://legacy.example", true);
    assert_eq!(
        session
            .logins_for("https://unrelated.example", Some("https://legacy.example"))
            .len(),
        1
    );
    for rules in [
        config(&[]),
        config(&[("https://legacy.example", WebsiteMode::Never)]),
    ] {
        session.set_entry_website_rules(&id, Some(rules)).unwrap();
        matches(&session, &id, "https://legacy.example", false);
        assert!(session.get_entry_website_rules(&id).unwrap().is_some());
    }
    session.set_entry_website_rules(&id, None).unwrap();
    assert!(session.get_entry_website_rules(&id).unwrap().is_none());
    matches(&session, &id, "https://legacy.example", true);
    assert!(session.autotype_match("Synthetic legacy.example").is_ok());
}

#[test]
fn setter_rejects_invalid_missing_read_only_and_snapshot_failure_atomically() {
    let (_dir, mut session, id) = session();
    let before = session.require_db().unwrap().clone();
    let state = session.state().unwrap().unwrap();
    let cache = serde_json::to_string(&session.cached_snapshot).unwrap();
    for (target, rules) in [
        (
            id.clone(),
            WebsiteRuleSet {
                version: 2,
                rules: vec![],
            },
        ),
        (id.clone(), config(&[("invalid", WebsiteMode::Never)])),
        (uuid::Uuid::new_v4().to_string(), config(&[])),
        ("invalid-id".to_owned(), config(&[])),
    ] {
        assert!(session
            .set_entry_website_rules(&target, Some(rules))
            .is_err());
        assert_eq!(session.require_db().unwrap(), &before);
        assert_eq!(session.revision, state.revision);
        assert_eq!(session.dirty, state.dirty);
    }
    let path = session.path.take();
    assert!(session
        .set_entry_website_rules(&id, Some(config(&[])))
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    assert_eq!(
        (session.revision, session.dirty, &session.modified_at),
        (state.revision, state.dirty, &state.modified_at)
    );
    assert_eq!(
        serde_json::to_string(&session.cached_snapshot).unwrap(),
        cache
    );
    session.path = path;
    for _ in 0..3 {
        session.note_save_failure();
    }
    assert!(session.is_read_only());
    for rules in [Some(config(&[])), None] {
        assert!(session.set_entry_website_rules(&id, rules).is_err());
        assert_eq!(session.require_db().unwrap(), &before);
    }
    session.close();
    assert!(session.set_entry_website_rules(&id, None).is_err());
}

#[test]
fn no_ops_keep_history_and_revision_and_policy_changes_follow_history_retention() {
    for cap in [0, 1, 10] {
        let (_dir, mut session, id) = session();
        session.require_db_mut().unwrap().meta.history_max_items = Some(cap);
        let rules = config(&[("https://safe.example", WebsiteMode::Host)]);
        session
            .set_entry_website_rules(&id, Some(rules.clone()))
            .unwrap();
        let before = session.require_db().unwrap().clone();
        let revision = session.revision;
        session.set_entry_website_rules(&id, Some(rules)).unwrap();
        assert_eq!(session.require_db().unwrap(), &before);
        assert_eq!(session.revision, revision);
        session.set_entry_website_rules(&id, None).unwrap();
        assert!(session.get_entry_history(&id).unwrap().len() <= cap as usize);
        let before = session.require_db().unwrap().clone();
        let revision = session.revision;
        session.set_entry_website_rules(&id, None).unwrap();
        assert_eq!(session.require_db().unwrap(), &before);
        assert_eq!(session.revision, revision);
    }
}

#[test]
fn full_batch_custom_edits_and_history_restore_preserve_managed_policy() {
    let (_dir, mut session, id) = session();
    let rules = config(&[("https://safe.example", WebsiteMode::Host)]);
    session
        .set_entry_website_rules(&id, Some(rules.clone()))
        .unwrap();
    let json = stored(&session, &id);
    let mut edit = input();
    edit.url = "https://new-display.example".into();
    edit.custom_fields.push(CustomField {
        name: FIELD_WEBSITE_RULES.into(),
        value: "{}".into(),
        protected: false,
    });
    edit.custom_fields.push(CustomField {
        name: "Ordinary".into(),
        value: "Synthetic".into(),
        protected: false,
    });
    session.update_entry(&id, &edit).unwrap();
    assert_eq!(stored(&session, &id), json);
    session
        .update_entries(
            std::slice::from_ref(&id),
            &EntryPatch {
                url: Some("https://batch-display.example".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(stored(&session, &id), json);
    let before = session.require_db().unwrap().clone();
    assert!(session
        .update_custom_field(&id, FIELD_WEBSITE_RULES, "{}", false)
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    matches(&session, &id, "https://safe.example", true);
    matches(&session, &id, "https://batch-display.example", false);
    session
        .set_entry_website_rules(
            &id,
            Some(config(&[("https://other.example", WebsiteMode::Exact)])),
        )
        .unwrap();
    session.restore_entry_version(&id, 0).unwrap();
    assert!(session.get_entry_website_rules(&id).unwrap() == Some(rules));
    assert_eq!(stored(&session, &id), json);
    matches(&session, &id, "https://safe.example/path", true);
    matches(&session, &id, "https://other.example", false);
    let history = serde_json::to_string(&session.get_entry_history(&id).unwrap()).unwrap();
    assert!(!history.contains(FIELD_WEBSITE_RULES) && !history.contains("safe.example"));
}

#[test]
fn override_conflicts_and_foreign_malformed_policies_never_fall_back() {
    let (_dir, mut session, id) = session();
    session
        .update_entry_flags(&id, Some("https://override.example".into()), None, None)
        .unwrap();
    let before = session.require_db().unwrap().clone();
    assert!(session
        .set_entry_website_rules(&id, Some(config(&[])))
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    session
        .update_entry_flags(&id, Some(String::new()), None, None)
        .unwrap();
    session
        .set_entry_website_rules(
            &id,
            Some(config(&[("https://safe.example", WebsiteMode::Site)])),
        )
        .unwrap();
    let before = session.require_db().unwrap().clone();
    assert!(session
        .update_entry_flags(
            &id,
            Some("https://override.example".into()),
            Some(false),
            None
        )
        .is_err());
    assert_eq!(session.require_db().unwrap(), &before);
    let entry_id = parse_entry_id(&id).unwrap();
    session
        .require_db_mut()
        .unwrap()
        .entry_mut(entry_id)
        .unwrap()
        .override_url = Some("https://override.example".into());
    matches(&session, &id, "https://safe.example", false);
    matches(&session, &id, "https://override.example", false);
    assert!(session.find_logins(&[], Some(&id), None, None)[0]
        .urls
        .is_empty());
    session
        .update_entry_flags(&id, Some(String::new()), None, None)
        .unwrap();
    matches(&session, &id, "https://safe.example", true);
    for raw in [
        "",
        "{",
        r#"{"version":1,"rules":[{"url":"https://safe.example","mode":"site"},{"url":"invalid","mode":"never"}]}"#,
    ] {
        session
            .require_db_mut()
            .unwrap()
            .entry_mut(entry_id)
            .unwrap()
            .set(FIELD_WEBSITE_RULES, Value::protected(raw.to_owned()));
        assert!(session.get_entry_website_rules(&id).is_err());
        matches(&session, &id, "https://safe.example", false);
        matches(&session, &id, "https://legacy.example", false);
        assert!(session.find_logins(&[], Some(&id), None, None)[0]
            .urls
            .is_empty());
    }
}

fn rpc_login(url: &str) -> RpcLoginWrite {
    RpcLoginWrite {
        title: "Synthetic updated".into(),
        urls: vec![url.into()],
        form_field_list: vec![
            RpcFieldWrite {
                field_type: "FFTusername".into(),
                value: "synthetic-updated".into(),
                ..Default::default()
            },
            RpcFieldWrite {
                field_type: "FFTpassword".into(),
                value: "synthetic-updated-fixture".into(),
                ..Default::default()
            },
        ],
        ..Default::default()
    }
}

#[test]
fn rpc_url_changes_reject_before_persistence_and_credential_updates_keep_policy() {
    let (dir, mut session, id) = session();
    session
        .set_entry_website_rules(
            &id,
            Some(config(&[("https://safe.example", WebsiteMode::Host)])),
        )
        .unwrap();
    session.save().unwrap();
    let path = dir.path().join("synthetic.kdbx");
    let bytes = std::fs::read(&path).unwrap();
    let before = session.require_db().unwrap().clone();
    let revision = session.revision;
    for mode in [1, 2, 3, 5] {
        assert!(session
            .update_login(&rpc_login("https://new.example"), &id, mode)
            .is_err());
        assert_eq!(session.require_db().unwrap(), &before);
        assert_eq!(session.revision, revision);
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
    let json = stored(&session, &id);
    session
        .update_login(&rpc_login("https://new.example"), &id, 4)
        .unwrap();
    assert_eq!(stored(&session, &id), json);
    assert_eq!(
        session
            .require_db()
            .unwrap()
            .entry(parse_entry_id(&id).unwrap())
            .unwrap()
            .get_url(),
        Some("https://legacy.example")
    );
    matches(&session, &id, "https://safe.example", true);
    matches(&session, &id, "https://new.example", false);
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    assert_eq!(stored(&session, &id), json);
    assert_eq!(
        session.get_entry_password(&id).unwrap(),
        "synthetic-updated-fixture"
    );
    session
        .set_login(
            "synthetic-bridge",
            "synthetic-bridge-fixture",
            "https://new.example",
            Some(&id),
        )
        .unwrap();
    assert_eq!(stored(&session, &id), json);
}

#[test]
fn rpc_custom_fields_cannot_create_or_replace_managed_policy() {
    let (dir, mut session, id) = session();
    session
        .set_entry_website_rules(&id, Some(config(&[])))
        .unwrap();
    session.save().unwrap();
    let before = session.require_db().unwrap().clone();
    let bytes = std::fs::read(dir.path().join("synthetic.kdbx")).unwrap();
    for name in [
        FIELD_WEBSITE_RULES.to_owned(),
        format!(" {FIELD_WEBSITE_RULES} "),
    ] {
        let mut login = rpc_login("https://new.example");
        login.form_field_list.push(RpcFieldWrite {
            display_name: name,
            field_type: "FFTtext".into(),
            value: "{}".into(),
            ..Default::default()
        });
        assert!(session.add_login(&login, "root").is_err());
        assert!(session.update_login(&login, &id, 4).is_err());
        assert_eq!(session.require_db().unwrap(), &before);
        assert_eq!(
            std::fs::read(dir.path().join("synthetic.kdbx")).unwrap(),
            bytes
        );
    }
}

#[test]
fn native_entry_exchange_keeps_policy_protection_and_matching_without_clipboard() {
    let (_dir, mut session, id) = session();
    let rules = config(&[
        ("https://safe.example:443", WebsiteMode::Host),
        ("https://ignored.example", WebsiteMode::Never),
    ]);
    session
        .set_entry_website_rules(&id, Some(rules.clone()))
        .unwrap();
    let json = stored(&session, &id);
    let document = Zeroizing::new(
        super::super::exchange::build_entries_document(
            session.require_db().unwrap(),
            &[parse_entry_id(&id).unwrap()],
        )
        .unwrap(),
    );
    let parsed = super::super::exchange::parse_entries_document(&document).unwrap();
    assert_eq!(parsed.len(), 1);
    let new_id = super::super::exchange::insert_entry(
        &mut session.require_db_mut().unwrap().root_mut(),
        &parsed[0],
    )
    .unwrap()
    .to_string();
    assert_ne!(new_id, id);
    assert!(session.get_entry_website_rules(&new_id).unwrap() == Some(rules));
    assert_eq!(stored(&session, &new_id), json);
    matches(&session, &new_id, "https://safe.example:443/path", true);
    matches(&session, &new_id, "https://safe.example/path", false);
    matches(&session, &new_id, "https://ignored.example", false);
    session.save().unwrap();
    let path = session.path.clone().unwrap();
    session.close();
    session
        .open(std::path::Path::new(&path), "synthetic-master", None)
        .unwrap();
    assert_eq!(stored(&session, &new_id), json);
}

#[test]
fn native_xml_export_and_batch_import_preserve_policy_and_reject_invalid_batches() {
    let (_dir, mut session, id) = session();
    let rules = config(&[("https://safe.example/测试?Case=YES#A", WebsiteMode::Exact)]);
    session
        .set_entry_website_rules(&id, Some(rules.clone()))
        .unwrap();
    let xml = Zeroizing::new(session.export_xml_content().unwrap());
    let file: super::super::exchange::KeePassFile = quick_xml::de::from_str(&xml).unwrap();
    let field = file.root.group.entries[0]
        .string_fields
        .iter()
        .find(|f| f.key == FIELD_WEBSITE_RULES)
        .unwrap();
    assert_eq!(field.value.value, stored(&session, &id));
    assert!(xml.contains("ProtectInMemory=\"True\""));
    let mut imported = input();
    imported.custom_fields.push(CustomField {
        name: FIELD_WEBSITE_RULES.into(),
        value: field.value.value.clone(),
        protected: false,
    });
    let state = session
        .import_entries(&[ImportEntryInput {
            input: imported.clone(),
            quality_check: None,
            favorite: None,
        }])
        .unwrap();
    let new_id = &state
        .root
        .entries
        .iter()
        .find(|entry| entry.uuid != id)
        .unwrap()
        .uuid;
    assert!(session.get_entry_website_rules(new_id).unwrap() == Some(rules));
    stored(&session, new_id);
    matches(
        &session,
        new_id,
        "https://safe.example/测试?Case=YES#A",
        true,
    );
    matches(
        &session,
        new_id,
        "https://safe.example/测试?Case=yes#A",
        false,
    );
    let before = session.require_db().unwrap().clone();
    let revision = session.revision;
    for duplicate in [false, true] {
        let mut invalid = imported.clone();
        if duplicate {
            invalid.custom_fields.push(invalid.custom_fields[0].clone());
        } else {
            invalid.custom_fields[0].value = "{}".into();
        }
        assert!(session
            .add_entries(&[imported.clone(), invalid.clone()])
            .is_err());
        assert!(session.add_entry(&invalid).is_err());
        assert_eq!(session.require_db().unwrap(), &before);
        assert_eq!(session.revision, revision);
    }
}
