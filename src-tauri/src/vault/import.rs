//! Structured import parsers (Bitwarden JSON and 1Password 1PIF exports).
//! a malformed or unexpected document fails with a readable error instead of
//! silently importing garbage.

use serde::Serialize;
use std::collections::HashMap;

mod bitwarden_links;
mod fields;
mod onepassword;
pub use onepassword::parse_1pif;

#[cfg(test)]
mod bitwarden_tests;
#[cfg(test)]
mod onepassword_tests;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCustomField {
    pub name: String,
    pub value: String,
    pub protected: bool,
}

/// One normalized import row shared by the CSV/KeePass-XML/Bitwarden paths.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportRow {
    pub group: String,
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totp: Option<String>,
    pub custom_fields: Vec<ImportCustomField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality_check: Option<bool>,
}

/// Parse an unencrypted Bitwarden `.json` export. Official item types 1–8 are
/// retained; specialized values become protected custom fields. Folders become the
/// group path, the first URI becomes the URL, `login.totp` seeds TOTP, and
/// custom fields round-trip as entry fields.
pub fn parse_bitwarden_json(text: &str) -> Result<Vec<ImportRow>, String> {
    let root: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("不是有效的 Bitwarden JSON: {e}"))?;
    for name in ["encrypted", "passwordProtected"] {
        match root.get(name) {
            Some(serde_json::Value::Bool(true)) => {
                return Err(
                    "这是加密的 Bitwarden 导出，请在 Bitwarden 中导出未加密 JSON 后重试".into(),
                );
            }
            Some(serde_json::Value::Bool(false)) | None => {}
            Some(_) => return Err("Bitwarden JSON 的加密标记无效".into()),
        }
    }
    let items = root
        .get("items")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "缺少 items 数组".to_owned())?;
    let folders: HashMap<String, String> = root
        .get("folders")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|folder| {
                    let id = folder.get("id")?.as_str()?.to_owned();
                    let name = folder
                        .get("name")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("")
                        .to_owned();
                    Some((id, name))
                })
                .collect()
        })
        .unwrap_or_default();

    let mut rows = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let invalid = || format!("第 {} 条不是受支持的有效 Bitwarden 记录", index + 1);
        let kind = item
            .get("type")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        if !(1..=8).contains(&kind) {
            return Err(invalid());
        }
        let title = item
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_owned();
        let notes = item
            .get("notes")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_owned();
        let group = item
            .get("folderId")
            .and_then(serde_json::Value::as_str)
            .and_then(|id| folders.get(id).cloned())
            .unwrap_or_default();

        let mut username = String::new();
        let mut password = String::new();
        let mut url = String::new();
        let mut totp = String::new();
        if let Some(login) = item.get("login") {
            username = login
                .get("username")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned();
            password = login
                .get("password")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned();
            totp = login
                .get("totp")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("")
                .to_owned();
            url = login
                .get("uris")
                .and_then(serde_json::Value::as_array)
                .and_then(|uris| {
                    uris.iter()
                        .find_map(|uri| uri.get("uri").and_then(serde_json::Value::as_str))
                })
                .unwrap_or("")
                .to_owned();
        }

        let mut names = fields::FieldNames::new("Bitwarden");
        let mut custom_fields: Vec<ImportCustomField> = item
            .get("fields")
            .and_then(serde_json::Value::as_array)
            .map(|fields| {
                fields
                    .iter()
                    .filter(|field| field.is_object())
                    .map(|field| -> Result<_, String> {
                        let name = field
                            .get("name")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("");
                        let field_type = field.get("type").and_then(serde_json::Value::as_i64);
                        let value = if field_type == Some(3) {
                            bitwarden_links::resolve(item, kind, field.get("linkedId"))
                                .map_err(|()| invalid())?
                        } else {
                            field
                                .get("value")
                                .and_then(serde_json::Value::as_str)
                                .unwrap_or("")
                                .to_owned()
                        };
                        Ok(names.field(name, value, matches!(field_type, Some(1 | 3))))
                    })
                    .collect::<Result<_, _>>()
            })
            .transpose()?
            .unwrap_or_default();

        if let Some((section, display, string_keys)) = typed_section(kind) {
            custom_fields.push(names.field("Item Type", display.to_owned(), false));
            match item.get(section) {
                None | Some(serde_json::Value::Null) => {}
                Some(serde_json::Value::Object(values)) => {
                    for (key, value) in values {
                        let value = match value {
                            serde_json::Value::Null => continue,
                            serde_json::Value::String(value) => value.clone(),
                            _ if string_keys.contains(&key.as_str()) => return Err(invalid()),
                            value => value.to_string(),
                        };
                        let label = format!("{display} {}", field_label(key));
                        // Financial/identity/private-key data belongs behind the
                        // same explicit reveal boundary as hidden custom values.
                        custom_fields.push(names.field(&label, value, true));
                    }
                }
                Some(_) => return Err(invalid()),
            }
        }

        rows.push(ImportRow {
            group,
            title,
            username,
            password,
            url,
            notes,
            totp: if totp.is_empty() { None } else { Some(totp) },
            custom_fields,
            quality_check: Some(kind == 1),
        });
    }
    Ok(rows)
}

/// Keys follow Bitwarden's public Export models. Unknown extensions remain
/// protected data; they must not be guessed into native KeePass controls.
fn typed_section(kind: i64) -> Option<(&'static str, &'static str, &'static [&'static str])> {
    match kind {
        3 => Some((
            "card",
            "Card",
            &[
                "cardholderName",
                "brand",
                "number",
                "expMonth",
                "expYear",
                "code",
            ],
        )),
        4 => Some((
            "identity",
            "Identity",
            &[
                "title",
                "firstName",
                "middleName",
                "lastName",
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
            ],
        )),
        5 => Some((
            "sshKey",
            "SSH",
            &["privateKey", "publicKey", "keyFingerprint"],
        )),
        6 => Some((
            "bankAccount",
            "Bank Account",
            &[
                "bankName",
                "nameOnAccount",
                "accountType",
                "accountNumber",
                "routingNumber",
                "branchNumber",
                "pin",
                "swiftCode",
                "iban",
                "bankContactPhone",
            ],
        )),
        7 => Some((
            "driversLicense",
            "Driver License",
            &[
                "firstName",
                "middleName",
                "lastName",
                "dateOfBirth",
                "licenseNumber",
                "issuingCountry",
                "issuingState",
                "issueDate",
                "expirationDate",
                "issuingAuthority",
                "licenseClass",
            ],
        )),
        8 => Some((
            "passport",
            "Passport",
            &[
                "surname",
                "givenName",
                "dateOfBirth",
                "sex",
                "birthPlace",
                "nationality",
                "issuingCountry",
                "passportNumber",
                "passportType",
                "nationalIdentificationNumber",
                "issuingAuthority",
                "issueDate",
                "expirationDate",
            ],
        )),
        _ => None,
    }
}

fn field_label(key: &str) -> String {
    match key {
        "ssn" => return "SSN".to_owned(),
        "pin" => return "PIN".to_owned(),
        "iban" => return "IBAN".to_owned(),
        _ => {}
    }
    let mut label = String::new();
    for (index, ch) in key.chars().enumerate() {
        if index == 0 {
            label.extend(ch.to_uppercase());
        } else {
            if ch.is_ascii_uppercase() {
                label.push(' ');
            }
            label.push(ch);
        }
    }
    label
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitwarden_json_parses_logins_secure_notes_and_cards() {
        let text = r#"{
          "encrypted": false,
          "folders": [{ "id": "f1", "name": "Work" }],
          "items": [
            {
              "type": 1,
              "name": "GitHub",
              "notes": "note",
              "folderId": "f1",
              "login": {
                "username": "octocat",
                "password": "s3cret",
                "totp": "otpauth://totp/GitHub:octocat?secret=JBSWY3DPEHPK3PXP&issuer=GitHub",
                "uris": [{ "uri": "https://github.com" }]
              },
              "fields": [{ "name": "PIN", "value": "1234", "type": 0 },
                         { "name": "Secret PIN", "value": "5678", "type": 1 }]
            },
            { "type": 2, "name": "Wifi", "notes": "psk: hunter2", "folderId": null },
            { "type": 3, "name": "Card", "card": { "number": "4111" } }
          ]
        }"#;
        let rows = parse_bitwarden_json(text).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].title, "GitHub");
        assert_eq!(rows[0].group, "Work");
        assert_eq!(rows[0].username, "octocat");
        assert_eq!(rows[0].password, "s3cret");
        assert_eq!(rows[0].url, "https://github.com");
        assert!(rows[0].totp.as_deref().unwrap().starts_with("otpauth://"));
        assert_eq!(rows[0].custom_fields.len(), 2);
        assert_eq!(rows[0].custom_fields[0].name, "Bitwarden PIN");
        assert!(!rows[0].custom_fields[0].protected);
        assert!(rows[0].custom_fields[1].protected);
        assert_eq!(rows[1].title, "Wifi");
        assert!(rows[1].username.is_empty());
        assert_eq!(rows[2].title, "Card");
        assert!(rows[2]
            .custom_fields
            .iter()
            .any(|field| field.value == "4111" && field.protected));
    }

    #[test]
    fn bitwarden_json_rejects_malformed_and_missing_items() {
        assert!(parse_bitwarden_json("not json").is_err());
        assert!(parse_bitwarden_json(r#"{ "folders": [] }"#).is_err());
        assert!(parse_bitwarden_json(r#"{ "items": [] }"#)
            .unwrap()
            .is_empty());
    }

    #[test]
    fn bitwarden_json_rejects_encrypted_exports_before_mapping_items() {
        for flags in [
            serde_json::json!({"encrypted": true}),
            serde_json::json!({"encrypted": true, "passwordProtected": true}),
            serde_json::json!({"encrypted": false, "passwordProtected": true}),
        ] {
            let mut export = flags;
            export["items"] = serde_json::json!([{
                "type": 1, "name": "synthetic-ciphertext-title",
                "login": {"password": "synthetic-ciphertext-password"}
            }]);
            let result = parse_bitwarden_json(&export.to_string());
            assert!(
                result.is_err(),
                "encrypted export was accepted as plaintext"
            );
            let error = result.err().unwrap();
            assert!(error.contains("加密"));
            assert!(!error.contains("synthetic-ciphertext"));
        }
    }

    #[test]
    fn bitwarden_json_validates_encryption_markers_and_accepts_legacy_plaintext() {
        for flag in [
            serde_json::json!("true"),
            serde_json::json!(1),
            serde_json::Value::Null,
        ] {
            for name in ["encrypted", "passwordProtected"] {
                let export = serde_json::json!({name: flag, "items": []});
                assert!(parse_bitwarden_json(&export.to_string()).is_err());
            }
        }
        for export in [
            r#"{"items": []}"#,
            r#"{"encrypted":false,"items": []}"#,
            r#"{"encrypted":false,"passwordProtected":false,"items": []}"#,
        ] {
            assert!(parse_bitwarden_json(export).unwrap().is_empty());
        }
    }
}
