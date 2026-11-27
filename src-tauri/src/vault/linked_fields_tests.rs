use super::*;
use crate::vault::{EntryPatch, EntrySaveRequest, VaultSession};
use keepass::Database;
use serde_json::json;

fn bindings() -> LinkedFieldSet {
    LinkedFieldSet {
        version: 1,
        links: vec![LinkedField {
            name: "Synthetic alias".into(),
            target: LinkedFieldTarget::Field {
                name: FIELD_PASSWORD.into(),
            },
        }],
    }
}

fn session() -> (tempfile::TempDir, VaultSession, std::path::PathBuf, String) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("synthetic.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let state = session.add_entry(&serde_json::from_value(json!({"groupUuid":"root", "title":"Synthetic linked entry", "username":"synthetic-user", "password":"synthetic-old", "url":"https://synthetic.example", "notes":"", "customFields":[
      {"name":"Synthetic alias","value":"synthetic-old","protected":true},
      {"name":FIELD_LINKED_FIELDS,"value":bindings().as_json().unwrap().as_str(),"protected":true}
    ]})).unwrap()).unwrap();
    (dir, session, path, state.root.entries[0].uuid.clone())
}

fn editor(uuid: Option<&str>, value: &str, links: serde_json::Value) -> EntrySaveRequest {
    serde_json::from_value(json!({"target":uuid.map(|uuid|json!({"mode":"edit","uuid":uuid})).unwrap_or(json!({"mode":"create"})),
      "input":{"groupUuid":"root", "title":"Synthetic linked entry", "username":"synthetic-user", "password":value, "url":"https://synthetic.example", "notes":"", "customFields":[{"name":"Synthetic alias","value":value,"protected":true}]}, "linkedFields":links})).unwrap()
}

#[test]
fn linked_editor_aliases_history_restore_kdbx_xml_and_exchange_preserve_live_targets() {
    let (_dir, mut session, path, uuid) = session();
    let revision = session.revision;
    let saved = session
        .save_entry(&editor(
            Some(&uuid),
            "synthetic-new",
            json!({"mode":"set","fields":bindings()}),
        ))
        .unwrap();
    assert_eq!(saved.state.revision, revision + 1);
    assert_eq!(session.get_entry_history(&uuid).unwrap().len(), 1);
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-new")
    );
    let masked = serde_json::to_string(&saved.state).unwrap();
    assert!(!masked.contains("synthetic-new") && !masked.contains(FIELD_LINKED_FIELDS));
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-new")
    );
    let id = crate::vault::helpers::parse_entry_id(&uuid).unwrap();
    let document =
        crate::vault::exchange::build_entries_document(session.require_db().unwrap(), &[id])
            .unwrap();
    let pasted = crate::vault::exchange::parse_entries_document(&document).unwrap();
    let copy_id = crate::vault::exchange::insert_entry(
        &mut session.require_db_mut().unwrap().root_mut(),
        &pasted[0],
    )
    .unwrap();
    session
        .update_entries(
            &[copy_id.to_string()],
            &EntryPatch {
                password: Some("synthetic-copy".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        session
            .get_custom_field_value(&copy_id.to_string(), "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-copy")
    );
    let xml = session.export_xml_content().unwrap();
    let plan = crate::vault::prepare_keepass_xml_import(xml.as_bytes()).unwrap();
    let lifetime = session.native_import_lifetime("root").unwrap();
    session.import_keepass_xml(plan, "root", lifetime).unwrap();
    let copies = session
        .require_db()
        .unwrap()
        .iter_all_entries()
        .filter(|e| e.fields.contains_key(FIELD_LINKED_FIELDS))
        .count();
    assert_eq!(copies, 4);
    for e in session
        .require_db()
        .unwrap()
        .iter_all_entries()
        .filter(|e| e.fields.contains_key(FIELD_LINKED_FIELDS))
    {
        assert!(
            e.fields[FIELD_LINKED_FIELDS].is_protected()
                && e.fields["Synthetic alias"].is_protected()
        );
        assert_eq!(
            value(&e, "Synthetic alias").unwrap().as_deref(),
            e.get(FIELD_PASSWORD)
        );
    }
    session
        .save_entry(&editor(
            Some(&uuid),
            "synthetic-new",
            json!({"mode":"clear"}),
        ))
        .unwrap();
    let history = session.get_entry_history(&uuid).unwrap();
    let linked_version = history
        .iter()
        .find(|h| h.linked_fields.is_some() && h.diff.linked_fields && !h.diff.password)
        .unwrap();
    assert!(linked_version.diff.custom_fields.is_empty());
    session
        .restore_entry_version(&uuid, linked_version.index, &linked_version.selection_token)
        .unwrap();
    session
        .update_entries(
            std::slice::from_ref(&uuid),
            &EntryPatch {
                password: Some("synthetic-restored".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-restored")
    );
}

#[test]
fn linked_alias_literal_changes_detach_and_invalid_editor_sets_publish_nothing() {
    let (_dir, mut session, _path, uuid) = session();
    let before = session.snapshot_without_icons().unwrap();
    let revision = session.revision;
    let mut invalid = bindings();
    invalid.links[0].target = LinkedFieldTarget::Field {
        name: "Synthetic alias".into(),
    };
    assert!(session
        .save_entry(&editor(
            Some(&uuid),
            "would-change",
            json!({"mode":"set","fields":invalid})
        ))
        .is_err());
    assert_eq!(session.revision, revision);
    assert_eq!(
        serde_json::to_value(session.snapshot_without_icons().unwrap()).unwrap(),
        serde_json::to_value(before).unwrap()
    );
    session
        .update_custom_field(&uuid, "Synthetic alias", "synthetic-independent", true)
        .unwrap();
    session
        .update_entries(
            std::slice::from_ref(&uuid),
            &EntryPatch {
                password: Some("synthetic-target".into()),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-independent")
    );
    assert!(session.snapshot_without_icons().unwrap().root.entries[0]
        .linked_fields
        .is_none());
    assert!(session
        .save_entry(&editor(
            None,
            "synthetic-create",
            json!({"mode":"set","fields":bindings()})
        ))
        .is_ok());
}

#[test]
fn linked_bridge_and_legacy_full_edits_update_aliases_with_one_history_transaction() {
    use crate::bridge::BridgeHost;
    let (_dir, mut session, _path, uuid) = session();
    session
        .set_login(
            "synthetic-user",
            "synthetic-bridge",
            "https://synthetic.example",
            Some(&uuid),
        )
        .unwrap();
    assert_eq!(session.get_entry_history(&uuid).unwrap().len(), 1);
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-bridge")
    );
    // The old stored alias may be submitted while the target changes; it remains linked.
    let input = serde_json::from_value(json!({"groupUuid":"root", "title":"Synthetic linked entry", "username":"synthetic-user", "password":"synthetic-full", "url":"https://synthetic.example", "notes":"", "customFields":[{"name":"Synthetic alias","value":"synthetic-bridge","protected":true}]})).unwrap();
    session.update_entry(&uuid, &input).unwrap();
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-full")
    );
}

#[test]
fn linked_rpc_writes_sync_credentials_detach_literal_aliases_and_refuse_managed_metadata() {
    use crate::rpc::{RpcFieldWrite, RpcHost, RpcLoginWrite};
    let (_dir, mut session, path, uuid) = session();
    session.save().unwrap();
    let mut login = RpcLoginWrite {
        title: "Synthetic linked entry".into(),
        urls: vec!["https://synthetic.example".into()],
        form_field_list: vec![RpcFieldWrite {
            field_type: "FFTpassword".into(),
            value: "synthetic-rpc".into(),
            ..Default::default()
        }],
        ..Default::default()
    };
    session.update_login(&login, &uuid, 4).unwrap();
    assert_eq!(session.get_entry_history(&uuid).unwrap().len(), 1);
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-rpc")
    );
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-rpc")
    );
    let bytes = std::fs::read(&path).unwrap();
    let revision = session.revision;
    login.form_field_list.push(RpcFieldWrite {
        display_name: FIELD_LINKED_FIELDS.into(),
        field_type: "FFTtext".into(),
        value: "{}".into(),
        ..Default::default()
    });
    assert!(session.update_login(&login, &uuid, 4).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(session.revision, revision);
    login.form_field_list.last_mut().unwrap().display_name = "Synthetic alias".into();
    login.form_field_list.last_mut().unwrap().value = "synthetic-independent".into();
    session.update_login(&login, &uuid, 4).unwrap();
    assert!(session.snapshot_without_icons().unwrap().root.entries[0]
        .linked_fields
        .is_none());
    login.form_field_list.truncate(1);
    login.form_field_list[0].value = "synthetic-new-rpc".into();
    session.update_login(&login, &uuid, 4).unwrap();
    assert_eq!(
        session
            .get_custom_field_value(&uuid, "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-independent")
    );
    let id = crate::vault::helpers::parse_entry_id(&uuid).unwrap();
    session
        .require_db_mut()
        .unwrap()
        .entry_mut(id)
        .unwrap()
        .set(FIELD_LINKED_FIELDS, Value::protected("{}"));
    let before = session.require_db().unwrap().clone();
    assert!(session.update_login(&login, &uuid, 4).is_err());
    assert_eq!(session.require_db().unwrap(), &before);
}

#[test]
fn foreign_unprotected_aliases_remain_masked_and_malformed_metadata_never_evaluates() {
    let mut db = Database::new();
    let mut root = db.root_mut();
    let mut entry = root.add_entry();
    entry.set(FIELD_PASSWORD, Value::protected("synthetic-target"));
    entry.set("Synthetic alias", Value::unprotected("synthetic-stale"));
    entry.set(
        FIELD_LINKED_FIELDS,
        Value::unprotected(bindings().as_json().unwrap().as_str()),
    );
    let snapshot = crate::vault::serialize::build_entry(&entry.as_ref(), "root");
    assert!(snapshot.custom_fields[0].protected && snapshot.custom_fields[0].value.is_empty());
    assert_eq!(
        value(&entry.as_ref(), "Synthetic alias")
            .unwrap()
            .as_deref(),
        Some("synthetic-target")
    );
    entry.set(FIELD_LINKED_FIELDS, Value::unprotected("{}"));
    assert!(value(&entry.as_ref(), "Synthetic alias").is_err());
    refresh(&mut entry);
    assert_eq!(entry.get("Synthetic alias"), Some("synthetic-stale"));
    assert!(
        crate::vault::serialize::build_entry(&entry.as_ref(), "root").custom_fields[0]
            .value
            .is_empty()
    );
}

#[test]
fn malformed_or_missing_link_metadata_rejects_native_exchange_before_insertion() {
    let (_dir, mut session, _path, uuid) = session();
    let id = crate::vault::helpers::parse_entry_id(&uuid).unwrap();
    let document =
        crate::vault::exchange::build_entries_document(session.require_db().unwrap(), &[id])
            .unwrap();
    let mut parsed = crate::vault::exchange::parse_entries_document(&document).unwrap();
    let field = parsed[0]
        .fields
        .iter_mut()
        .find(|f| f.0 == FIELD_LINKED_FIELDS)
        .unwrap();
    field.1 = "{}".into();
    let before = session.require_db().unwrap().iter_all_entries().count();
    assert!(crate::vault::exchange::insert_entry(
        &mut session.require_db_mut().unwrap().root_mut(),
        &parsed[0]
    )
    .is_err());
    assert_eq!(
        session.require_db().unwrap().iter_all_entries().count(),
        before
    );
    let mut parsed = crate::vault::exchange::parse_entries_document(&document).unwrap();
    parsed[0].fields.retain(|f| f.0 != "Synthetic alias");
    assert!(crate::vault::exchange::insert_entry(
        &mut session.require_db_mut().unwrap().root_mut(),
        &parsed[0]
    )
    .is_err());
    let text = String::from_utf8(document).unwrap();
    let malformed = text.replace(bindings().as_json().unwrap().as_str(), "{}");
    assert_ne!(malformed, text);
    assert!(crate::vault::exchange::parse_entries_document(malformed.as_bytes()).is_err());
    // Native complete XML validates current and historical metadata too.
    session
        .require_db_mut()
        .unwrap()
        .entry_mut(id)
        .unwrap()
        .set(FIELD_LINKED_FIELDS, Value::protected("{}"));
    let xml = session.export_xml_content().unwrap();
    assert!(crate::vault::prepare_keepass_xml_import(xml.as_bytes()).is_err());
}

#[test]
fn current_target_is_read_literally_and_stored_alias_updates_inside_the_same_transaction() {
    let mut db = Database::new();
    let id = {
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        entry.set(FIELD_PASSWORD, Value::protected("synthetic-old"));
        entry.set("Synthetic alias", Value::protected("synthetic-old"));
        set(&mut entry, Some(&bindings()));
        entry.id()
    };
    {
        let mut entry = db.entry_mut(id).unwrap();
        entry.edit_tracking(|tracked| {
            let mut current = tracked.as_mut();
            current.set(
                FIELD_PASSWORD,
                Value::protected("{REF:P@I:synthetic}{CMD:synthetic}"),
            );
            refresh(&mut current);
        });
    }
    let entry = db.entry(id).unwrap();
    assert_eq!(
        value(&entry, "Synthetic alias").unwrap().as_deref(),
        Some("{REF:P@I:synthetic}{CMD:synthetic}")
    );
    assert_eq!(entry.get("Synthetic alias"), entry.get(FIELD_PASSWORD));
    assert!(entry.fields["Synthetic alias"].is_protected());
    assert_eq!(
        entry.historical(0).unwrap().get("Synthetic alias"),
        Some("synthetic-old")
    );
    let snapshot = super::super::serialize::build_entry(&entry, "synthetic-group");
    assert_eq!(
        snapshot
            .custom_fields
            .iter()
            .find(|f| f.name == "Synthetic alias")
            .unwrap()
            .value,
        ""
    );
    assert_eq!(snapshot.linked_fields.unwrap(), bindings());
    assert!(!snapshot
        .custom_fields
        .iter()
        .any(|f| f.name == FIELD_LINKED_FIELDS));
}

#[test]
fn malformed_cycles_reserved_targets_duplicates_and_oversized_metadata_fail_closed() {
    for raw in [
        "{}",
        r#"{"version":2,"links":[]}"#,
        r#"{"version":1,"links":[],"other":true}"#,
        r#"{"version":1,"links":[{"name":"Synthetic alias","target":{"kind":"field","name":"Synthetic alias"}}]}"#,
        r#"{"version":1,"links":[{"name":"Synthetic alias","target":{"kind":"field","name":"otp"}}]}"#,
    ] {
        assert!(LinkedFieldSet::from_json(raw).is_err());
    }
    let mut duplicated = bindings();
    duplicated.links.push(duplicated.links[0].clone());
    assert!(duplicated.validate().is_err());
    assert!(LinkedFieldSet::from_json(&" ".repeat(MAX_BYTES + 1)).is_err());
}

#[test]
fn full_name_uses_raw_local_components_and_link_edits_detach_without_affecting_other_bindings() {
    let mut db = Database::new();
    let mut root = db.root_mut();
    let mut entry = root.add_entry();
    entry.set("title", Value::protected("  Dr "));
    entry.set("first", Value::protected("  Synthetic  "));
    entry.set("last", Value::protected("Person "));
    entry.set("Full alias", Value::protected(""));
    entry.set("Synthetic alias", Value::protected("synthetic-old"));
    entry.set(FIELD_PASSWORD, Value::protected("synthetic-old"));
    let mut links = bindings();
    links.links.push(LinkedField {
        name: "Full alias".into(),
        target: LinkedFieldTarget::BitwardenFullName {
            fields: [
                "title".into(),
                "first".into(),
                "middle".into(),
                "last".into(),
            ],
        },
    });
    set(&mut entry, Some(&links));
    assert_eq!(
        value(&entry.as_ref(), "Full alias").unwrap().as_deref(),
        Some("Dr    Synthetic   Person")
    );
    detach(&mut entry, "Synthetic alias").unwrap();
    entry.set(FIELD_PASSWORD, Value::protected("synthetic-new"));
    refresh(&mut entry);
    assert_eq!(entry.get("Synthetic alias"), Some("synthetic-old"));
    assert_eq!(read(&entry.as_ref()).unwrap().unwrap().links.len(), 1);
}
