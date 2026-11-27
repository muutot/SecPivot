//! Bounded native JSON import; source identities never become KeePass controls.
use super::{onepux_archive::SecretJson, parse_bitwarden_rows_with_uris, tree::SourceArchive};
use crate::vault::{
    BitwardenUriSettings, VaultSession, VaultState, FIELD_FAVORITE, FIELD_FAVORITE_TRUE, FIELD_OTP,
    FIELD_WEBSITE_RULES,
};
use chrono::{Datelike, NaiveDateTime, Timelike};
use keepass::db::{Database, Value};
use serde::Serialize;
use serde_json::Value as Json;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;
use zeroize::Zeroizing;

const INVALID: &str = "Bitwarden JSON 内容无效、身份重复或分组引用缺失；未导入任何内容";
const LIMIT: &str = "Bitwarden JSON 超过导入资源上限；未导入任何内容";
const MAX_BYTES: usize = 64 * 1024 * 1024;
const SOURCE_ITEM: &str = "Bitwarden Source Item";

#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BitwardenCounts {
    pub folders: usize,
    pub collections: usize,
    pub items: usize,
    /// Managed exports can contain memberships outside the exported collection scope.
    pub unresolved_memberships: usize,
}

/// Deliberately no Debug/Serialize: prepared secrets stay in native memory.
pub struct BitwardenImport {
    database: Database,
    archive: Value<String>,
    uri_settings_archive: Value<String>,
    counts: BitwardenCounts,
}

impl BitwardenImport {
    pub fn counts(&self) -> BitwardenCounts {
        self.counts
    }
}

// Bound structural tokens/depth BEFORE serde allocates its owned tree. This is
// a conservative ceiling, not a JSON validator or an aggregate heap estimate.
fn preflight(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_BYTES {
        return Err(LIMIT.into());
    }
    let (mut quoted, mut escaped, mut depth, mut tokens) = (false, false, 0usize, 0usize);
    for &byte in bytes {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
            continue;
        }
        match byte {
            b'"' => quoted = true,
            b'{' | b'[' => {
                depth += 1;
                tokens += 1;
                if depth >= 64 {
                    return Err(LIMIT.into());
                }
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            b',' | b':' => tokens += 1,
            _ => {}
        }
        if tokens > 262_144 {
            return Err(LIMIT.into());
        }
    }
    Ok(())
}

fn array<'a>(root: &'a Json, key: &str, required: bool) -> Result<&'a [Json], String> {
    if !root.is_object() {
        return Err(INVALID.into());
    }
    match root.get(key) {
        Some(Json::Array(values)) => Ok(values),
        None | Some(Json::Null) if !required => Ok(&[]),
        _ => Err(INVALID.into()),
    }
}

fn text<'a>(root: &'a Json, key: &str) -> Result<&'a str, String> {
    if !root.is_object() {
        return Err(INVALID.into());
    }
    super::optional_string(root.get(key)).map_err(|_| INVALID.into())
}

fn identity(root: &Json) -> Result<&str, String> {
    let id = text(root, "id")?;
    if id.is_empty() || id.chars().any(char::is_whitespace) {
        return Err(INVALID.into());
    }
    Ok(id)
}

fn timestamp(item: &Json, key: &str) -> Result<Option<NaiveDateTime>, String> {
    match item.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(Json::String(value)) => {
            let date = chrono::DateTime::parse_from_rfc3339(value)
                .map_err(|_| INVALID)?
                .naive_utc();
            if !(1..=9999).contains(&date.year()) {
                return Err(INVALID.into());
            }
            // KDBX dates have second precision; exact source text stays archived.
            Ok(date.with_nanosecond(0))
        }
        _ => Err(INVALID.into()),
    }
}

fn xml_text(value: &str) -> Result<(), String> {
    if value.chars().all(|ch| matches!(ch, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')) {
        Ok(())
    } else {
        Err(INVALID.into())
    }
}

#[derive(Default)]
struct Budget {
    text: usize,
    fields: usize,
}

impl Budget {
    fn value(&mut self, text: String, protected: bool) -> Result<Value<String>, String> {
        // Take secret ownership before a fallible budget or validation check.
        let mut text = Zeroizing::new(text);
        self.text = self
            .text
            .checked_add(text.len())
            .filter(|n| *n <= 128 * 1024 * 1024)
            .ok_or(LIMIT)?;
        self.fields += 1;
        if self.fields > 65_536 {
            return Err(LIMIT.into());
        }
        if !protected {
            xml_text(&text)?;
        }
        Ok(if protected {
            Value::protected(std::mem::take(&mut *text))
        } else {
            Value::unprotected(std::mem::take(&mut *text))
        })
    }
}

fn reserve_uri_policy_bytes(total: &mut usize, next: usize, archived: usize) -> Result<(), String> {
    let retained = total.checked_add(next).ok_or(LIMIT)?;
    if archived
        .checked_add(retained)
        .is_none_or(|bytes| bytes > 128 * 1024 * 1024)
    {
        return Err(LIMIT.into());
    }
    *total = retained;
    Ok(())
}

pub fn prepare_bitwarden_import(
    bytes: &[u8],
    uri_settings: &BitwardenUriSettings,
) -> Result<BitwardenImport, String> {
    uri_settings.validate()?;
    preflight(bytes)?;
    let source = std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
        .map_err(|_| INVALID)?;
    let root: SecretJson = serde_json::from_str(source).map_err(|_| INVALID)?;
    let root = &root.0;
    for flag in ["encrypted", "passwordProtected"] {
        match root.get(flag) {
            Some(Json::Bool(true)) => {
                return Err(
                    "这是加密的 Bitwarden 导出，请在 Bitwarden 中导出未加密 JSON 后重试".into(),
                )
            }
            None | Some(Json::Bool(false)) => {}
            _ => return Err(INVALID.into()),
        }
    }
    let folders = array(root, "folders", false)?;
    let collections = array(root, "collections", false)?;
    let items = array(root, "items", true)?;
    if [folders.len(), collections.len(), items.len()]
        .into_iter()
        .any(|n| n > 8192)
    {
        return Err(LIMIT.into());
    }
    let mut database = Database::new();
    database.root_mut().name = "Bitwarden".into();
    let mut folder_groups = HashMap::new();
    for folder in folders {
        let id = identity(folder)?;
        let name = text(folder, "name")?;
        xml_text(name)?;
        let mut root_group = database.root_mut();
        let mut group = root_group.add_group();
        group.name = name.to_owned();
        if folder_groups.insert(id, group.id()).is_some() {
            return Err(INVALID.into());
        }
    }
    let mut collection_names = HashMap::new();
    for collection in collections {
        let id = identity(collection)?;
        let name = text(collection, "name")?;
        xml_text(name)?;
        // Validate standard metadata, preserve every extension in the archive.
        let organization = text(collection, "organizationId")?;
        text(collection, "externalId")?;
        if collection_names.insert(id, (name, organization)).is_some() {
            return Err(INVALID.into());
        }
    }
    let mut ids = HashSet::new();
    let mut counts = BitwardenCounts {
        folders: folders.len(),
        collections: collections.len(),
        items: items.len(),
        ..Default::default()
    };
    let mut memberships = Vec::with_capacity(items.len());
    let mut total_fields = 0usize;
    let mut tag_bytes = 0usize;
    for item in items {
        let id = text(item, "id")?;
        if !id.is_empty() && (!ids.insert(id) || id.chars().any(char::is_whitespace)) {
            return Err(INVALID.into());
        }
        let folder = text(item, "folderId")?;
        let organization = text(item, "organizationId")?;
        if !folder.is_empty() && !folder_groups.contains_key(folder) {
            return Err(INVALID.into());
        }
        let fields = array(item, "fields", false)?;
        total_fields += fields.len();
        if fields.len() > 4096 || total_fields > 65_536 {
            return Err(LIMIT.into());
        }
        timestamp(item, "creationDate")?;
        timestamp(item, "revisionDate")?;
        let mut seen = HashSet::new();
        let mut tags = Vec::new();
        let collection_ids = array(item, "collectionIds", false)?;
        if collection_ids.len() > 4096 {
            return Err(LIMIT.into());
        }
        for value in collection_ids {
            let id = value.as_str().filter(|s| !s.is_empty()).ok_or(INVALID)?;
            if !seen.insert(id) {
                return Err(INVALID.into());
            }
            if let Some((name, collection_organization)) = collection_names.get(id) {
                if !organization.is_empty()
                    && !collection_organization.is_empty()
                    && organization != *collection_organization
                {
                    return Err(INVALID.into());
                }
                // Official KeePass maps nested collection names to display tags.
                // Match KDBX delimiter normalization now so save/reopen is stable.
                let display = name.replace('/', " / ");
                for tag in display
                    .split([';', ',', '\t'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                {
                    if !tags.iter().any(|s| s == tag) {
                        tag_bytes = tag_bytes
                            .checked_add(tag.len())
                            .filter(|n| *n <= MAX_BYTES)
                            .ok_or(LIMIT)?;
                        tags.push(tag.to_owned());
                    }
                }
            } else {
                // Bitwarden managed exports retain memberships outside their
                // collection scope. Preserve IDs in Source Item and report them.
                counts.unresolved_memberships += 1;
            }
        }
        memberships.push(tags);
    }
    let mut budget = Budget::default();
    let archive = budget.value(source.to_owned(), true)?;
    let uri_settings_archive = budget.value(
        serde_json::to_string(uri_settings).map_err(|_| INVALID)?,
        true,
    )?;
    let mut uri_bytes = 0usize;
    let uri_plans = items
        .iter()
        .map(|item| {
            if item.get("type").and_then(Json::as_i64) == Some(1) {
                let (primary, rules) =
                    super::bitwarden_uris::prepare(item.get("login"), uri_settings)?;
                reserve_uri_policy_bytes(&mut uri_bytes, rules.as_json().len(), budget.text)?;
                // Retain serialized policy only, dropping its repeated source
                // settings tree before preparing the next item. Reject the
                // aggregate output budget before allocating another retained copy.
                Ok(Some((primary, Zeroizing::new(rules.as_json().to_owned()))))
            } else {
                Ok(None)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let rows = parse_bitwarden_rows_with_uris(root, |_, index| {
        Ok(uri_plans[index]
            .as_ref()
            .map(|(primary, _)| primary.clone())
            .unwrap_or_default())
    })?;
    for (((mut row, item), tags), uri_plan) in
        rows.into_iter().zip(items).zip(memberships).zip(uri_plans)
    {
        let group_id = folder_groups
            .get(text(item, "folderId")?)
            .copied()
            .unwrap_or_else(|| database.root().id());
        let mut group = database.group_mut(group_id).ok_or(INVALID)?;
        let mut entry = group.add_entry();
        for (key, value, protected) in [
            ("Title", &mut row.title, false),
            ("UserName", &mut row.username, false),
            ("Password", &mut row.password, true),
            ("URL", &mut row.url, false),
            ("Notes", &mut row.notes, false),
        ] {
            entry
                .fields
                .insert(key.into(), budget.value(std::mem::take(value), protected)?);
        }
        if let Some(totp) = row.totp.take() {
            entry
                .fields
                .insert(FIELD_OTP.into(), budget.value(totp, true)?);
        }
        for mut field in row.custom_fields.drain(..) {
            xml_text(&field.name)?;
            entry.fields.insert(
                std::mem::take(&mut field.name),
                budget.value(std::mem::take(&mut field.value), field.protected)?,
            );
        }
        if let Some((_, mut rules)) = uri_plan {
            entry.fields.insert(
                FIELD_WEBSITE_RULES.into(),
                budget.value(std::mem::take(&mut *rules), true)?,
            );
        }
        // Source labels may already use this name; never overwrite source data.
        let mut archive_key = SOURCE_ITEM.to_owned();
        let mut suffix = 2;
        while entry.fields.contains_key(&archive_key) {
            archive_key = format!("{SOURCE_ITEM} ({suffix})");
            suffix += 1;
        }
        entry.fields.insert(
            archive_key,
            budget.value(serde_json::to_string(item).map_err(|_| INVALID)?, true)?,
        );
        if row.favorite == Some(true) {
            entry.fields.insert(
                FIELD_FAVORITE.into(),
                Value::unprotected(FIELD_FAVORITE_TRUE),
            );
        }
        entry.quality_check = row.quality_check.unwrap_or(true);
        entry.tags = tags;
        if let Some(date) = timestamp(item, "creationDate")? {
            entry.times.creation = Some(date);
        }
        if let Some(date) = timestamp(item, "revisionDate")? {
            entry.times.last_modification = Some(date);
        }
    }
    Ok(BitwardenImport {
        database,
        archive,
        uri_settings_archive,
        counts,
    })
}

impl VaultSession {
    pub(crate) fn import_bitwarden(
        &mut self,
        plan: BitwardenImport,
        target: &str,
        lifetime: Uuid,
    ) -> Result<VaultState, String> {
        self.import_prepared_tree(
            plan.database,
            SourceArchive {
                title: "Bitwarden Source Archive",
                key: "Bitwarden Source Export",
                value: plan.archive,
                companion: Some(("Bitwarden URI Source Settings", plan.uri_settings_archive)),
                literal_fields: true,
            },
            target,
            lifetime,
        )
    }
}

#[cfg(test)]
#[path = "bitwarden_native_tests.rs"]
mod tests;
