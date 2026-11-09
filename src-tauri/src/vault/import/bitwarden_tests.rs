use crate::vault::{EntryInput, VaultSession, FIELD_PASSWORD};
use tempfile::TempDir;

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
    let input: EntryInput = serde_json::from_value(wire).unwrap();
    let mut session = VaultSession::default();
    session
        .create(&path, "synthetic-master", "Aes", "Aes256", "None", None)
        .unwrap();
    let state = session.add_entries(&[input]).unwrap();
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
