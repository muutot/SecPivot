use super::*;
use crate::vault::{EntryInput, VaultSession};
use serde_json::json;
use tempfile::TempDir;

fn policy(url: &str) -> WebsiteRuleSet {
    WebsiteRuleSet {
        version: 1,
        rules: vec![WebsiteRule {
            url: url.into(),
            mode: WebsiteMode::Exact,
        }],
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
    let input: EntryInput = serde_json::from_value(json!({
        "groupUuid":"root", "title":"Synthetic", "username":"", "password":"",
        "url":"https://legacy.example", "notes":""
    }))
    .unwrap();
    let uuid = session.add_entry(&input).unwrap().root.entries[0]
        .uuid
        .clone();
    (dir, session, uuid)
}

#[test]
fn historical_rule_read_returns_selected_content_and_both_sides_survive_reopen() {
    let (dir, mut session, uuid) = fixture();
    let old = policy("https://old.example/exact?value=1");
    let current = policy("https://new.example/exact?value=2");
    session
        .set_entry_website_rules(&uuid, Some(old.clone()))
        .unwrap();
    session
        .set_entry_website_rules(&uuid, Some(current.clone()))
        .unwrap();
    let before = serde_json::to_value(session.state().unwrap()).unwrap();
    let selected = session.get_entry_history(&uuid).unwrap()[0].clone();
    for (source, expected) in [
        (HistoryRuleSource::Version, &old),
        (HistoryRuleSource::Current, &current),
    ] {
        let result = session
            .get_history_website_rules(&uuid, selected.index, &selected.selection_token, source)
            .unwrap()
            .unwrap();
        assert_eq!(
            serde_json::to_value(result).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
    assert_eq!(
        serde_json::to_value(session.state().unwrap()).unwrap(),
        before
    );
    let history_wire = serde_json::to_string(&session.get_entry_history(&uuid).unwrap()).unwrap();
    assert!(!history_wire.contains("old.example") && !history_wire.contains("new.example"));
    session.save().unwrap();
    session.close();
    session
        .open(&dir.path().join("synthetic.kdbx"), "synthetic-master", None)
        .unwrap();
    assert!(session
        .get_history_website_rules(
            &uuid,
            selected.index,
            &selected.selection_token,
            HistoryRuleSource::Version
        )
        .is_err());
    let fresh = session.get_entry_history(&uuid).unwrap()[0].clone();
    for (source, expected) in [
        (HistoryRuleSource::Version, old),
        (HistoryRuleSource::Current, current),
    ] {
        assert_eq!(
            serde_json::to_value(
                session
                    .get_history_website_rules(&uuid, fresh.index, &fresh.selection_token, source)
                    .unwrap()
                    .unwrap()
            )
            .unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
}

#[test]
fn historical_rule_absence_empty_and_malformed_are_distinct() {
    let (_dir, mut session, uuid) = fixture();
    let empty = WebsiteRuleSet {
        version: 1,
        rules: vec![],
    };
    session.set_entry_website_rules(&uuid, Some(empty)).unwrap();
    let absent = session.get_entry_history(&uuid).unwrap()[0].clone();
    assert!(session
        .get_history_website_rules(
            &uuid,
            absent.index,
            &absent.selection_token,
            HistoryRuleSource::Version
        )
        .unwrap()
        .is_none());
    assert!(session
        .get_history_website_rules(
            &uuid,
            absent.index,
            &absent.selection_token,
            HistoryRuleSource::Current
        )
        .unwrap()
        .unwrap()
        .rules
        .is_empty());
    session.set_entry_website_rules(&uuid, None).unwrap();
    let empty = session.get_entry_history(&uuid).unwrap()[0].clone();
    assert!(session
        .get_history_website_rules(
            &uuid,
            empty.index,
            &empty.selection_token,
            HistoryRuleSource::Version
        )
        .unwrap()
        .unwrap()
        .rules
        .is_empty());
    let id = parse_entry_id(&uuid).unwrap();
    session
        .require_db_mut()
        .unwrap()
        .entry_mut(id)
        .unwrap()
        .fields
        .insert(
            FIELD_WEBSITE_RULES.into(),
            Value::protected("synthetic-invalid-policy".to_owned()),
        );
    session.mark_dirty();
    // Clearing snapshots a foreign malformed policy without treating it as absent.
    session.set_entry_website_rules(&uuid, None).unwrap();
    let malformed = session.get_entry_history(&uuid).unwrap()[0].clone();
    assert!(malformed.has_website_rules);
    let error = session
        .get_history_website_rules(
            &uuid,
            malformed.index,
            &malformed.selection_token,
            HistoryRuleSource::Version,
        )
        .err()
        .unwrap();
    assert!(!error.contains("synthetic-invalid-policy"));
    assert!(session
        .get_history_website_rules(
            &uuid,
            malformed.index,
            &malformed.selection_token,
            HistoryRuleSource::Current
        )
        .unwrap()
        .is_none());
}

#[test]
fn historical_rule_reads_reject_shifted_or_missing_versions_for_either_side() {
    let (_dir, mut session, uuid) = fixture();
    session
        .set_entry_website_rules(&uuid, Some(policy("https://old.example")))
        .unwrap();
    let selected = session.get_entry_history(&uuid).unwrap()[0].clone();
    session
        .set_entry_website_rules(&uuid, Some(policy("https://new.example")))
        .unwrap();
    let before = serde_json::to_value(session.state().unwrap()).unwrap();
    for source in [HistoryRuleSource::Version, HistoryRuleSource::Current] {
        assert!(session
            .get_history_website_rules(&uuid, selected.index, &selected.selection_token, source)
            .err()
            .unwrap()
            .starts_with("HISTORY_SELECTION_STALE:"));
        let forged = session.history_selection_token(parse_entry_id(&uuid).unwrap(), 99);
        assert!(session
            .get_history_website_rules(&uuid, 99, &forged, source)
            .is_err());
    }
    assert_eq!(
        serde_json::to_value(session.state().unwrap()).unwrap(),
        before
    );
}
