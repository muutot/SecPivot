//! 1PIF is one JSON object per line, with optional `***UUID***` separators.
//! Map known fields and retain unmapped secure data as protected JSON fields.
//! Binary attachments and folder hierarchy require a separate import contract.

use super::{fields::FieldNames, ImportRow};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashSet;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Record {
    #[serde(default)]
    trashed: bool,
    type_name: Option<String>,
    title: Option<String>,
    location: Option<String>,
    secure_contents: Option<Map<String, Value>>,
    overview: Option<Overview>,
    details: Option<Map<String, Value>>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Deserialize)]
struct Overview {
    title: String,
    url: Option<String>,
    #[serde(rename = "URLs")]
    urls: Option<Vec<WinUrl>>,
}

#[derive(Deserialize)]
struct WinUrl {
    u: String,
}

/// Parse the standard and Windows OpVault JSON-line variants. No partial result
/// escapes when any record is malformed. Errors contain line numbers, not data.
pub fn parse_1pif(text: &str) -> Result<Vec<ImportRow>, String> {
    if text.len() > 8 * 1024 * 1024 {
        return Err("1PIF 文件超过 8 MiB 上限".into());
    }
    let mut rows = Vec::new();
    let mut records = 0;
    for (index, line) in text.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(id) = line.strip_prefix("***").and_then(|s| s.strip_suffix("***")) {
            if uuid::Uuid::parse_str(id).is_ok() {
                continue;
            }
        }
        let fail = || format!("第 {} 行不是受支持的有效 1Password 1PIF 记录", index + 1);
        let record: Record = serde_json::from_str(line).map_err(|_| fail())?;
        records += 1;
        if record.trashed {
            continue;
        }
        // Folder metadata cannot be restored through the row-only contract yet.
        // Refuse rather than silently flattening hierarchy or dropping attachments.
        if record.type_name.as_deref() == Some("system.folder")
            || record.extra.get("folderUuid").is_some_and(nonempty)
            || record.extra.get("folderUUID").is_some_and(nonempty)
        {
            return Err(format!("第 {} 行含有尚不支持的 1PIF 分组层级", index + 1));
        }
        rows.push(parse_record(record).map_err(|_| fail())?);
    }
    if records == 0 {
        return Err("未找到有效 1PIF JSON 行记录；1PUX ZIP 格式尚不支持".into());
    }
    Ok(rows)
}

fn parse_record(record: Record) -> Result<ImportRow, ()> {
    let mut names = FieldNames::new("1Password");
    let (title, mut contents, mut urls) = if let Some(overview) = record.overview {
        let mut urls: Vec<String> = overview
            .urls
            .unwrap_or_default()
            .into_iter()
            .map(|u| u.u)
            .collect();
        urls.extend(overview.url);
        (overview.title, record.details.ok_or(())?, urls)
    } else {
        if record.type_name.as_deref().is_none_or(str::is_empty) {
            return Err(());
        }
        (
            record.title.ok_or(())?,
            record.secure_contents.ok_or(())?,
            record.location.into_iter().collect(),
        )
    };
    if ["attachments", "documentAttributes"].iter().any(|key| {
        contents
            .get(*key)
            .or_else(|| record.extra.get(*key))
            .is_some_and(nonempty)
    }) {
        return Err(());
    }
    let mut row = ImportRow {
        group: String::new(),
        title,
        username: String::new(),
        password: take_string(&mut contents, "password")?,
        url: String::new(),
        notes: take_string(&mut contents, "notesPlain")?.replace("\r\n", "\n"),
        totp: None,
        custom_fields: Vec::new(),
    };
    let mut extra = record.extra;
    for url in take_array(&mut contents, "URLs")?
        .into_iter()
        .chain(take_array(&mut extra, "URLs")?)
    {
        let u = url.as_object().ok_or(())?;
        urls.push(u.get("url").and_then(Value::as_str).ok_or(())?.to_owned());
    }
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for url in urls {
        if !url.is_empty() && seen.insert(url.clone()) {
            unique.push(url);
        }
    }
    row.url = unique.join(" ");
    for field in take_array(&mut contents, "fields")? {
        add_field(&mut row, &mut names, &field, false, "")?;
    }
    for section in take_array(&mut contents, "sections")? {
        let mut section = section.as_object().ok_or(())?.clone();
        let title = take_string(&mut section, "title")?;
        for field in take_array(&mut section, "fields")? {
            add_field(&mut row, &mut names, &field, true, &title)?;
        }
    }
    // Password history and specialized identity/card fields remain available,
    // protected in KDBX; they are not native history or typed identity records.
    if !contents.is_empty() {
        add_custom(
            &mut row,
            &mut names,
            "Additional Data",
            Value::Object(contents).to_string(),
            true,
        );
    }
    Ok(row)
}

fn nonempty(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Array(v) => !v.is_empty(),
        Value::Object(v) => !v.is_empty(),
        Value::String(v) => !v.is_empty(),
        _ => true,
    }
}

fn take_string(map: &mut Map<String, Value>, key: &str) -> Result<String, ()> {
    match map.remove(key) {
        None | Some(Value::Null) => Ok(String::new()),
        Some(Value::String(s)) => Ok(s),
        _ => Err(()),
    }
}

fn take_array(map: &mut Map<String, Value>, key: &str) -> Result<Vec<Value>, ()> {
    match map.remove(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(v)) => Ok(v),
        _ => Err(()),
    }
}

fn add_field(
    row: &mut ImportRow,
    names: &mut FieldNames,
    field: &Value,
    section: bool,
    section_title: &str,
) -> Result<(), ()> {
    let f = field.as_object().ok_or(())?;
    let key = if section { "n" } else { "designation" };
    let designation = optional_string(f, key)?;
    let value = match f.get(if section { "v" } else { "value" }) {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(v) => v.to_string(),
    };
    let kind = optional_string(f, "k")?;
    let field_type = optional_string(f, "type")?;
    let guarded = match f.get("a") {
        None | Some(Value::Null) => false,
        Some(Value::Object(a)) => optional_string(a, "guarded")? == "yes",
        _ => return Err(()),
    };
    let protected = designation == "password"
        || designation.starts_with("TOTP_")
        || kind == "concealed"
        || field_type == "P"
        || guarded;
    if !value.is_empty() {
        if designation == "username" && row.username.is_empty() && !protected {
            row.username = value;
            return Ok(());
        }
        if designation == "password" && row.password.is_empty() {
            row.password = value;
            return Ok(());
        }
        if designation.starts_with("TOTP_") && row.totp.is_none() {
            row.totp = Some(value);
            return Ok(());
        }
    }
    let name = optional_string(f, if section { "t" } else { "name" })?;
    let name = if name.trim().is_empty() {
        designation
    } else {
        name
    };
    let name = if section_title.is_empty() {
        name.to_owned()
    } else {
        format!("{section_title}: {name}")
    };
    add_custom(row, names, &name, value, protected);
    Ok(())
}

fn optional_string<'a>(map: &'a Map<String, Value>, key: &str) -> Result<&'a str, ()> {
    match map.get(key) {
        None | Some(Value::Null) => Ok(""),
        Some(Value::String(s)) => Ok(s),
        _ => Err(()),
    }
}

fn add_custom(
    row: &mut ImportRow,
    names: &mut FieldNames,
    name: &str,
    value: String,
    protected: bool,
) {
    row.custom_fields.push(names.field(name, value, protected));
}
