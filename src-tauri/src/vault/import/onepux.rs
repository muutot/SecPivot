//! Prepare complete native import data before touching the destination database.
//! Source history/unknown metadata are protected archives, not native history.
use super::fields::FieldNames;
use super::{parse_1pux_archive, OnePuxArchive};
use crate::vault::{helpers::parse_group_id, VaultSession, VaultState, ROOT_GROUP_UUID};
use keepass::db::{GroupMut, Value};
use serde::Serialize;
use serde_json::{Map, Value as Json};
use std::collections::{BTreeMap, HashSet};

const INVALID: &str = "1PUX 导入数据无效或附件身份不明确；未导入任何内容";
const LIMIT: &str = "1PUX 导入计划超过资源上限；未导入任何内容";

#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnePuxCounts {
    pub accounts: usize,
    pub vaults: usize,
    pub items: usize,
    pub archived: usize,
    pub attachments: usize,
    pub unreferenced_files: usize,
}

pub struct OnePuxImport {
    root: PreparedGroup,
    counts: OnePuxCounts,
}

impl OnePuxImport {
    pub fn counts(&self) -> OnePuxCounts {
        self.counts
    }
}

struct PreparedGroup {
    name: String,
    children: Vec<PreparedGroup>,
    items: Vec<PreparedItem>,
}

impl PreparedGroup {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            children: Vec::new(),
            items: Vec::new(),
        }
    }
}

#[derive(Default)]
struct PreparedItem {
    fields: BTreeMap<String, Value<String>>,
    files: BTreeMap<String, Value<Vec<u8>>>,
    tags: Vec<String>,
    created: Option<chrono::NaiveDateTime>,
    modified: Option<chrono::NaiveDateTime>,
    quality: bool,
}

#[derive(Default)]
struct Budget {
    binary: usize,
    text: usize,
    groups: usize,
    fields: usize,
}

fn reserve(total: &mut usize, size: usize, cap: usize) -> Result<(), String> {
    *total = total.checked_add(size).filter(|n| *n <= cap).ok_or(LIMIT)?;
    Ok(())
}

impl Budget {
    fn text(&mut self, value: &str, protected: bool) -> Result<Value<String>, String> {
        reserve(&mut self.text, value.len(), 64 * 1024 * 1024)?;
        reserve(&mut self.fields, 1, 65_536)?;
        Ok(if protected {
            Value::protected(value.to_owned())
        } else {
            Value::unprotected(value.to_owned())
        })
    }

    fn json(&mut self, value: &Json) -> Result<Value<String>, String> {
        let text = zeroize::Zeroizing::new(serde_json::to_string(value).map_err(|_| INVALID)?);
        self.text(&text, true)
    }

    fn binary(&mut self, bytes: &[u8]) -> Result<Value<Vec<u8>>, String> {
        // Shared source files may be referenced by many entries: cap every copy.
        reserve(&mut self.binary, bytes.len(), 256 * 1024 * 1024)?;
        Ok(Value::protected(bytes.to_vec()))
    }

    fn group(&mut self, name: &str) -> Result<PreparedGroup, String> {
        reserve(&mut self.groups, 1, 8192)?;
        Ok(PreparedGroup::new(name))
    }
}

fn object(value: &Json) -> Result<&Map<String, Json>, String> {
    value.as_object().ok_or_else(|| INVALID.to_owned())
}

fn text<'a>(value: &'a Json, key: &str) -> Result<&'a str, String> {
    match object(value)?.get(key) {
        Some(Json::String(value)) => Ok(value),
        None | Some(Json::Null) => Ok(""),
        _ => Err(INVALID.into()),
    }
}

fn identity(value: &Json) -> Result<&str, String> {
    let id = text(value, "uuid")?;
    if id.is_empty() {
        return Err(INVALID.into());
    }
    Ok(id)
}

fn array<'a>(value: &'a Json, key: &str, required: bool) -> Result<&'a [Json], String> {
    match object(value)?.get(key) {
        Some(Json::Array(value)) => Ok(value),
        None | Some(Json::Null) if !required => Ok(&[]),
        _ => Err(INVALID.into()),
    }
}

fn timestamp(value: &Json, key: &str) -> Result<Option<chrono::NaiveDateTime>, String> {
    match object(value)?.get(key) {
        None | Some(Json::Null) => Ok(None),
        Some(value) => {
            let seconds = value.as_i64().ok_or(INVALID)?;
            Ok(Some(
                chrono::DateTime::from_timestamp(seconds, 0)
                    .ok_or(INVALID)?
                    .naive_utc(),
            ))
        }
    }
}

fn custom(item: &mut PreparedItem, names: &mut FieldNames, label: &str, value: Value<String>) {
    item.fields
        .insert(names.field(label, String::new(), true).name, value);
}

fn document(
    value: &Json,
    archive: &OnePuxArchive,
    item: &mut PreparedItem,
    aliases: &mut BTreeMap<String, String>,
    used: &mut HashSet<String>,
    budget: &mut Budget,
    counts: &mut OnePuxCounts,
) -> Result<(), String> {
    let id = text(value, "documentId")?;
    let name = text(value, "fileName")?;
    if [id, name].iter().any(|part| {
        part.is_empty()
            || *part == "."
            || *part == ".."
            || part
                .chars()
                .any(|ch| ch.is_control() || ch == '/' || ch == '\\')
    }) {
        return Err(INVALID.into());
    }
    // Official examples and KeePassXC use different separators. Resolve only
    // these exact full identities; never use basename/prefix or fuzzy matching.
    let candidates = [
        format!("files/{id}__{name}"),
        format!("files/{id}___{name}"),
    ];
    let candidates: Vec<_> = candidates
        .iter()
        .filter(|path| archive.files().contains_key(*path))
        .collect();
    if candidates.len() != 1 {
        return Err(INVALID.into());
    }
    let path = candidates[0];
    let bytes = &archive.files()[path];
    if object(value)?
        .get("decryptedSize")
        .is_some_and(|size| size.as_u64() != Some(bytes.len() as u64))
    {
        return Err(INVALID.into());
    }
    if let Some(previous) = aliases.get(name) {
        if previous != path {
            return Err(INVALID.into());
        }
        return Ok(());
    }
    if item.files.len() >= 128 {
        return Err(LIMIT.into());
    }
    item.files.insert(name.to_owned(), budget.binary(bytes)?);
    aliases.insert(name.to_owned(), path.to_owned());
    used.insert(path.to_owned());
    counts.attachments += 1;
    Ok(())
}

fn prepare_item(
    source: &Json,
    archive: &OnePuxArchive,
    used: &mut HashSet<String>,
    budget: &mut Budget,
    counts: &mut OnePuxCounts,
) -> Result<(PreparedItem, bool), String> {
    let overview = source.get("overview").ok_or(INVALID)?;
    let details = source.get("details").ok_or(INVALID)?;
    object(details)?;
    let mut item = PreparedItem {
        quality: text(source, "categoryUuid")? == "001",
        created: timestamp(source, "createdAt")?,
        modified: timestamp(source, "updatedAt")?,
        ..Default::default()
    };
    item.fields.insert(
        "Title".into(),
        budget.text(text(overview, "title")?, false)?,
    );
    item.fields.insert(
        "Notes".into(),
        budget.text(text(details, "notesPlain")?, false)?,
    );
    let mut names = FieldNames::new("1Password");
    custom(&mut item, &mut names, "Source Item", budget.json(source)?);
    let urls = array(overview, "urls", false)?;
    for url in urls {
        text(url, "label")?;
        text(url, "url")?;
    }
    let mut url = text(overview, "url")?;
    if let Some(first) = urls.first().filter(|_| url.is_empty()) {
        url = text(first, "url")?;
    }
    item.fields.insert("URL".into(), budget.text(url, false)?);
    for tag in array(overview, "tags", false)? {
        item.tags.push(tag.as_str().ok_or(INVALID)?.to_owned());
    }
    if let Some(index) = source.get("favIndex") {
        let index = index.as_u64().ok_or(INVALID)?;
        if index > 0 {
            item.fields
                .insert("SecPivot.Favorite".into(), budget.text("true", false)?);
        }
    }
    let mut designations = HashSet::new();
    for field in array(details, "loginFields", false)? {
        let designation = text(field, "designation")?;
        let kind = text(field, "fieldType")?;
        let alternate = text(field, "type")?;
        if !kind.is_empty() && !alternate.is_empty() && kind != alternate {
            return Err(INVALID.into());
        }
        let kind = if kind.is_empty() { alternate } else { kind };
        let value = text(field, "value")?;
        let guarded = match field.get("guarded") {
            None | Some(Json::Null) => false,
            Some(Json::Bool(value)) => *value,
            _ => return Err(INVALID.into()),
        };
        if designation == "username" || designation == "password" {
            if !designations.insert(designation) || (designation == "username" && kind == "P") {
                return Err(INVALID.into());
            }
            let password = designation == "password";
            if !password && guarded {
                custom(&mut item, &mut names, "Username", budget.text(value, true)?);
                continue;
            }
            item.fields.insert(
                if password { "Password" } else { "UserName" }.into(),
                budget.text(value, password)?,
            );
        } else {
            custom(
                &mut item,
                &mut names,
                text(field, "name")?,
                budget.text(value, true)?,
            );
        }
    }
    let mut aliases = BTreeMap::new();
    let mut field_count = 0;
    for section in array(details, "sections", false)? {
        let section_title = text(section, "title")?;
        for field in array(section, "fields", false)? {
            field_count += 1;
            if field_count > 512 {
                return Err(LIMIT.into());
            }
            let value = field.get("value").ok_or(INVALID)?;
            let label = format!("{} {}", section_title, text(field, "title")?);
            if let Some(seed) = object(value)?.get("totp") {
                let seed = seed.as_str().ok_or(INVALID)?;
                if item.fields.contains_key("otp") {
                    return Err(INVALID.into());
                }
                item.fields.insert("otp".into(), budget.text(seed, true)?);
            }
            if let Some(file) = value.get("file") {
                document(file, archive, &mut item, &mut aliases, used, budget, counts)?;
            }
            // Every typed/guarded/unknown value retains protection regardless
            // of a contradictory guarded:false. Source JSON preserves typing.
            custom(&mut item, &mut names, &label, budget.json(value)?);
        }
    }
    if let Some(file) = details.get("documentAttributes") {
        document(file, archive, &mut item, &mut aliases, used, budget, counts)?;
    }
    let archived = match text(source, "state")? {
        "" | "active" => false,
        "archived" => true,
        _ => return Err(INVALID.into()),
    };
    Ok((item, archived))
}

/// Native-only prepared plan. Passwords, OTP, source JSON and binaries stay in
/// protected KeePass Values; it has no Debug/Serialize or plaintext row API.
pub fn prepare_1pux_import(bytes: &[u8]) -> Result<OnePuxImport, String> {
    let archive = parse_1pux_archive(bytes)?;
    let mut budget = Budget::default();
    let mut counts = OnePuxCounts::default();
    let mut used = HashSet::new();
    let mut account_ids = HashSet::new();
    let mut root = budget.group("1Password")?;
    for account in array(archive.data(), "accounts", true)? {
        let attrs = account.get("attrs").ok_or(INVALID)?;
        if !account_ids.insert(identity(attrs)?) {
            return Err(INVALID.into());
        }
        let mut name = text(attrs, "name")?;
        if name.is_empty() {
            name = text(attrs, "accountName")?;
        }
        if name.is_empty() {
            return Err(INVALID.into());
        }
        let mut account_group = budget.group(name)?;
        counts.accounts += 1;
        let mut vault_ids = HashSet::new();
        for vault in array(account, "vaults", true)? {
            let attrs = vault.get("attrs").ok_or(INVALID)?;
            if !vault_ids.insert(identity(attrs)?) {
                return Err(INVALID.into());
            }
            let name = text(attrs, "name")?;
            if name.is_empty() {
                return Err(INVALID.into());
            }
            let mut vault_group = budget.group(name)?;
            let mut archived_group = None;
            counts.vaults += 1;
            let mut item_ids = HashSet::new();
            for source in array(vault, "items", true)? {
                if !item_ids.insert(identity(source)?) {
                    return Err(INVALID.into());
                }
                reserve(&mut counts.items, 1, 8192)?;
                let (item, archived) =
                    prepare_item(source, &archive, &mut used, &mut budget, &mut counts)?;
                if archived {
                    if archived_group.is_none() {
                        archived_group = Some(budget.group("归档")?);
                    }
                    archived_group.as_mut().unwrap().items.push(item);
                    counts.archived += 1;
                } else {
                    vault_group.items.push(item);
                }
            }
            if let Some(group) = archived_group {
                vault_group.children.push(group);
            }
            account_group.children.push(vault_group);
        }
        root.children.push(account_group);
    }
    let mut metadata = PreparedItem::default();
    metadata
        .fields
        .insert("Title".into(), budget.text("1Password 导入元数据", false)?);
    metadata.fields.insert(
        "1Password Export Attributes".into(),
        budget.json(archive.attributes())?,
    );
    metadata
        .fields
        .insert("1Password Export Data".into(), budget.json(archive.data())?);
    for (path, bytes) in archive.files() {
        if !used.contains(path) {
            metadata
                .files
                .insert(path.to_owned(), budget.binary(bytes)?);
            counts.unreferenced_files += 1;
        }
    }
    // Preserve otherwise unassociated binaries/avatars and parent metadata;
    // never silently drop them or guess which item they belong to.
    root.items.push(metadata);
    Ok(OnePuxImport { root, counts })
}

fn insert(parent: &mut GroupMut<'_>, plan: PreparedGroup) {
    let mut group = parent.add_group();
    group.name = plan.name;
    for item in plan.items {
        let mut entry = group.add_entry();
        entry.fields = item.fields.into_iter().collect();
        entry.tags = item.tags;
        entry.quality_check = item.quality;
        if item.created.is_some() {
            entry.times.creation = item.created;
        }
        if item.modified.is_some() {
            entry.times.last_modification = item.modified;
        }
        for (name, bytes) in item.files {
            entry.add_attachment(name, bytes);
        }
    }
    for child in plan.children {
        insert(&mut group, child);
    }
}

impl VaultSession {
    /// Admission check shared by the command's pre-read gate and publication.
    pub(crate) fn validate_1pux_target(&self, target: &str) -> Result<(), String> {
        if self.is_read_only() {
            return Err("数据库只读；未导入任何内容".into());
        }
        let db = self.require_db()?;
        if target != ROOT_GROUP_UUID {
            db.group(parse_group_id(target)?)
                .ok_or("目标分组不存在；未导入任何内容")?;
        }
        Ok(())
    }

    /// One publication of groups/entries/attachments. No earlier group IPCs and
    /// no partial mutation when target validation or final snapshot fails.
    pub fn import_1pux(&mut self, plan: OnePuxImport, target: &str) -> Result<VaultState, String> {
        self.validate_1pux_target(target)?;
        let db = self.require_db()?;
        let target_id = if target == ROOT_GROUP_UUID {
            db.root().id()
        } else {
            let id = parse_group_id(target)?;
            db.group(id).ok_or("目标分组不存在；未导入任何内容")?;
            id
        };
        let mut candidate = db.clone();
        {
            let mut parent = candidate
                .group_mut(target_id)
                .ok_or("目标分组不存在；未导入任何内容")?;
            insert(&mut parent, plan.root);
        }
        let previous = self.db.replace(candidate);
        let previous_meta = (
            self.dirty,
            self.revision,
            self.modified_at.clone(),
            self.cached_snapshot.take(),
        );
        self.mark_dirty();
        match self.snapshot_without_icons() {
            Ok(state) => Ok(state),
            Err(error) => {
                self.db = previous;
                (
                    self.dirty,
                    self.revision,
                    self.modified_at,
                    self.cached_snapshot,
                ) = previous_meta;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
#[path = "onepux_tests.rs"]
mod tests;
