use crate::vault::{ImportEntryInput, VaultSession, FIELD_PASSWORD};
use tempfile::TempDir;

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
