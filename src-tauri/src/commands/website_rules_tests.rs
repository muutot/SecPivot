use super::*;
use crate::vault::{EntryInput, WebsiteMode};
use serde_json::{json, Value};
use std::path::Path;

fn write(uuid: &str, rules: Value) -> WebsiteRulesWrite {
    serde_json::from_value(json!({"uuid": uuid, "rules": rules})).unwrap()
}

fn policy() -> Value {
    json!({"version":1,"rules":[
        {"url":"https://synthetic.example:443/path","mode":"host"},
        {"url":"https://synthetic.example/exact?x=1","mode":"exact"},
        {"url":"https://other.example/","mode":"site"},
        {"url":"https://ignored.example/","mode":"never"}
    ]})
}

fn open(vaults: &VaultSessions, active: &Mutex<VaultSession>, path: &Path) -> (String, String) {
    let opened = vaults
        .open(&mut active.lock().unwrap(), |target| {
            target.create(path, "synthetic-master", "Aes", "Aes256", "None", None)?;
            let input: EntryInput = serde_json::from_value(json!({
                "groupUuid":"root", "title":"Synthetic", "username":"",
                "password":"synthetic-password", "url":"https://legacy.example", "notes":""
            }))
            .unwrap();
            target.add_entry(&input)
        })
        .unwrap();
    (opened.session_id, opened.state.root.entries[0].uuid.clone())
}

#[test]
fn required_nullable_write_contract_rejects_omission_and_sanitizes_bad_values() {
    for value in [
        json!({}),
        json!({"uuid":"synthetic"}),
        json!({"rules":null}),
        json!({"uuid":"synthetic","rules":null,"extra":true}),
    ] {
        assert!(serde_json::from_value::<WebsiteRulesWrite>(value).is_err());
    }
    assert!(write("synthetic", Value::Null).rules.0.is_none());
    let parsed = write("synthetic", policy()).rules.0.unwrap();
    assert_eq!(serde_json::to_value(&parsed).unwrap(), policy());
    assert!(parsed.rules[0].mode == WebsiteMode::Host);
    for rules in [
        json!("synthetic-private-marker"),
        json!(true),
        json!([]),
        json!({"version":1,"rules":[{"url":"synthetic-private-marker","mode":"unknown"}]}),
        json!({"version":1,"rules":[],"private":"synthetic-private-marker"}),
    ] {
        let error =
            serde_json::from_value::<WebsiteRulesWrite>(json!({"uuid":"synthetic","rules":rules}))
                .err()
                .unwrap();
        assert!(!error.to_string().contains("synthetic-private-marker"));
    }
}

#[test]
fn addressed_policy_write_and_read_keep_the_other_active_vault_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let (a, uuid) = open(&vaults, &active, &dir.path().join("a.kdbx"));
    let (b, _) = open(&vaults, &active, &dir.path().join("b.kdbx"));
    let before_b = serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap();
    let before_a = with_vault_session(&vaults, &active, Some(&a), |target| target.state())
        .unwrap()
        .unwrap();
    assert!(get_for_session(&vaults, &active, &a, &uuid)
        .unwrap()
        .is_none());
    let state = set_for_session(&vaults, &active, &a, write(&uuid, policy())).unwrap();
    assert_eq!(state.revision, before_a.revision + 1);
    assert!(state.root.entries[0].has_website_rules);
    assert!(state.root.entries[0]
        .custom_fields
        .iter()
        .all(|field| field.name != "SecPivot.WebsiteRules"));
    let wire = serde_json::to_value(&state).unwrap();
    assert_eq!(wire["root"]["entries"][0]["hasWebsiteRules"], true);
    assert!(!wire.to_string().contains("synthetic-password"));
    assert!(!wire.to_string().contains("synthetic.example"));
    assert_eq!(
        serde_json::to_value(
            get_for_session(&vaults, &active, &a, &uuid)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        policy()
    );
    assert_eq!(
        serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap(),
        before_b
    );
    assert!(get_for_session(&vaults, &active, &b, &uuid).is_err());
    assert_eq!(
        serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap(),
        before_b
    );
}

#[test]
fn explicit_clear_empty_and_no_op_preserve_distinct_policy_and_history_states() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let (id, uuid) = open(&vaults, &active, &dir.path().join("target.kdbx"));
    let before = active
        .lock()
        .unwrap()
        .get_entry_history(&uuid)
        .unwrap()
        .len();
    let changed = set_for_session(&vaults, &active, &id, write(&uuid, policy())).unwrap();
    let unchanged = set_for_session(&vaults, &active, &id, write(&uuid, policy())).unwrap();
    assert_eq!(changed.revision, unchanged.revision);
    let history = active.lock().unwrap().get_entry_history(&uuid).unwrap();
    assert_eq!(history.len(), before + 1);
    assert!(!history.last().unwrap().has_website_rules);
    let empty = json!({"version":1,"rules":[]});
    set_for_session(&vaults, &active, &id, write(&uuid, empty.clone())).unwrap();
    assert_eq!(
        serde_json::to_value(
            get_for_session(&vaults, &active, &id, &uuid)
                .unwrap()
                .unwrap()
        )
        .unwrap(),
        empty
    );
    assert!(active
        .lock()
        .unwrap()
        .get_entry_history(&uuid)
        .unwrap()
        .iter()
        .any(|version| version.has_website_rules));
    let history_wire =
        serde_json::to_string(&active.lock().unwrap().get_entry_history(&uuid).unwrap()).unwrap();
    assert!(!history_wire.contains("synthetic.example"));
    assert!(!history_wire.contains("synthetic-password"));
    let cleared = set_for_session(&vaults, &active, &id, write(&uuid, Value::Null)).unwrap();
    assert!(!cleared.root.entries[0].has_website_rules);
    assert!(get_for_session(&vaults, &active, &id, &uuid)
        .unwrap()
        .is_none());
}

#[test]
fn invalid_rules_or_closed_unknown_and_empty_origins_never_redirect_a_write() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let (a, uuid) = open(&vaults, &active, &dir.path().join("a.kdbx"));
    for rules in [
        json!({"version":2,"rules":[]}),
        json!({"version":1,"rules":[{"url":"synthetic-private-marker","mode":"host"}]}),
    ] {
        let before = serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap();
        let error = set_for_session(&vaults, &active, &a, write(&uuid, rules))
            .err()
            .unwrap();
        assert!(!error.contains("synthetic-private-marker"));
        assert_eq!(
            serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap(),
            before
        );
    }
    let (_, b_uuid) = open(&vaults, &active, &dir.path().join("b.kdbx"));
    vaults.close(&mut active.lock().unwrap(), Some(&a)).unwrap();
    let before = serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap();
    for id in [&a, "", "synthetic-unknown"] {
        assert!(get_for_session(&vaults, &active, id, &b_uuid).is_err());
        assert!(set_for_session(&vaults, &active, id, write(&b_uuid, Value::Null)).is_err());
        assert_eq!(
            serde_json::to_value(active.lock().unwrap().state().unwrap()).unwrap(),
            before
        );
    }
}

#[test]
fn foreign_malformed_policy_has_a_presence_hint_but_cannot_be_read_as_valid_rules() {
    let dir = tempfile::tempdir().unwrap();
    let vaults = VaultSessions::default();
    let active = Mutex::new(VaultSession::default());
    let (id, uuid) = open(&vaults, &active, &dir.path().join("foreign.kdbx"));
    let state = with_vault_session(&vaults, &active, Some(&id), |target| {
        target
            .require_db_mut()?
            .entry_mut(uuid::Uuid::parse_str(&uuid).unwrap().into())
            .unwrap()
            .set(
                "SecPivot.WebsiteRules",
                keepass::db::Value::protected("synthetic-private-marker".to_owned()),
            );
        target.mark_dirty();
        target.state()
    })
    .unwrap()
    .unwrap();
    assert!(state.root.entries[0].has_website_rules);
    assert!(!serde_json::to_string(&state)
        .unwrap()
        .contains("synthetic-private-marker"));
    let error = get_for_session(&vaults, &active, &id, &uuid).err().unwrap();
    assert!(!error.contains("synthetic-private-marker"));
    // Explicit clear remains a recoverable operation for foreign malformed data.
    let cleared = set_for_session(&vaults, &active, &id, write(&uuid, Value::Null)).unwrap();
    assert!(!cleared.root.entries[0].has_website_rules);
}
