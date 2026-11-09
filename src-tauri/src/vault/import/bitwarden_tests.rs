use crate::vault::{ImportEntryInput, VaultSession, FIELD_PASSWORD};
use tempfile::TempDir;

#[test]
fn bitwarden_explicit_uri_modes_fail_closed_before_import_rows() {
    for mode in 0..=5 {
        let text = serde_json::json!({"items":[
            {"type":2,"name":"Earlier note","notes":"synthetic-sensitive-note"},
            {"type":1,"name":"synthetic-sensitive-title","login":{
                "password":"synthetic-sensitive-password",
                "uris":[{"uri":"https://synthetic-sensitive.example/login","match":mode}]
            }}
        ]})
        .to_string();
        let result = crate::commands::parse_bitwarden_json(text);
        assert!(result.is_err(), "mode {mode} was silently weakened");
        let error = result.err().unwrap();
        assert!(error.contains("第 2 条"));
        assert!(error.contains("当前不支持"));
        assert!(!error.contains("synthetic-sensitive"));
    }
}

#[test]
fn bitwarden_multiple_default_uris_fail_closed_instead_of_dropping_destinations() {
    for uris in [
        serde_json::json!([
            {"uri":"https://first.example.test/login"},
            {"uri":"https://second.example.test/login","match":null}
        ]),
        serde_json::json!([
            {"uri":null},{"uri":""},
            {"uri":"https://first.example.test/login"},
            {"uri":"https://second.example.test/login"}
        ]),
    ] {
        let text = serde_json::json!({"items":[
            {"type":2,"name":"Earlier note"},
            {"type":1,"name":"Multiple URIs","login":{"uris":uris}}
        ]})
        .to_string();
        let result = crate::commands::parse_bitwarden_json(text);
        assert!(result.is_err(), "additional destination was discarded");
        assert!(result.err().unwrap().contains("第 2 条"));
    }
}

#[test]
fn bitwarden_invalid_uri_shapes_never_create_matching_destinations() {
    for uris in [
        serde_json::json!({"uri":"synthetic-sensitive"}),
        serde_json::json!("synthetic-sensitive"),
        serde_json::json!([null]),
        serde_json::json!(["synthetic-sensitive"]),
        serde_json::json!([{"uri":42}]),
        serde_json::json!([{"uri":true}]),
        serde_json::json!([{"uri":"https://first.example.test","match":true}]),
        serde_json::json!([{"uri":"https://first.example.test","match":"5"}]),
        serde_json::json!([{"uri":"https://first.example.test","match":-1}]),
        serde_json::json!([{"uri":"https://first.example.test","match":6}]),
        serde_json::json!([{"uri":"https://first.example.test","match":0.5}]),
        serde_json::json!([{"uri":"first.example.test second.example.test"}]),
    ] {
        let text = serde_json::json!({"items":[{"type":1,"login":{"uris":uris}}]}).to_string();
        let result = crate::commands::parse_bitwarden_json(text);
        assert!(result.is_err(), "malformed URI was accepted");
        let error = result.err().unwrap();
        assert!(error.contains("网址格式无效"));
        assert!(!error.contains("synthetic-sensitive"));
        assert!(!error.contains("example.test"));
    }
}

#[test]
fn bitwarden_default_single_uri_and_empty_uri_shapes_survive_kdbx_reopen() {
    for (login, expected) in [
        (serde_json::json!({}), ""),
        (serde_json::json!({"uris":null}), ""),
        (serde_json::json!({"uris":[]}), ""),
        (
            serde_json::json!({"uris":[{"uri":null},{"uri":"","match":null}]}),
            "",
        ),
        (
            serde_json::json!({"uris":[{"uri":"https://account.example.test/login"}]}),
            "https://account.example.test/login",
        ),
        (
            serde_json::json!({"uris":[{}, {"uri":null}, {"uri":"https://account.example.test/login","match":null}]}),
            "https://account.example.test/login",
        ),
    ] {
        let text =
            serde_json::json!({"items":[{"type":1,"name":"Synthetic default URI","login":login}]})
                .to_string();
        let (_dir, session, id) = imported(&text);
        let entry = session
            .require_db()
            .unwrap()
            .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
            .unwrap();
        assert_eq!(entry.get_url().unwrap_or_default(), expected);
    }
}

#[test]
fn bitwarden_linked_card_identity_and_full_name_targets_follow_official_ids() {
    for (kind, section, first_id, keys) in [
        (
            3,
            "card",
            300,
            vec![
                "cardholderName",
                "expMonth",
                "expYear",
                "code",
                "brand",
                "number",
            ],
        ),
        (
            4,
            "identity",
            400,
            vec![
                "title",
                "middleName",
                "address1",
                "address2",
                "address3",
                "city",
                "state",
                "postalCode",
                "country",
                "company",
                "email",
                "phone",
                "ssn",
                "username",
                "passportNumber",
                "licenseNumber",
                "firstName",
                "lastName",
            ],
        ),
    ] {
        let mut item = serde_json::json!({"type":kind,"name":"Synthetic links"});
        let mut values = serde_json::Map::new();
        for key in &keys {
            values.insert(
                (*key).to_owned(),
                serde_json::json!(format!("synthetic-{key}")),
            );
        }
        let mut fields: Vec<_> = keys.iter().enumerate().map(|(index, _)| serde_json::json!({"name":format!("Alias {index}"),"type":3,"linkedId":first_id+index,"value":"synthetic-stale-value"})).collect();
        if kind == 4 {
            values.insert("title".into(), serde_json::json!("\u{feff} Dr "));
            values.insert("firstName".into(), serde_json::json!(" Synthetic "));
            values.insert("middleName".into(), serde_json::json!(" \t "));
            values.insert("lastName".into(), serde_json::json!(" Holder "));
            fields.push(serde_json::json!({"name":"Full Name","type":3,"linkedId":418}));
        }
        item["fields"] = serde_json::json!(fields);
        item[section] = serde_json::json!(values);
        let (_dir, session, id) = imported(&serde_json::json!({"items":[item]}).to_string());
        let db = session.require_db().unwrap();
        let entry = db
            .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
            .unwrap();
        for (index, key) in keys.iter().enumerate() {
            let stored = &entry.fields[&format!("Bitwarden Alias {index}")];
            assert!(stored.is_protected());
            assert_eq!(stored.get(), values[*key].as_str().unwrap());
        }
        if kind == 4 {
            assert_eq!(
                entry.fields["Bitwarden Full Name"].get(),
                "Dr   Synthetic   Holder"
            );
        }
    }
    // Recognized optional targets may legitimately be absent/null/empty.
    for value in [
        None,
        Some(serde_json::Value::Null),
        Some(serde_json::json!("")),
    ] {
        let mut login = serde_json::json!({});
        if let Some(value) = value {
            login["password"] = value;
        }
        let text = serde_json::json!({"items":[{"type":1,"name":"Synthetic empty link","login":login,"fields":[{"name":"Alias","type":3,"linkedId":101}]}]}).to_string();
        let (_dir, session, id) = imported(&text);
        let db = session.require_db().unwrap();
        let entry = db
            .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
            .unwrap();
        assert!(entry.fields["Bitwarden Alias"].is_protected());
        assert_eq!(entry.fields["Bitwarden Alias"].get(), "");
    }
}

#[test]
fn bitwarden_linked_fields_resolve_typed_values_as_protected_import_snapshots() {
    for (kind, section, values, linked_id, expected) in [
        (
            1,
            "login",
            serde_json::json!({"username":"synthetic-user","password":"synthetic-password\r\n"}),
            101,
            "synthetic-password\r\n",
        ),
        (
            1,
            "login",
            serde_json::json!({"username":"synthetic-user"}),
            100,
            "synthetic-user",
        ),
        (
            3,
            "card",
            serde_json::json!({"number":"00004242","code":"007"}),
            305,
            "00004242",
        ),
        (
            4,
            "identity",
            serde_json::json!({"ssn":"000-00-0000"}),
            412,
            "000-00-0000",
        ),
    ] {
        let mut item = serde_json::json!({"type":kind,"name":"Synthetic link","fields":[{"name":"Alias","type":3,"linkedId":linked_id,"value":null}]});
        item[section] = values;
        let text = serde_json::json!({"items":[item]}).to_string();
        let (_dir, session, id) = imported(&text);
        let db = session.require_db().unwrap();
        let entry = db
            .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
            .unwrap();
        let alias = &entry.fields["Bitwarden Alias"];
        assert!(
            alias.is_protected(),
            "linked credential alias was imported unprotected"
        );
        assert_eq!(alias.get(), expected, "linked alias lost its source value");
    }
}

#[test]
fn bitwarden_linked_fields_reject_unknown_or_malformed_targets_without_partial_rows() {
    for linked_id in [
        serde_json::json!(999),
        serde_json::json!(305),
        serde_json::json!("101"),
        serde_json::Value::Null,
    ] {
        let text = serde_json::json!({"items":[{"type":2,"name":"valid"},{"type":1,"name":"synthetic-error-sentinel","login":{"password":"synthetic-error-sentinel"},"fields":[{"name":"alias","type":3,"linkedId":linked_id}]}]}).to_string();
        let error = crate::commands::parse_bitwarden_json(text)
            .err()
            .expect("unresolved link was accepted as a blank field");
        assert!(error.contains('2'));
        assert!(!error.contains("synthetic-error-sentinel"));
    }
    for login in [
        serde_json::json!({"password": 101}),
        serde_json::json!("synthetic-error-sentinel"),
    ] {
        let text = serde_json::json!({"items":[{"type":2,"name":"valid"},{"type":1,"name":"synthetic-error-sentinel","login":login,"fields":[{"name":"alias","type":3,"linkedId":101}]}]}).to_string();
        let error = crate::commands::parse_bitwarden_json(text)
            .err()
            .expect("malformed linked target was accepted");
        assert!(error.contains('2'));
        assert!(!error.contains("synthetic-error-sentinel"));
    }
}

#[test]
fn import_quality_metadata_is_atomic_and_survives_kdbx_reopen() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-import-quality.kdbx");
    let row = |title: &str, quality: Option<bool>| {
        let mut wire = serde_json::json!({"groupUuid":"root","title":title,"username":"","password":"","url":"","notes":""});
        if let Some(quality) = quality {
            wire["qualityCheck"] = serde_json::json!(quality);
        }
        serde_json::from_value::<ImportEntryInput>(wire).unwrap()
    };
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let mut invalid = row("invalid", Some(false));
    invalid.input.group_uuid = "invalid-group".into();
    let before = session.state().unwrap().unwrap();
    assert!(session
        .import_entries(&[row("discarded", Some(false)), invalid])
        .is_err());
    let after = session.state().unwrap().unwrap();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.dirty, before.dirty);
    assert!(after.root.entries.is_empty());
    let state = session
        .import_entries(&[
            row("non-password", Some(false)),
            row("login", Some(true)),
            row("legacy", None),
        ])
        .unwrap();
    let ids: Vec<_> = state
        .root
        .entries
        .iter()
        .map(|entry| entry.uuid.clone())
        .collect();
    session.save().unwrap();
    session.close();
    let state = session.open(&path, "synthetic-master", None).unwrap();
    assert_eq!(
        state
            .root
            .entries
            .iter()
            .map(|entry| entry.quality_check)
            .collect::<Vec<_>>(),
        [false, true, true]
    );
    let report = session.security_report().unwrap();
    assert_eq!(report.total, 3);
    assert_eq!(report.empty, ids[1..]);
    assert!(report.weak.is_empty());
    let mut invalid = serde_json::json!({"groupUuid":"root","title":"","username":"","password":"","url":"","notes":"","qualityCheck":"false"});
    assert!(serde_json::from_value::<ImportEntryInput>(invalid.take()).is_err());
}

#[test]
fn bitwarden_typed_records_survive_command_wire_and_kdbx_reopen() {
    let specifications = [
        (
            3,
            "card",
            serde_json::json!({"cardholderName":"Synthetic Holder","number":" 00004242 ","code":"007","expMonth":"03"}),
        ),
        (
            4,
            "identity",
            serde_json::json!({"firstName":"Synthetic","ssn":"000-00-0000","passportNumber":"0000123","address1":"Synthetic road"}),
        ),
        (
            5,
            "sshKey",
            serde_json::json!({"privateKey":"synthetic-private\r\nkey","publicKey":"synthetic-public-key","keyFingerprint":"synthetic-fingerprint"}),
        ),
        (
            6,
            "bankAccount",
            serde_json::json!({"bankName":"Synthetic Bank","accountNumber":"0000123","routingNumber":"0000000","pin":"007"}),
        ),
        (
            7,
            "driversLicense",
            serde_json::json!({"firstName":"Synthetic","licenseNumber":"000099","dateOfBirth":"2000-01-01"}),
        ),
        (
            8,
            "passport",
            serde_json::json!({"givenName":"Synthetic","passportNumber":"000088","nationalIdentificationNumber":"000077"}),
        ),
    ];
    for (kind, section, contents) in specifications {
        let mut item = serde_json::json!({
            "type":kind, "name":format!("Synthetic type {kind}"),"notes":"Synthetic notes",
            "folderId":"f1", "fields":[{"name":"PIN","value":"synthetic-custom","type":1}]
        });
        item[section] = contents.clone();
        let text = serde_json::json!({"encrypted":false,"folders":[{"id":"f1","name":"Synthetic Group"}],"items":[item]}).to_string();
        let rows = crate::commands::parse_bitwarden_json(text.clone()).unwrap();
        assert_eq!(rows.len(), 1, "a supported typed record was discarded");
        assert_eq!(rows[0].group, "Synthetic Group");
        let (_dir, session, id) = imported(&text);
        assert!(
            session.security_report().unwrap().empty.is_empty(),
            "a typed non-password record was reported as an empty password"
        );
        let db = session.require_db().unwrap();
        let entry = db
            .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
            .unwrap();
        assert_eq!(entry.get("Notes"), Some("Synthetic notes"));
        for value in contents.as_object().unwrap().values() {
            assert!(
                entry
                    .fields
                    .values()
                    .any(|stored| stored.get() == value.as_str().unwrap() && stored.is_protected()),
                "a typed value or protection flag was lost"
            );
        }
        assert!(entry
            .fields
            .values()
            .any(|v| v.get() == "synthetic-custom" && v.is_protected()));
    }
}

#[test]
fn bitwarden_typed_records_validate_sections_before_returning_rows() {
    for bad in [
        serde_json::json!({"type":99,"name":"synthetic-error-sentinel"}),
        serde_json::json!({"type":3,"name":"synthetic-error-sentinel","card":"synthetic-error-sentinel"}),
        serde_json::json!({"type":3,"name":"synthetic-error-sentinel","card":{"number":42}}),
    ] {
        let text =
            serde_json::json!({"items":[{"type":2,"name":"Synthetic note"},bad]}).to_string();
        let error = crate::commands::parse_bitwarden_json(text)
            .err()
            .expect("unsupported or malformed record was silently accepted");
        assert!(error.contains('2'));
        assert!(!error.contains("synthetic-error-sentinel"));
    }
}

#[test]
fn bitwarden_typed_record_extension_fields_keep_nested_data_and_unique_names() {
    let text = r#"{"items":[{"type":3,"name":"Synthetic extensions","card":{
        "number":"00004242","extra":{"secret":"synthetic-nested"},"futureArray":["synthetic-array"],"empty":null
    },"fields":[{"name":"Card Number","value":"synthetic-collision","type":1}]}]}"#;
    let (_dir, session, id) = imported(text);
    let db = session.require_db().unwrap();
    let entry = db
        .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
        .unwrap();
    for expected in [
        "00004242",
        "synthetic-collision",
        r#"{"secret":"synthetic-nested"}"#,
        r#"["synthetic-array"]"#,
    ] {
        assert!(
            entry
                .fields
                .values()
                .any(|v| v.get() == expected && v.is_protected()),
            "typed extension or colliding value was lost"
        );
    }
}

/// Real parser command -> ImportRow wire fields -> bulk mutation -> KDBX reopen.
/// The group binding mirrors io.importEntries; all data is synthetic.
fn imported(text: &str) -> (TempDir, VaultSession, String) {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("synthetic-bitwarden.kdbx");
    let mut rows = crate::commands::parse_bitwarden_json(text.to_owned()).unwrap();
    assert_eq!(rows.len(), 1);
    let mut wire = serde_json::to_value(rows.remove(0)).unwrap();
    wire["groupUuid"] = serde_json::json!("root");
    wire["attachments"] = serde_json::json!([]);
    let input: ImportEntryInput = serde_json::from_value(wire).unwrap();
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let state = session.import_entries(&[input]).unwrap();
    let id = state.root.entries[0].uuid.clone();
    assert!(state.root.entries[0]
        .custom_fields
        .iter()
        .all(|f| !f.protected || f.value.is_empty()));
    session.save().unwrap();
    session.close();
    session.open(&path, "synthetic-master", None).unwrap();
    (dir, session, id)
}

#[test]
fn bitwarden_password_history_survives_wire_and_kdbx_as_protected_source_data() {
    let history = serde_json::json!([
        {"password":"prior-synthetic-α", "lastUsedDate":"2025-03-01T10:20:30.456Z"},
        {"password":"prior-synthetic-α", "lastUsedDate":"2024-03-01T10:20:30+08:00"},
        {"password":"", "lastUsedDate":null},
        {"password":"older-synthetic", "sourceExtension":{"label":"preserve"}}
    ]);
    let text = serde_json::json!({"encrypted":false,"items":[{
        "type":1,"name":"Synthetic history", "passwordHistory":history,
        "login":{"password":"current-synthetic"}
    }]})
    .to_string();
    let (_dir, mut session, id) = imported(&text);
    let state = session.state().unwrap().unwrap();
    let field = state.root.entries[0]
        .custom_fields
        .iter()
        .find(|field| field.name == "Bitwarden Password History");
    assert!(field.is_some(), "source password history was discarded");
    let field = field.unwrap();
    assert!(field.protected);
    assert!(field.value.is_empty());
    let stored = session
        .get_custom_field_value(&id, &field.name)
        .unwrap()
        .unwrap();
    let stored: serde_json::Value = serde_json::from_str(&stored).unwrap();
    assert_eq!(stored, history);
    assert_eq!(
        session.get_entry_password(&id).unwrap(),
        "current-synthetic"
    );
    assert!(
        session.get_entry_history(&id).unwrap().is_empty(),
        "password-only source data must not invent full native entry versions"
    );
    let wire = serde_json::to_string(&state).unwrap();
    assert!(!wire.contains("prior-synthetic"));
    assert!(!wire.contains("older-synthetic"));
}

#[test]
fn bitwarden_password_history_does_not_replace_a_colliding_custom_field() {
    let (_dir, mut session, id) = imported(
        r#"{"items":[{
        "type":1,"name":"Synthetic collision",
        "fields":[{"name":"Password History","value":"synthetic custom value","type":0}],
        "passwordHistory":[{"password":"prior-synthetic","lastUsedDate":"2025-01-01T00:00:00Z"}]
    }]}"#,
    );
    assert_eq!(
        session
            .get_custom_field_value(&id, "Bitwarden Password History")
            .unwrap()
            .as_deref(),
        Some("synthetic custom value")
    );
    let state = session.state().unwrap().unwrap();
    let archive = state.root.entries[0]
        .custom_fields
        .iter()
        .find(|field| field.name == "Bitwarden Password History (2)");
    assert!(
        archive.is_some(),
        "source archive lost after custom-label collision"
    );
    assert!(archive.unwrap().protected);
    let value = session
        .get_custom_field_value(&id, "Bitwarden Password History (2)")
        .unwrap()
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&value).unwrap(),
        serde_json::json!([{"password":"prior-synthetic","lastUsedDate":"2025-01-01T00:00:00Z"}])
    );
}

#[test]
fn bitwarden_password_history_rejects_malformed_values_for_the_whole_document() {
    for history in [
        serde_json::json!("synthetic-marker"),
        serde_json::json!(1),
        serde_json::json!({}),
        serde_json::json!([null]),
        serde_json::json!([{}]),
        serde_json::json!([{"password":null}]),
        serde_json::json!([{"password":7}]),
        serde_json::json!([{"password":"synthetic-marker", "lastUsedDate":7}]),
    ] {
        let text = serde_json::json!({"items":[
            {"type":1,"name":"Valid first"},
            {"type":1,"name":"Invalid second", "passwordHistory":history}
        ]})
        .to_string();
        let result = crate::commands::parse_bitwarden_json(text);
        assert!(
            result.is_err(),
            "malformed source history was silently discarded"
        );
        let error = result.err().unwrap();
        assert!(!error.contains("synthetic-marker"));
        assert!(error.contains("第 2 条"));
    }
}

#[test]
fn bitwarden_password_history_empty_sources_add_no_archive() {
    for item in [
        serde_json::json!({"type":1}),
        serde_json::json!({"type":1,"passwordHistory":null}),
        serde_json::json!({"type":1,"passwordHistory":[]}),
    ] {
        let rows =
            crate::commands::parse_bitwarden_json(serde_json::json!({"items":[item]}).to_string())
                .unwrap();
        assert!(rows[0].custom_fields.is_empty());
    }
}

#[test]
fn bitwarden_custom_reserved_and_blank_names_survive_kdbx_import() {
    let (_dir, session, id) = imported(
        r#"{"encrypted":false,"items":[{
      "type":1,"name":"Synthetic fields","login":{"password":"main-synthetic"},
      "fields":[
        {"name":"Password","value":"custom-synthetic","type":1},
        {"name":" otp ","value":"not-an-otp","type":1},
        {"name":"SecPivot.Favorite","value":"favorite-synthetic","type":0},
        {"name":"SecPivot.OriginalGroup","value":"group-synthetic","type":0},
        {"name":"","value":"unnamed-synthetic","type":1},
        {"name":null,"value":"null-name-synthetic","type":1},
        {"value":"missing-name-synthetic","type":1}
      ]
    }]}"#,
    );
    let db = session.require_db().unwrap();
    let entry = db
        .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
        .unwrap();
    assert_eq!(entry.fields[FIELD_PASSWORD].get(), "main-synthetic");
    for (expected, hidden) in [
        ("custom-synthetic", true),
        ("not-an-otp", true),
        ("favorite-synthetic", false),
        ("group-synthetic", false),
        ("unnamed-synthetic", true),
        ("null-name-synthetic", true),
        ("missing-name-synthetic", true),
    ] {
        assert!(
            entry
                .fields
                .values()
                .any(|v| v.get() == expected && v.is_protected() == hidden),
            "an imported custom value or protection flag was lost"
        );
    }
}

#[test]
fn bitwarden_duplicate_custom_labels_keep_every_value_after_reopen() {
    let (_dir, session, id) = imported(
        r#"{"items":[{
      "type":2,"name":"Synthetic duplicates","fields":[
        {"name":"PIN","value":"first-synthetic","type":1},
        {"name":" PIN ","value":"second-synthetic","type":1},
        {"name":"PIN (2)","value":"third-synthetic","type":0},
        {"name":"PIN","value":"fourth-synthetic","type":1}
      ]
    }]}"#,
    );
    let db = session.require_db().unwrap();
    let entry = db
        .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
        .unwrap();
    for expected in [
        "first-synthetic",
        "second-synthetic",
        "third-synthetic",
        "fourth-synthetic",
    ] {
        assert!(
            entry.fields.values().any(|v| v.get() == expected),
            "a duplicate field value was lost"
        );
    }
}

#[test]
fn bitwarden_custom_labels_do_not_activate_native_otp() {
    let (_dir, session, id) = imported(
        r#"{"items":[{
      "type":1,"name":"Synthetic aliases","login":{
        "totp":"JBSWY3DPEHPK3PXP","uris":[{"uri":"https://correct.example"}]
      },"fields":[
        {"name":"HmacOtp","value":"JBSWY3DPEHPK3PXP:6:42","type":1}
      ]
    }]}"#,
    );
    let db = session.require_db().unwrap();
    let entry = db
        .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
        .unwrap();
    let spec = crate::vault::helpers::parse_entry_otp_spec(&entry).unwrap();
    assert_eq!(
        spec.kind,
        crate::crypto::otp::OtpKind::Totp,
        "a custom label shadowed the imported authenticator"
    );
    assert_eq!(
        crate::vault::helpers::entry_primary_url(&entry),
        "https://correct.example"
    );
    assert!(crate::vault::helpers::entry_match_urls(&entry)
        .iter()
        .all(|url| url == "https://correct.example"));
}

#[test]
fn bitwarden_custom_labels_do_not_activate_native_url_settings() {
    let mut failures = 0;
    for field in [
        serde_json::json!({"name":"OverrideURL","value":"https://wrong.example","type":0}),
        serde_json::json!({"name":"KPRPC JSON","value":"{\"altURLs\":[\"https://extra.example\"]}","type":1}),
    ] {
        let text = serde_json::json!({"items":[{
            "type":1,"name":"Synthetic URL labels","login":{
                "uris":[{"uri":"https://correct.example"}]
            },"fields":[field]
        }]})
        .to_string();
        let (_dir, session, id) = imported(&text);
        let db = session.require_db().unwrap();
        let entry = db
            .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
            .unwrap();
        if crate::vault::helpers::entry_primary_url(&entry) != "https://correct.example"
            || crate::vault::helpers::entry_match_urls(&entry)
                .iter()
                .any(|url| url != "https://correct.example")
        {
            failures += 1;
        }
    }
    assert_eq!(failures, 0, "custom labels activated native URL matching");
}

#[test]
fn bitwarden_favorites_survive_parser_wire_and_kdbx_reopen() {
    for (source, expected) in [
        (serde_json::json!(true), true),
        (serde_json::json!(false), false),
        (serde_json::Value::Null, false),
    ] {
        let mut item = serde_json::json!({"type":1,"name":"Synthetic favorite","login":{"password":"synthetic-password"},"fields":[{"name":"SecPivot.Favorite","value":"true","type":0}]});
        if !source.is_null() {
            item["favorite"] = source;
        }
        let text = serde_json::json!({"encrypted":false,"items":[item]}).to_string();
        let (_dir, mut session, id) = imported(&text);
        let state = session.state().unwrap().unwrap();
        assert_eq!(
            state.root.entries[0].favorite, expected,
            "source favorite was discarded"
        );
        let db = session.require_db().unwrap();
        let entry = db
            .entry(crate::vault::helpers::parse_entry_id(&id).unwrap())
            .unwrap();
        assert_eq!(
            entry.get(crate::vault::FIELD_FAVORITE) == Some(crate::vault::FIELD_FAVORITE_TRUE),
            expected
        );
        assert!(
            entry
                .fields
                .values()
                .any(|f| f.get() == "true" && !f.is_protected()),
            "source custom field was discarded"
        );
    }
}

#[test]
fn bitwarden_favorite_requires_a_boolean_before_returning_any_rows() {
    for favorite in [
        serde_json::json!("true"),
        serde_json::json!(1),
        serde_json::json!({}),
        serde_json::Value::Null,
    ] {
        let text=serde_json::json!({"items":[{"type":2,"name":"Earlier"},{"type":1,"name":"synthetic-content-marker","favorite":favorite}]}).to_string();
        let result = crate::commands::parse_bitwarden_json(text);
        assert!(result.is_err(), "invalid favorite silently accepted");
        let error = result.err().unwrap();
        assert!(error.contains("第 2 条"));
        assert!(!error.contains("synthetic-content-marker"));
    }
}

#[test]
fn import_favorite_metadata_validates_the_batch_and_keeps_ordinary_edit_contract() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("favorite-batch.kdbx");
    let row = |name: &str, favorite: Option<bool>| {
        let mut wire = serde_json::json!({"groupUuid":"root","title":name,"username":"","password":"","url":"","notes":""});
        if let Some(favorite) = favorite {
            wire["favorite"] = serde_json::json!(favorite);
        }
        serde_json::from_value::<ImportEntryInput>(wire).unwrap()
    };
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let mut invalid = row("invalid", Some(false));
    invalid.input.group_uuid = "invalid-group".into();
    let before = session.state().unwrap().unwrap();
    assert!(session
        .import_entries(&[row("discarded", Some(true)), invalid])
        .is_err());
    let after = session.state().unwrap().unwrap();
    assert_eq!(after.revision, before.revision);
    assert_eq!(after.dirty, before.dirty);
    assert!(after.root.entries.is_empty());
    let state = session
        .import_entries(&[
            row("favorite", Some(true)),
            row("ordinary", Some(false)),
            row("legacy", None),
        ])
        .unwrap();
    assert_eq!(
        state
            .root
            .entries
            .iter()
            .map(|e| e.favorite)
            .collect::<Vec<_>>(),
        [true, false, false]
    );
    let id = state.root.entries[0].uuid.clone();
    let mut edit = row("edited", Some(false)).input;
    edit.notes = "synthetic changed notes".into();
    session.update_entry(&id, &edit).unwrap();
    session.save().unwrap();
    session.close();
    let state = session.open(&path, "synthetic-master", None).unwrap();
    assert_eq!(
        state
            .root
            .entries
            .iter()
            .map(|e| e.favorite)
            .collect::<Vec<_>>(),
        [true, false, false],
        "ordinary edit reset favorite"
    );
    assert!(serde_json::from_value::<ImportEntryInput>(serde_json::json!({"groupUuid":"root","title":"","username":"","password":"","url":"","notes":"","favorite":"false"})).is_err());
}
