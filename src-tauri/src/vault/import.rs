//! Structured import parsers (Bitwarden JSON export). Every parser is strict:
//! a malformed or unexpected document fails with a readable error instead of
//! silently importing garbage.

use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCustomField {
    pub name: String,
    pub value: String,
}

/// One normalized import row shared by the CSV/KeePass-XML/Bitwarden paths.
#[derive(Debug, Clone, Serialize)]
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
}

/// Parse a Bitwarden `.json` export. Login (type 1) and secure-note (type 2)
/// items are imported; cards and identities are skipped. Folders become the
/// group path, the first URI becomes the URL, `login.totp` seeds TOTP, and
/// custom fields round-trip as entry fields.
pub fn parse_bitwarden_json(text: &str) -> Result<Vec<ImportRow>, String> {
    let root: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("不是有效的 Bitwarden JSON: {e}"))?;
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
    for item in items {
        let kind = item
            .get("type")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(0);
        if kind != 1 && kind != 2 {
            continue;
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

        let custom_fields = item
            .get("fields")
            .and_then(serde_json::Value::as_array)
            .map(|fields| {
                fields
                    .iter()
                    .filter_map(|field| {
                        let name = field.get("name")?.as_str()?.to_owned();
                        let value = field
                            .get("value")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("")
                            .to_owned();
                        Some(ImportCustomField { name, value })
                    })
                    .collect()
            })
            .unwrap_or_default();

        rows.push(ImportRow {
            group,
            title,
            username,
            password,
            url,
            notes,
            totp: if totp.is_empty() { None } else { Some(totp) },
            custom_fields,
        });
    }
    Ok(rows)
}

/// Parse a 1Password Interchange Format (`.1pif`) text export. Items are
/// `***Key:value` blocks separated by `***End of Item***`; tab-indented lines
/// continue the previous value; `***Folder:` names become the group path.
/// Folder definitions (`***Type:system.folder`) are skipped.
pub fn parse_1pif(text: &str) -> Vec<ImportRow> {
    let mut rows = Vec::new();
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut item_type = String::new();

    for raw_line in text.lines() {
        let line = raw_line.trim_end_matches('\r');
        if line.starts_with('\t') {
            // Continuation of the previous value (multi-line notes).
            if let Some((_, value)) = fields.last_mut() {
                if !value.is_empty() {
                    value.push('\n');
                }
                value.push_str(line.trim_start_matches('\t'));
            }
            continue;
        }
        if line.starts_with("***End of Item***") {
            if let Some(row) = finish_1pif_item(&item_type, &mut fields) {
                rows.push(row);
            }
            fields.clear();
            item_type.clear();
            continue;
        }
        let Some(rest) = line.strip_prefix("***") else {
            continue;
        };
        let Some((key, value)) = rest.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key == "Type" {
            item_type = value.trim().to_owned();
        }
        fields.push((key.to_owned(), value.trim().to_owned()));
    }
    if let Some(row) = finish_1pif_item(&item_type, &mut fields) {
        rows.push(row);
    }
    rows
}

fn finish_1pif_item(item_type: &str, fields: &mut [(String, String)]) -> Option<ImportRow> {
    if item_type == "system.folder" || item_type.is_empty() {
        return None;
    }
    let get = |key: &str| -> String {
        fields
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    let title = get("Name");
    let title = if title.is_empty() {
        get("Title")
    } else {
        title
    };
    let mut custom_fields = Vec::new();
    for (key, value) in fields.iter() {
        if key.eq_ignore_ascii_case("Field") {
            if let Some((name, value)) = value.split_once('=') {
                let name = name.trim().to_owned();
                let value = value.trim().trim_matches(['[', ']']).to_owned();
                if !name.is_empty() {
                    custom_fields.push(ImportCustomField { name, value });
                }
            }
        }
    }
    let url = get("Location");
    let url = if url.is_empty() { get("URL") } else { url };
    let totp = get("TOTP");
    Some(ImportRow {
        group: get("Folder"),
        title,
        username: get("Username"),
        password: get("Password"),
        url,
        notes: get("Notes"),
        totp: if totp.is_empty() { None } else { Some(totp) },
        custom_fields,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitwarden_json_parses_logins_secure_notes_and_skips_cards() {
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
              "fields": [{ "name": "PIN", "value": "1234", "type": 0 }]
            },
            { "type": 2, "name": "Wifi", "notes": "psk: hunter2", "folderId": null },
            { "type": 3, "name": "Card", "card": { "number": "4111" } }
          ]
        }"#;
        let rows = parse_bitwarden_json(text).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "GitHub");
        assert_eq!(rows[0].group, "Work");
        assert_eq!(rows[0].username, "octocat");
        assert_eq!(rows[0].password, "s3cret");
        assert_eq!(rows[0].url, "https://github.com");
        assert!(rows[0].totp.as_deref().unwrap().starts_with("otpauth://"));
        assert_eq!(rows[0].custom_fields.len(), 1);
        assert_eq!(rows[0].custom_fields[0].name, "PIN");
        assert_eq!(rows[1].title, "Wifi");
        assert!(rows[1].username.is_empty());
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
    fn one_password_1pif_parses_items_folders_and_continuations() {
        let text = "\
***Top of File***
***UUID:111
***Type:webforms.WebForm
***Name:GitHub
***Location:https://github.com
***Username:octocat
***Password:s3cret
***Notes:line one
\tline two
***Field:PIN=1234
***Folder:Work
***End of Item***
***UUID:222
***Type:system.folder
***Name:Work
***End of Item***
***UUID:333
***Type:webforms.WebForm
***Name:Wifi
***Notes:psk
***End of Item***
";
        let rows = parse_1pif(text);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].title, "GitHub");
        assert_eq!(rows[0].group, "Work");
        assert_eq!(rows[0].username, "octocat");
        assert_eq!(rows[0].password, "s3cret");
        assert_eq!(rows[0].url, "https://github.com");
        assert_eq!(rows[0].notes, "line one\nline two");
        assert_eq!(rows[0].custom_fields.len(), 1);
        assert_eq!(rows[0].custom_fields[0].name, "PIN");
        assert_eq!(rows[0].custom_fields[0].value, "1234");
        assert_eq!(rows[1].title, "Wifi");
        assert!(rows[1].group.is_empty());
    }
}
