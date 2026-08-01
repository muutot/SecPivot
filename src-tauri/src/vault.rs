//! Vault session: keep the decrypted `keepass::Database` in memory and expose
//! the IPC-facing commands as testable methods. Serialized shapes mirror
//! `src/lib/types/vault.ts`.

use crate::autotype::AutotypeContext;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::NaiveDateTime;
use keepass::config::{CompressionConfig, KdfConfig, OuterCipherConfig};
use keepass::db::{EntryId, EntryMut, EntryRef, GroupId, GroupRef, Value, TOTP};
use keepass::{Database, DatabaseKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Virtual root group id used by the frontend; maps to the DB root group.
pub const ROOT_GROUP_UUID: &str = "root";
pub const ROOT_GROUP_NAME: &str = "Root";

const FIELD_TITLE: &str = "Title";
const FIELD_USERNAME: &str = "UserName";
const FIELD_PASSWORD: &str = "Password";
const FIELD_URL: &str = "URL";
const FIELD_NOTES: &str = "Notes";
const FIELD_OTP: &str = "otp";
/// Custom field used to mark an entry as pinned/favorite.
const FIELD_FAVORITE: &str = "KeyVault.Favorite";
const FIELD_FAVORITE_TRUE: &str = "true";

/// Standard fields that are surfaced through the entry's own columns and must
/// not leak into the custom-fields list.
const RESERVED_FIELDS: [&str; 7] = [
    FIELD_TITLE,
    FIELD_USERNAME,
    FIELD_PASSWORD,
    FIELD_URL,
    FIELD_NOTES,
    FIELD_OTP,
    FIELD_FAVORITE,
];

// Argon2 parameters for newly created vaults (OWASP-recommended).
const ARGON2_ITERATIONS: u64 = 3;
const ARGON2_MEMORY_KIB: u32 = 65536; // 64 MiB
const ARGON2_PARALLELISM: u32 = 4;
// KeePass default for the legacy AES-KDF.
const AES_KDF_ROUNDS: u64 = 600_000;

// ---------------------------------------------------------------------------
// Serde DTOs (camelCase on the wire)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultEntry {
    pub uuid: String,
    pub group_uuid: String,
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub totp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    pub favorite: bool,
    pub custom_fields: Vec<CustomField>,
    pub attachments: Vec<AttachmentInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomField {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentInfo {
    pub name: String,
    pub size: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentInput {
    pub name: String,
    #[serde(default)]
    pub size: usize,
    /// Base64-encoded content. Absent when the attachment already exists and
    /// should be kept as-is.
    #[serde(default)]
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultGroup {
    pub uuid: String,
    pub parent_uuid: Option<String>,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<u32>,
    pub children: Vec<VaultGroup>,
    pub entries: Vec<VaultEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultState {
    pub path: String,
    pub file_name: String,
    pub password: String,
    pub root: VaultGroup,
    pub dirty: bool,
    pub modified_at: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryInput {
    pub group_uuid: String,
    pub title: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    #[serde(default)]
    pub totp: Option<String>,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
    #[serde(default)]
    pub attachments: Vec<AttachmentInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInput {
    pub parent_uuid: Option<String>,
    pub name: String,
}

/// A computed one-time code for display with a local countdown.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpCode {
    pub code: String,
    /// Seconds until this code expires (1..=period).
    pub valid_for: u64,
    /// Total period in seconds (usually 30).
    pub period: u64,
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// The currently open vault. `db` holds the decrypted database; `password`
/// is kept only for save and zeroized on close.
#[derive(Default)]
pub struct VaultSession {
    path: Option<String>,
    password: Option<String>,
    db: Option<Database>,
    dirty: bool,
    modified_at: String,
}

impl VaultSession {
    pub fn is_open(&self) -> bool {
        self.db.is_some()
    }

    /// Decrypt and open an existing `.kdbx`.
    pub fn open(&mut self, path: &Path, password: &str) -> Result<VaultState, String> {
        if password.is_empty() {
            return Err("主密码不能为空".to_owned());
        }
        let data = std::fs::read(path).map_err(|e| format!("无法读取数据库文件: {e}"))?;
        let key = DatabaseKey::new().with_password(password);
        let db = Database::parse(&data, key).map_err(classify_open_error)?;
        self.replace(db, path, password);
        self.snapshot()
    }

    /// Create an empty vault with the given KDF / cipher / compression and
    /// persist it immediately.
    pub fn create(
        &mut self,
        path: &Path,
        password: &str,
        kdf: &str,
        cipher: &str,
        compression: &str,
    ) -> Result<VaultState, String> {
        if password.is_empty() {
            return Err("主密码不能为空".to_owned());
        }
        let mut db = Database::new();
        apply_kdf(&mut db, kdf)?;
        apply_cipher(&mut db, cipher)?;
        apply_compression(&mut db, compression)?;
        save_database(&db, path, password)?;
        self.replace(db, path, password);
        self.snapshot()
    }

    pub fn close(&mut self) {
        self.path = None;
        self.password = None;
        self.db = None;
        self.dirty = false;
        self.modified_at.clear();
    }

    pub fn state(&self) -> Result<Option<VaultState>, String> {
        if !self.is_open() {
            return Ok(None);
        }
        Ok(Some(self.snapshot()?))
    }

    pub fn save(&mut self) -> Result<VaultState, String> {
        let path = self.require_path()?.to_owned();
        let password = self.require_password()?.to_owned();
        let db = self.require_db()?;
        save_database(db, Path::new(&path), &password)?;
        self.dirty = false;
        self.modified_at = now_iso();
        self.snapshot()
    }

    pub fn add_entry(&mut self, input: &EntryInput) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let mut group = if input.group_uuid == ROOT_GROUP_UUID {
                db.root_mut()
            } else {
                let group_id = parse_group_id(&input.group_uuid)?;
                db.group_mut(group_id)
                    .ok_or_else(|| "目标分组不存在".to_owned())?
            };
            let mut entry = group.add_entry();
            write_fields(&mut entry, input);
            sync_custom_fields(&mut entry, &input.custom_fields);
            sync_attachments(&mut entry, &input.attachments)?;
        }
        self.mark_dirty();
        self.snapshot()
    }

    pub fn update_entry(&mut self, uuid: &str, input: &EntryInput) -> Result<VaultState, String> {
        let id = parse_entry_id(uuid)?;
        let target_group = resolve_group_id(self.require_db()?, &input.group_uuid)?;
        {
            let db = self.require_db_mut()?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            if entry.parent_mut().id() != target_group {
                entry
                    .move_to(target_group)
                    .map_err(|e| format!("移动条目失败: {e}"))?;
            }
            write_fields(&mut entry, input);
            sync_custom_fields(&mut entry, &input.custom_fields);
            sync_attachments(&mut entry, &input.attachments)?;
        }
        self.mark_dirty();
        self.snapshot()
    }

    pub fn delete_entry(&mut self, uuid: &str) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            entry.remove();
        }
        self.mark_dirty();
        self.snapshot()
    }

    /// Write an entry attachment to an arbitrary destination path.
    pub fn save_attachment(&self, uuid: &str, name: &str, dest: &str) -> Result<(), String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        let attachment = entry
            .attachment_by_name(name)
            .ok_or_else(|| "附件不存在".to_owned())?;
        std::fs::write(dest, attachment.data.get()).map_err(|e| format!("写入附件失败: {e}"))
    }

    /// Toggle the favorite/pin marker on an entry (persisted as a custom field).
    pub fn toggle_favorite(&mut self, uuid: &str) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            if entry.get(FIELD_FAVORITE) == Some(FIELD_FAVORITE_TRUE) {
                entry.fields.remove(FIELD_FAVORITE);
            } else {
                entry.set(
                    FIELD_FAVORITE,
                    Value::unprotected(FIELD_FAVORITE_TRUE.to_owned()),
                );
            }
        }
        self.mark_dirty();
        self.snapshot()
    }

    /// Compute the current TOTP code for an entry that carries an `otp` seed.
    pub fn totp_code(&self, uuid: &str) -> Result<TotpCode, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        let seed = entry
            .get_raw_otp_value()
            .ok_or_else(|| "该条目没有 TOTP 种子".to_owned())?;
        compute_totp_now(seed)
    }

    /// Collect the fields an auto-type sequence can substitute, for the given entry.
    pub fn autotype_context(&self, uuid: &str) -> Result<AutotypeContext, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        Ok(AutotypeContext {
            username: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
            password: entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
            title: entry.get_title().unwrap_or_default().to_owned(),
            url: entry.get(FIELD_URL).unwrap_or_default().to_owned(),
            notes: entry.get(FIELD_NOTES).unwrap_or_default().to_owned(),
        })
    }

    pub fn add_group(&mut self, input: &GroupInput) -> Result<VaultState, String> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err("分组名称不能为空".to_owned());
        }
        {
            let db = self.require_db_mut()?;
            let mut parent = match input.parent_uuid.as_deref() {
                None | Some(ROOT_GROUP_UUID) => db.root_mut(),
                Some(parent) => {
                    let parent_id = parse_group_id(parent)?;
                    db.group_mut(parent_id)
                        .ok_or_else(|| "父分组不存在".to_owned())?
                }
            };
            let mut group = parent.add_group();
            group.name = name.to_owned();
        }
        self.mark_dirty();
        self.snapshot()
    }

    pub fn rename_group(&mut self, uuid: &str, name: &str) -> Result<VaultState, String> {
        if uuid == ROOT_GROUP_UUID {
            return Err("不能重命名根分组".to_owned());
        }
        let name = name.trim();
        if name.is_empty() {
            return Err("分组名称不能为空".to_owned());
        }
        {
            let db = self.require_db_mut()?;
            let id = parse_group_id(uuid)?;
            let mut group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
            group.name = name.to_owned();
        }
        self.mark_dirty();
        self.snapshot()
    }

    /// Delete a group; its entries and child groups bubble up to the root.
    pub fn delete_group(&mut self, uuid: &str) -> Result<VaultState, String> {
        if uuid == ROOT_GROUP_UUID {
            return Err("不能删除根分组".to_owned());
        }
        {
            let db = self.require_db_mut()?;
            let id = parse_group_id(uuid)?;
            let root_id = db.root().id();
            let (entries, children) = {
                let group = db.group(id).ok_or_else(|| "分组不存在".to_owned())?;
                (
                    group.entries().map(|e| e.id()).collect::<Vec<EntryId>>(),
                    group.groups().map(|g| g.id()).collect::<Vec<GroupId>>(),
                )
            };
            for entry_id in entries {
                let mut entry = db
                    .entry_mut(entry_id)
                    .ok_or_else(|| "条目不存在".to_owned())?;
                entry
                    .move_to(root_id)
                    .map_err(|e| format!("移动条目失败: {e}"))?;
            }
            for child_id in children {
                let mut child = db
                    .group_mut(child_id)
                    .ok_or_else(|| "子分组不存在".to_owned())?;
                child
                    .move_to(root_id)
                    .map_err(|e| format!("移动分组失败: {e}"))?;
            }
            let group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
            group.remove();
        }
        self.mark_dirty();
        self.snapshot()
    }

    // -- internals ----------------------------------------------------------

    fn replace(&mut self, db: Database, path: &Path, password: &str) {
        self.path = Some(path.to_string_lossy().into_owned());
        self.password = Some(password.to_owned());
        self.db = Some(db);
        self.dirty = false;
        self.modified_at = now_iso();
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.modified_at = now_iso();
    }

    fn require_db(&self) -> Result<&Database, String> {
        self.db.as_ref().ok_or_else(|| "数据库未打开".to_owned())
    }

    fn require_db_mut(&mut self) -> Result<&mut Database, String> {
        self.db.as_mut().ok_or_else(|| "数据库未打开".to_owned())
    }

    fn require_path(&self) -> Result<&str, String> {
        self.path
            .as_deref()
            .ok_or_else(|| "数据库未打开".to_owned())
    }

    fn require_password(&self) -> Result<&str, String> {
        self.password
            .as_deref()
            .ok_or_else(|| "数据库未打开".to_owned())
    }

    fn snapshot(&self) -> Result<VaultState, String> {
        let db = self.require_db()?;
        let path = self.require_path()?;
        let password = self.require_password()?;
        Ok(VaultState {
            path: path.to_owned(),
            file_name: Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(path)
                .to_owned(),
            password: password.to_owned(),
            root: build_group_tree(db),
            dirty: self.dirty,
            modified_at: self.modified_at.clone(),
        })
    }
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

fn build_group_tree(db: &Database) -> VaultGroup {
    let root_ref = db.root();
    VaultGroup {
        uuid: ROOT_GROUP_UUID.to_owned(),
        parent_uuid: None,
        name: ROOT_GROUP_NAME.to_owned(),
        icon: None,
        children: root_ref
            .groups()
            .map(|g| build_group(&g, ROOT_GROUP_UUID))
            .collect(),
        entries: root_ref
            .entries()
            .map(|e| build_entry(&e, ROOT_GROUP_UUID))
            .collect(),
    }
}

fn build_group(group: &GroupRef<'_>, parent_uuid: &str) -> VaultGroup {
    let uuid = group.id().uuid().to_string();
    VaultGroup {
        uuid: uuid.clone(),
        parent_uuid: Some(parent_uuid.to_owned()),
        name: group.name.clone(),
        icon: None,
        children: group.groups().map(|g| build_group(&g, &uuid)).collect(),
        entries: group.entries().map(|e| build_entry(&e, &uuid)).collect(),
    }
}

fn build_entry(entry: &EntryRef<'_>, group_uuid: &str) -> VaultEntry {
    VaultEntry {
        uuid: entry.id().uuid().to_string(),
        group_uuid: group_uuid.to_owned(),
        title: entry.get_title().unwrap_or_default().to_owned(),
        username: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
        password: entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
        url: entry.get(FIELD_URL).unwrap_or_default().to_owned(),
        notes: entry.get(FIELD_NOTES).unwrap_or_default().to_owned(),
        totp: entry.get_raw_otp_value().map(str::to_owned),
        icon: None,
        created: entry.times.creation.map(format_iso),
        modified: entry.times.last_modification.map(format_iso),
        tags: if entry.tags.is_empty() {
            None
        } else {
            Some(entry.tags.join(", "))
        },
        favorite: entry.get(FIELD_FAVORITE) == Some(FIELD_FAVORITE_TRUE),
        custom_fields: {
            let mut fields: Vec<CustomField> = entry
                .fields
                .iter()
                .filter(|(name, _)| !name.is_empty() && !RESERVED_FIELDS.contains(&name.as_str()))
                .map(|(name, value)| CustomField {
                    name: name.clone(),
                    value: value.get().clone(),
                })
                .collect();
            fields.sort_by(|a, b| a.name.cmp(&b.name));
            fields
        },
        attachments: entry
            .attachments_named()
            .map(|(name, attachment)| AttachmentInfo {
                name: name.to_owned(),
                size: attachment.data.get().len(),
            })
            .collect(),
    }
}

fn format_iso(time: NaiveDateTime) -> String {
    time.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

fn write_fields(entry: &mut EntryMut<'_>, input: &EntryInput) {
    entry.set(FIELD_TITLE, Value::unprotected(input.title.clone()));
    entry.set(FIELD_USERNAME, Value::unprotected(input.username.clone()));
    // The password is sensitive; protect it in memory.
    entry.set(FIELD_PASSWORD, Value::protected(input.password.clone()));
    entry.set(FIELD_URL, Value::unprotected(input.url.clone()));
    entry.set(FIELD_NOTES, Value::unprotected(input.notes.clone()));
    // TOTP seed: a raw Base32 key or an otpauth:// URI; absent = remove.
    match input.totp.as_deref() {
        Some(seed) if !seed.trim().is_empty() => {
            entry.set(FIELD_OTP, Value::unprotected(seed.trim().to_owned()));
        }
        _ => {
            entry.fields.remove(FIELD_OTP);
        }
    }
}

/// Replace the entry's custom fields with the given list, keeping standard
/// columns (Title, UserName, …) untouched and dropping empty or reserved names.
fn sync_custom_fields(entry: &mut EntryMut<'_>, fields: &[CustomField]) {
    let mut desired = HashMap::new();
    for field in fields {
        let name = field.name.trim().to_owned();
        if name.is_empty() || RESERVED_FIELDS.contains(&name.as_str()) {
            continue;
        }
        desired.insert(name, field.value.clone());
    }
    let current: Vec<String> = entry.fields.keys().cloned().collect();
    for name in current {
        if !RESERVED_FIELDS.contains(&name.as_str()) && !desired.contains_key(&name) {
            entry.fields.remove(&name);
        }
    }
    for (name, value) in desired {
        entry.set(name, Value::unprotected(value));
    }
}

/// Make the entry's attachment set match the given list. Names that no longer
/// appear are removed; entries carrying `data` are added or replaced.
fn sync_attachments(
    entry: &mut EntryMut<'_>,
    attachments: &[AttachmentInput],
) -> Result<(), String> {
    let desired: Vec<&AttachmentInput> = attachments
        .iter()
        .filter(|a| !a.name.trim().is_empty())
        .collect();
    let current: Vec<String> = entry
        .as_ref()
        .attachments_named()
        .map(|(name, _)| name.to_owned())
        .collect();
    for name in current {
        if !desired.iter().any(|a| a.name == name) {
            entry.remove_attachment_by_name(&name);
        }
    }
    for attachment in desired {
        if let Some(data) = &attachment.data {
            let bytes = BASE64
                .decode(data.trim())
                .map_err(|e| format!("附件数据解码失败: {e}"))?;
            entry.add_attachment(attachment.name.clone(), Value::protected(bytes));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_entry_id(s: &str) -> Result<EntryId, String> {
    Uuid::parse_str(s)
        .map(EntryId::from_uuid)
        .map_err(|_| format!("无效的条目 UUID: {s}"))
}

fn parse_group_id(s: &str) -> Result<GroupId, String> {
    Uuid::parse_str(s)
        .map(GroupId::from_uuid)
        .map_err(|_| format!("无效的分组 UUID: {s}"))
}

/// Map the virtual `"root"` id to the DB root group id, validating the rest.
fn resolve_group_id(db: &Database, uuid: &str) -> Result<GroupId, String> {
    if uuid == ROOT_GROUP_UUID {
        return Ok(db.root().id());
    }
    let id = parse_group_id(uuid)?;
    if db.group(id).is_some() {
        Ok(id)
    } else {
        Err("目标分组不存在".to_owned())
    }
}

/// Keepass `TOTP` parses `otpauth://` URIs only. Raw Base32 keys are wrapped
/// into a URI with RFC 6238 defaults (SHA-1, 6 digits, 30s period).
fn normalize_totp_seed(seed: &str) -> String {
    let trimmed = seed.trim();
    if trimmed.to_ascii_lowercase().starts_with("otpauth://") {
        trimmed.to_owned()
    } else {
        let secret = trimmed.replace([' ', '-'], "").to_uppercase();
        format!("otpauth://totp/KeyVault?secret={secret}&digits=6&period=30")
    }
}

/// Compute the code at a specific unix timestamp (deterministic; used by tests).
fn compute_totp_at(seed: &str, unix_time: u64) -> Result<TotpCode, String> {
    let totp =
        TOTP::from_str(&normalize_totp_seed(seed)).map_err(|e| format!("TOTP 种子无效: {e}"))?;
    let code = totp.value_at(unix_time);
    Ok(TotpCode {
        code: code.code,
        valid_for: code.valid_for.as_secs(),
        period: code.period.as_secs(),
    })
}

/// Compute the code for the current time.
fn compute_totp_now(seed: &str) -> Result<TotpCode, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("读取系统时间失败: {e}"))?
        .as_secs();
    compute_totp_at(seed, now)
}

fn classify_open_error<E: std::fmt::Display>(e: E) -> String {
    let msg = format!("{e}");
    let lower = msg.to_lowercase();
    if lower.contains("hmac")
        || lower.contains("mac")
        || lower.contains("decrypt")
        || lower.contains("padding")
        || lower.contains("key")
    {
        "无法打开数据库: 密码或密钥文件错误".to_owned()
    } else {
        format!("无法打开数据库: {msg}")
    }
}

fn save_database(db: &Database, path: &Path, password: &str) -> Result<(), String> {
    let key = DatabaseKey::new().with_password(password);
    let mut buffer = Vec::new();
    db.save(&mut Cursor::new(&mut buffer), key)
        .map_err(|e| format!("序列化数据库失败: {e}"))?;
    let tmp = path.with_extension("kdbx.tmp");
    std::fs::write(&tmp, &buffer).map_err(|e| format!("写入数据库失败: {e}"))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("保存数据库失败: {e}"));
    }
    Ok(())
}

fn apply_kdf(db: &mut Database, kdf: &str) -> Result<(), String> {
    db.config.kdf_config = match kdf {
        "Argon2id" => KdfConfig::Argon2id {
            iterations: ARGON2_ITERATIONS,
            memory: u64::from(ARGON2_MEMORY_KIB) * 1024,
            parallelism: ARGON2_PARALLELISM,
            version: argon2::Version::Version13,
        },
        "Argon2" => KdfConfig::Argon2 {
            iterations: ARGON2_ITERATIONS,
            memory: u64::from(ARGON2_MEMORY_KIB) * 1024,
            parallelism: ARGON2_PARALLELISM,
            version: argon2::Version::Version13,
        },
        "Aes" => KdfConfig::Aes {
            rounds: AES_KDF_ROUNDS,
        },
        other => {
            return Err(format!(
                "kdf 取值 {other:?} 不受支持 (可用: Argon2id / Argon2 / Aes)"
            ))
        }
    };
    Ok(())
}

fn apply_cipher(db: &mut Database, cipher: &str) -> Result<(), String> {
    db.config.outer_cipher_config = match cipher {
        "Aes256" => OuterCipherConfig::AES256,
        "ChaCha20" => OuterCipherConfig::ChaCha20,
        other => {
            return Err(format!(
                "cipher 取值 {other:?} 不受支持 (可用: Aes256 / ChaCha20)"
            ))
        }
    };
    Ok(())
}

fn apply_compression(db: &mut Database, compression: &str) -> Result<(), String> {
    db.config.compression_config = match compression {
        "None" => CompressionConfig::None,
        "Gzip" => CompressionConfig::GZip,
        other => {
            return Err(format!(
                "compression 取值 {other:?} 不受支持 (可用: None / Gzip)"
            ))
        }
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_session(dir: &TempDir) -> (VaultSession, std::path::PathBuf) {
        let path = dir.path().join("test.kdbx");
        let mut session = VaultSession::default();
        session
            .create(&path, "master-password", "Aes", "Aes256", "None")
            .unwrap();
        (session, path)
    }

    #[test]
    fn create_then_reopen_round_trip() {
        let dir = TempDir::new().unwrap();
        let (session, path) = create_session(&dir);
        let state = session.state().unwrap().unwrap();
        assert_eq!(state.root.name, "Root");
        assert_eq!(state.root.uuid, ROOT_GROUP_UUID);
        assert_eq!(state.file_name, "test.kdbx");
        drop(session);

        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password").unwrap();
        assert_eq!(state.root.children.len(), 0);
        assert!(reopened.is_open());
    }

    #[test]
    fn wrong_password_is_rejected() {
        let dir = TempDir::new().unwrap();
        let (session, path) = create_session(&dir);
        drop(session);
        let mut reopened = VaultSession::default();
        let err = reopened.open(&path, "wrong").unwrap_err();
        assert!(err.contains("密码"), "unexpected error: {err}");
    }

    #[test]
    fn empty_password_is_rejected() {
        let dir = TempDir::new().unwrap();
        let (_session, path) = create_session(&dir);
        let err = VaultSession::default().open(&path, "").unwrap_err();
        assert!(err.contains("主密码"));
        assert!(!VaultSession::default().is_open());
    }

    #[test]
    fn group_and_entry_crud_flow() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "  Web  ".into(),
            })
            .unwrap();
        let group = &state.root.children[0];
        assert_eq!(group.name, "Web");
        assert_eq!(group.parent_uuid.as_deref(), Some(ROOT_GROUP_UUID));

        let state = session
            .add_entry(&EntryInput {
                group_uuid: group.uuid.clone(),
                title: "GitHub".into(),
                username: "alice".into(),
                password: "s3cret".into(),
                url: "https://github.com".into(),
                notes: "work".into(),
                totp: Some("JBSWY3DPEHPK3PXP".into()),
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let group = &state.root.children[0];
        assert_eq!(group.entries.len(), 1);
        let entry = &group.entries[0];
        assert_eq!(entry.title, "GitHub");
        assert_eq!(entry.password, "s3cret");
        assert_eq!(entry.totp.as_deref(), Some("JBSWY3DPEHPK3PXP"));
        let entry_uuid = entry.uuid.clone();
        assert!(entry.created.is_some());
        assert!(entry.modified.is_some());
        assert!(state.dirty);

        let state = session
            .update_entry(
                &entry_uuid,
                &EntryInput {
                    group_uuid: group.uuid.clone(),
                    title: "GitHub (work)".into(),
                    username: "alice".into(),
                    password: "s3cret2".into(),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    custom_fields: vec![],
                    attachments: vec![],
                },
            )
            .unwrap();
        let entry = &state.root.children[0].entries[0];
        assert_eq!(entry.title, "GitHub (work)");
        assert_eq!(entry.password, "s3cret2");
        assert!(entry.totp.is_none());

        let state = session.rename_group(&group.uuid, "Accounts").unwrap();
        assert_eq!(state.root.children[0].name, "Accounts");

        let state = session.delete_entry(&entry_uuid).unwrap();
        assert_eq!(state.root.children[0].entries.len(), 0);
    }

    #[test]
    fn save_clears_dirty_and_persists() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "Mail".into(),
            })
            .unwrap();
        let saved = session.save().unwrap();
        assert!(!saved.dirty);
        drop(session);

        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password").unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].name, "Mail");
    }

    #[test]
    fn delete_group_bubbles_entries_and_children_to_root() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "Parent".into(),
            })
            .unwrap();
        let parent_uuid = state.root.children[0].uuid.clone();

        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(parent_uuid.clone()),
                name: "Child".into(),
            })
            .unwrap();
        let child_uuid = session.state().unwrap().unwrap().root.children[0].children[0]
            .uuid
            .clone();
        let _ = state;

        let state = session
            .add_entry(&EntryInput {
                group_uuid: parent_uuid.clone(),
                title: "Loopback".into(),
                username: "root".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let entry_uuid = session.state().unwrap().unwrap().root.children[0].entries[0]
            .uuid
            .clone();
        let _ = state;

        session.delete_group(&parent_uuid).unwrap();
        let root = session.state().unwrap().unwrap().root;
        // The entry bubbled to root.
        assert!(
            root.entries.iter().any(|e| e.uuid == entry_uuid),
            "entry should have bubbled to root"
        );
        // The child group bubbled to root.
        assert!(
            root.children.iter().any(|g| g.uuid == child_uuid),
            "child group should have bubbled to root"
        );
        assert!(!root.children.iter().any(|g| g.uuid == parent_uuid));
    }

    #[test]
    fn rejects_invalid_parameters() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        let err = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "   ".into(),
            })
            .unwrap_err();
        assert!(err.contains("分组名称"));

        let err = session
            .add_group(&GroupInput {
                parent_uuid: Some("not-a-uuid".into()),
                name: "X".into(),
            })
            .unwrap_err();
        assert!(err.contains("UUID"));

        let err = session.delete_group(ROOT_GROUP_UUID).unwrap_err();
        assert!(err.contains("根分组"));

        let err = session
            .add_entry(&EntryInput {
                group_uuid: "missing".into(),
                title: "T".into(),
                username: "".into(),
                password: "".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap_err();
        assert!(err.contains("UUID"));

        // Unknown kdf/cipher/compression rejected at create time.
        let path = dir.path().join("bad.kdbx");
        let err = VaultSession::default()
            .create(&path, "pw", "scrypt", "Aes256", "None")
            .unwrap_err();
        assert!(err.contains("kdf"));
    }

    #[test]
    fn dto_wire_format_uses_camel_case() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "Web".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        let state = session
            .add_entry(&EntryInput {
                group_uuid,
                title: "GitHub".into(),
                username: "alice".into(),
                password: "s3cret".into(),
                url: "https://github.com".into(),
                notes: "work".into(),
                totp: Some("JBSWY3DPEHPK3PXP".into()),
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();

        let json = serde_json::to_value(&state).unwrap();
        let obj = json.as_object().unwrap();
        for key in [
            "path",
            "fileName",
            "password",
            "root",
            "dirty",
            "modifiedAt",
        ] {
            assert!(obj.contains_key(key), "missing VaultState key {key}");
        }
        let root = json["root"].as_object().unwrap();
        for key in ["uuid", "parentUuid", "name", "children", "entries"] {
            assert!(root.contains_key(key), "missing VaultGroup key {key}");
        }
        let group = &json["root"]["children"][0];
        assert_eq!(group["parentUuid"].as_str(), Some(ROOT_GROUP_UUID));
        let entry = &group["entries"][0];
        for key in [
            "uuid",
            "groupUuid",
            "title",
            "username",
            "password",
            "url",
            "notes",
        ] {
            assert!(entry.get(key).is_some(), "missing VaultEntry key {key}");
        }
        assert!(entry["totp"].is_string());
        // Optional fields absent on the entry are skipped entirely (not null).
        assert!(entry.get("icon").is_none());
        assert!(entry.get("tags").is_none());
        // Favorite is always present and a boolean.
        assert!(entry["favorite"].is_boolean());
    }

    #[test]
    fn inputs_deserialize_from_camel_case() {
        let entry: EntryInput = serde_json::from_value(serde_json::json!({
            "groupUuid": "g1",
            "title": "T",
            "username": "u",
            "password": "p",
            "url": "https://x",
            "notes": "n",
            "totp": null,
        }))
        .unwrap();
        assert_eq!(entry.group_uuid, "g1");
        assert_eq!(entry.totp, None);

        let group: GroupInput = serde_json::from_value(serde_json::json!({
            "parentUuid": null,
            "name": "Root",
        }))
        .unwrap();
        assert_eq!(group.parent_uuid, None);

        let nested: GroupInput = serde_json::from_value(serde_json::json!({
            "parentUuid": "abc",
            "name": "Web",
        }))
        .unwrap();
        assert_eq!(nested.parent_uuid.as_deref(), Some("abc"));
    }

    #[test]
    fn totp_computes_rfc6238_vector_codes() {
        // RFC 6238 Appendix B: secret = ASCII "12345678901234567890".
        let seed =
            "otpauth://totp/RFC6238:test?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&digits=8&period=30";
        let at_59 = compute_totp_at(seed, 59).unwrap();
        assert_eq!(at_59.code, "94287082");
        assert_eq!(at_59.period, 30);
        assert_eq!(at_59.valid_for, 1);
        let at_2e9 = compute_totp_at(seed, 2_000_000_000).unwrap();
        assert_eq!(at_2e9.code, "69279037");
    }

    #[test]
    fn totp_accepts_raw_base32_seed() {
        // Same secret as above, provided as a raw Base32 key → SHA-1 / 6 digits.
        let at_59 = compute_totp_at("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59).unwrap();
        assert_eq!(at_59.code, "287082");
        assert_eq!(at_59.period, 30);
        assert_eq!(at_59.valid_for, 1);
    }

    #[test]
    fn totp_rejects_invalid_seed() {
        let err = compute_totp_at("INVALID!", 59).unwrap_err();
        assert!(err.contains("TOTP"), "unexpected error: {err}");
    }

    #[test]
    fn totp_code_requires_totp_field() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "G".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        let state = session
            .add_entry(&EntryInput {
                group_uuid,
                title: "Plain".into(),
                username: "".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.children[0].entries[0].uuid.clone();
        let err = session.totp_code(&uuid).unwrap_err();
        assert!(err.contains("TOTP"));
    }

    #[test]
    fn totp_code_session_returns_current_code() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "G".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        let state = session
            .add_entry(&EntryInput {
                group_uuid,
                title: "2FA".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: Some("JBSWY3DPEHPK3PXP".into()),
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.children[0].entries[0].uuid.clone();
        let code = session.totp_code(&uuid).unwrap();
        assert_eq!(code.code.len(), 6);
        assert_eq!(code.period, 30);
        assert!((1..=code.period).contains(&code.valid_for));
    }

    #[test]
    fn totp_code_wire_format_uses_camel_case() {
        let code = compute_totp_at("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59).unwrap();
        let json = serde_json::to_value(&code).unwrap();
        let obj = json.as_object().unwrap();
        for key in ["code", "validFor", "period"] {
            assert!(obj.contains_key(key), "missing TotpCode key {key}");
        }
    }

    #[test]
    fn toggle_favorite_round_trips_field() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "G".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        let state = session
            .add_entry(&EntryInput {
                group_uuid,
                title: "E".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.children[0].entries[0].uuid.clone();

        assert!(!session.snapshot().unwrap().root.children[0].entries[0].favorite);
        session.toggle_favorite(&uuid).unwrap();
        assert!(session.snapshot().unwrap().root.children[0].entries[0].favorite);
        // Second toggle removes the marker again.
        session.toggle_favorite(&uuid).unwrap();
        assert!(!session.snapshot().unwrap().root.children[0].entries[0].favorite);
    }

    #[test]
    fn favorite_persists_after_save_and_reopen() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "G".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        let state = session
            .add_entry(&EntryInput {
                group_uuid,
                title: "E".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.children[0].entries[0].uuid.clone();
        session.toggle_favorite(&uuid).unwrap();
        session.save().unwrap();
        drop(session);

        let mut reopened = VaultSession::default();
        let _ = reopened
            .open(&dir.path().join("test.kdbx"), "master-password")
            .unwrap();
        let favorite = reopened.snapshot().unwrap().root.children[0].entries[0].favorite;
        assert!(favorite);
    }

    #[test]
    fn custom_fields_and_attachments_round_trip() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "G".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();

        let data = BASE64.encode(b"hello attachment".as_slice());
        let state = session
            .add_entry(&EntryInput {
                group_uuid: group_uuid.clone(),
                title: "E".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                custom_fields: vec![
                    CustomField {
                        name: "PIN".into(),
                        value: "1234".into(),
                    },
                    CustomField {
                        name: "Question".into(),
                        value: "Answer".into(),
                    },
                ],
                attachments: vec![AttachmentInput {
                    name: "note.txt".into(),
                    size: data.len(),
                    data: Some(data),
                }],
            })
            .unwrap();
        let entry = &state.root.children[0].entries[0];
        assert_eq!(entry.custom_fields.len(), 2);
        assert_eq!(
            entry
                .custom_fields
                .iter()
                .find(|f| f.name == "PIN")
                .map(|f| f.value.as_str()),
            Some("1234")
        );
        assert_eq!(entry.attachments.len(), 1);
        assert_eq!(entry.attachments[0].name, "note.txt");
        assert_eq!(entry.attachments[0].size, b"hello attachment".len());
        let uuid = entry.uuid.clone();

        // Update: drop one field, keep the attachment untouched (no data), add one.
        let state = session
            .update_entry(
                &uuid,
                &EntryInput {
                    group_uuid: group_uuid.clone(),
                    title: "E".into(),
                    username: "u".into(),
                    password: "pw".into(),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    custom_fields: vec![CustomField {
                        name: "PIN".into(),
                        value: "9999".into(),
                    }],
                    attachments: vec![
                        AttachmentInput {
                            name: "note.txt".into(),
                            size: 0,
                            data: None,
                        },
                        AttachmentInput {
                            name: "second.bin".into(),
                            size: 0,
                            data: Some(BASE64.encode([1u8, 2, 3, 4].as_slice())),
                        },
                    ],
                },
            )
            .unwrap();
        let entry = &state.root.children[0].entries[0];
        assert_eq!(entry.custom_fields.len(), 1);
        assert_eq!(entry.custom_fields[0].name, "PIN");
        assert_eq!(entry.custom_fields[0].value, "9999");
        assert_eq!(entry.attachments.len(), 2);
        let note = entry
            .attachments
            .iter()
            .find(|a| a.name == "note.txt")
            .expect("note.txt attachment present");
        assert_eq!(note.size, b"hello attachment".len());

        // Persist and reopen: everything survives.
        session.save().unwrap();
        drop(session);
        let mut reopened = VaultSession::default();
        let state = reopened
            .open(&dir.path().join("test.kdbx"), "master-password")
            .unwrap();
        let entry = &state.root.children[0].entries[0];
        assert_eq!(entry.custom_fields.len(), 1);
        assert_eq!(entry.attachments.len(), 2);
    }

    #[test]
    fn custom_fields_exclude_reserved_names() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "G".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        let state = session
            .add_entry(&EntryInput {
                group_uuid,
                title: "E".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "n".into(),
                totp: None,
                custom_fields: vec![
                    CustomField {
                        name: FIELD_OTP.to_owned(),
                        value: "should-not-appear".into(),
                    },
                    CustomField {
                        name: FIELD_TITLE.to_owned(),
                        value: "should-not-appear".into(),
                    },
                    CustomField {
                        name: "   ".into(),
                        value: "ignored".into(),
                    },
                    CustomField {
                        name: "Nickname".into(),
                        value: "alice".into(),
                    },
                ],
                attachments: vec![],
            })
            .unwrap();
        let entry = &state.root.children[0].entries[0];
        assert_eq!(entry.custom_fields.len(), 1);
        assert_eq!(entry.custom_fields[0].name, "Nickname");
    }

    #[test]
    fn save_attachment_writes_file() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "G".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        let payload = b"\x00\x01binary data\xff".to_vec();
        let state = session
            .add_entry(&EntryInput {
                group_uuid,
                title: "E".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                custom_fields: vec![],
                attachments: vec![AttachmentInput {
                    name: "blob.bin".into(),
                    size: payload.len(),
                    data: Some(BASE64.encode(payload.clone())),
                }],
            })
            .unwrap();
        let uuid = state.root.children[0].entries[0].uuid.clone();
        let dest = dir.path().join("out.bin");
        session
            .save_attachment(&uuid, "blob.bin", dest.to_str().unwrap())
            .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), payload);
    }
}
