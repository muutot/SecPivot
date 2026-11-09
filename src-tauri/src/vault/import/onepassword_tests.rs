//! Synthetic fixtures follow the standard/Windows schemas in Bitwarden's
//! 1PIF importer at 1a8bdbfe625bff90a9c0cabef2002e37f6038fa1.
//! No real credentials, native export or licensed fixture text is copied.

use super::parse_1pif;
use crate::vault::{CustomField, EntryInput, VaultSession, ROOT_GROUP_UUID};

const STANDARD: &str = r#"{"typeName":"webforms.WebForm","title":"测试 Login","location":"https://one.example","secureContents":{"fields":[{"name":"email","designation":"username","value":" demo "},{"name":"password","designation":"password","type":"P","value":" synthetic "}],"URLs":[{"url":"https://two.example"},{"url":"https://one.example"}],"notesPlain":"first\r\nsecond","sections":[{"title":"Admin","fields":[{"n":"secret","t":"PIN","k":"concealed","v":"1234"},{"n":"public","t":"PIN","k":"string","v":"5678"},{"n":"TOTP_seed","t":"OTP","k":"concealed","v":"JBSWY3DPEHPK3PXP"},{"n":"reserved","t":"Password","k":"concealed","v":"extra"},{"n":"empty","t":"","k":"string","v":""}]}],"passwordHistory":[{"value":"prior-synthetic","time":1700000000}]}}"#;
const WINDOWS: &str = r#"{"hmac":"synthetic","category":"001","overview":{"title":"Windows Login","URLs":[{"u":"https://win.example"}],"url":"https://win.example"},"details":{"fields":[{"designation":"username","name":"user","type":"T","value":"win-user"},{"designation":"password","name":"pass","type":"P","value":"win-synthetic"}],"sections":[{"title":"Extra","fields":[{"n":"code","t":"Code","k":"concealed","v":"hidden"}]}],"notesPlain":"Windows note"}}"#;

#[test]
fn onepassword_standard_windows_and_secure_notes_parse_real_json_line_schema() {
    let text = format!("\u{feff}***aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee***\r\n{STANDARD}\r\n{WINDOWS}\n{{\"typeName\":\"securenotes.SecureNote\",\"title\":\"Note\",\"secureContents\":{{\"notesPlain\":\"note\"}}}}\n{{\"trashed\":true}}\n");
    let rows = parse_1pif(&text).unwrap();
    assert_eq!(rows.len(), 3);
    let row = &rows[0];
    assert_eq!(row.username, " demo ");
    assert_eq!(row.password, " synthetic ");
    assert_eq!(row.notes, "first\nsecond");
    assert_eq!(row.url, "https://one.example https://two.example");
    assert_eq!(row.totp.as_deref(), Some("JBSWY3DPEHPK3PXP"));
    let pin = &row.custom_fields[0];
    assert_eq!(pin.name, "1Password Admin: PIN");
    assert!(pin.protected);
    assert_eq!(pin.value, "1234");
    assert_eq!(row.custom_fields[1].name, "1Password Admin: PIN (2)");
    assert!(!row.custom_fields[1].protected);
    assert_eq!(row.custom_fields[2].value, "extra");
    let history = row
        .custom_fields
        .iter()
        .find(|f| f.name == "1Password Additional Data")
        .unwrap();
    assert!(history.protected && history.value.contains("prior-synthetic"));
    assert_eq!(rows[1].username, "win-user");
    assert_eq!(rows[1].password, "win-synthetic");
    assert_eq!(rows[1].url, "https://win.example");
    assert!(rows[1].custom_fields[0].protected);
    assert_eq!(rows[2].notes, "note");
}

#[test]
fn onepassword_invalid_mixed_records_fail_without_data_or_partial_results() {
    for bad in [
        "not-json secret-sentinel",
        "***Type:webforms.WebForm",
        "{}",
        "[]",
        r#"{"typeName":"webforms.WebForm","title":"secret-sentinel","secureContents":{"fields":{}}}"#,
        r#"{"typeName":"webforms.WebForm","title":"secret-sentinel","secureContents":{"fields":[{"name":"PIN","value":"secret-sentinel","k":true}]}}"#,
        r#"{"typeName":"webforms.WebForm","title":"secret-sentinel","secureContents":{"password":42}}"#,
        r#"{"typeName":"system.folder","title":"secret-sentinel"}"#,
        r#"{"typeName":"webforms.WebForm","title":"secret-sentinel","folderUuid":"f","secureContents":{}}"#,
        r#"{"typeName":"webforms.WebForm","title":"secret-sentinel","secureContents":{"attachments":[{"name":"secret-sentinel"}]}}"#,
    ] {
        let error = parse_1pif(&format!("{STANDARD}\n{bad}")).err().unwrap();
        assert!(error.contains("第 2 行"));
        assert!(!error.contains("secret-sentinel"));
    }
    assert!(parse_1pif("").is_err());
    assert!(parse_1pif("***aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee***").is_err());
    assert!(parse_1pif(&" ".repeat(8 * 1024 * 1024 + 1)).is_err());
}

#[test]
fn onepassword_complex_identity_data_and_guarded_values_are_preserved() {
    let text = r#"{"typeName":"identities.Identity","title":"Identity","secureContents":{"firstname":"Demo","sections":[{"fields":[{"n":"address","t":"Address","v":{"city":"测试"},"a":{"guarded":"yes"}},{"n":"date","t":"Date","k":"date","v":1700000000}]}]}}"#;
    let rows = parse_1pif(text).unwrap();
    assert!(rows[0].custom_fields[0].protected);
    assert_eq!(rows[0].custom_fields[0].value, r#"{"city":"测试"}"#);
    assert_eq!(rows[0].custom_fields[1].value, "1700000000");
    assert!(rows[0].custom_fields[2].protected);
    assert!(rows[0].custom_fields[2].value.contains("Demo"));
}

#[test]
fn onepassword_concealed_username_stays_hidden_and_duplicate_labels_stay_unique() {
    let fields = vec![
        serde_json::json!({"n":"username", "t":"Login", "v":"hidden-name", "k":"concealed"}),
        serde_json::json!({"n":"x", "t":"PIN", "v":"a"}),
        serde_json::json!({"n":"x", "t":"PIN (2)", "v":"b"}),
        serde_json::json!({"n":"x", "t":"PIN", "v":"c"}),
    ];
    let text = serde_json::json!({"typeName":"webforms.WebForm", "title":"Test", "secureContents":{"sections":[{"fields":fields}]}}).to_string();
    let rows = parse_1pif(&text).unwrap();
    assert!(rows[0].username.is_empty());
    assert!(rows[0].custom_fields[0].protected);
    assert_eq!(rows[0].custom_fields[0].value, "hidden-name");
    let names: std::collections::HashSet<_> =
        rows[0].custom_fields.iter().map(|f| &f.name).collect();
    assert_eq!(names.len(), 4);
}

#[test]
fn onepassword_rows_cross_serde_entry_input_and_kdbx_reopen_without_exposing_secrets() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("onepassword.kdbx");
    let mut session = VaultSession::default();
    session
        .create(&path, "master-synthetic", "Aes", "Aes256", "None", None)
        .unwrap();
    let rows = parse_1pif(&format!("{STANDARD}\n{WINDOWS}")).unwrap();
    let inputs: Vec<EntryInput> = rows
        .iter()
        .map(|row| {
            // Exercise camelCase protection through the IPC DTO contract, not a
            // manually copied flag alone. Frontend mapping is covered separately.
            let mut json = serde_json::to_value(row).unwrap();
            json.as_object_mut()
                .unwrap()
                .insert("groupUuid".into(), ROOT_GROUP_UUID.into());
            serde_json::from_value(json).unwrap()
        })
        .collect();
    let state = session.add_entries(&inputs).unwrap();
    assert_eq!(state.root.entries.len(), 2);
    let uuid = state.root.entries[0].uuid.clone();
    for field in &state.root.entries[0].custom_fields {
        if field.protected {
            assert!(field.value.is_empty());
        }
    }
    assert_eq!(session.get_entry_password(&uuid).unwrap(), " synthetic ");
    session.save().unwrap();
    session.close();
    session.open(&path, "master-synthetic", None).unwrap();
    let state = session.snapshot().unwrap();
    let entry = state.root.entries.iter().find(|e| e.uuid == uuid).unwrap();
    let pin = entry
        .custom_fields
        .iter()
        .find(|f| f.name == "1Password Admin: PIN")
        .unwrap();
    assert!(pin.protected && pin.value.is_empty());
    assert_eq!(
        session
            .get_custom_field_value(&uuid, &pin.name)
            .unwrap()
            .as_deref(),
        Some("1234")
    );
    assert_eq!(
        session.get_entry_totp(&uuid).unwrap().as_deref(),
        Some("JBSWY3DPEHPK3PXP")
    );
    assert_eq!(entry.url, "https://one.example https://two.example");
    // The type is the same DTO used by normal imported writes.
    let _: &CustomField = pin;
}
