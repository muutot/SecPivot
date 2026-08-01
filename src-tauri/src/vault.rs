//! Vault session: keep the decrypted `keepass::Database` in memory and expose
//! the IPC-facing commands as testable methods. Serialized shapes mirror
//! `src/lib/types/vault.ts`.

use crate::autotype::AutotypeContext;
use crate::remote::{RemoteStorage, REMOTE_URI_PREFIX};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::NaiveDateTime;
use keepass::config::{CompressionConfig, KdfConfig, OuterCipherConfig};
use keepass::db::{EntryId, EntryMut, EntryRef, GroupId, GroupRef, Value, TOTP};
use keepass::{Database, DatabaseKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
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
/// Custom field recording the group an entry lived in before being recycled,
/// so it can be restored to its original location.
const FIELD_ORIGINAL_GROUP: &str = "KeyVault.OriginalGroup";

/// Standard fields that are surfaced through the entry's own columns and must
/// not leak into the custom-fields list.
const RESERVED_FIELDS: [&str; 8] = [
    FIELD_TITLE,
    FIELD_USERNAME,
    FIELD_PASSWORD,
    FIELD_URL,
    FIELD_NOTES,
    FIELD_OTP,
    FIELD_FAVORITE,
    FIELD_ORIGINAL_GROUP,
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
    pub expires: Option<String>,
    pub expired: bool,
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
    pub is_recycle_bin: bool,
    pub children: Vec<VaultGroup>,
    pub entries: Vec<VaultEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultState {
    pub path: String,
    pub file_name: String,
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
    /// ISO-8601 expiry datetime; empty/absent disables expiry.
    #[serde(default)]
    pub expires: Option<String>,
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

/// Server-side security analysis. Passwords never cross into the report.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityReport {
    pub total: usize,
    pub empty: Vec<String>,
    pub weak: Vec<WeakEntry>,
    pub duplicates: Vec<DuplicatePasswords>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeakEntry {
    pub uuid: String,
    pub bits: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicatePasswords {
    pub count: usize,
    pub uuids: Vec<String>,
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// How a remote vault is persisted. `InMemory` uploads to S3 only; `SaveLocal`
/// also mirrors the file under `<app_data>/Storage/remote/<local_dir>/`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteMode {
    InMemory,
    SaveLocal,
}

impl RemoteMode {
    pub fn parse(mode: &str) -> Result<Self, String> {
        match mode {
            "memory" => Ok(RemoteMode::InMemory),
            "local" => Ok(RemoteMode::SaveLocal),
            other => Err(format!(
                "远程保存模式 {other:?} 不受支持 (可用: memory / local)"
            )),
        }
    }
}

/// Where a remote vault lives: the transport, its object key, and how saves
/// should behave. Dropped on `close` so S3 credentials leave memory.
pub struct RemoteTarget {
    pub storage: Arc<dyn RemoteStorage>,
    pub key: String,
    pub mode: RemoteMode,
    pub local_dir: PathBuf,
    pub backup_count: usize,
}

/// The currently open vault. `db` holds the decrypted database; `password`
/// and `keyfile` are kept only for save and cleared on close. `remote`
/// is set when the vault came from S3.
#[derive(Default)]
pub struct VaultSession {
    path: Option<String>,
    password: Option<String>,
    keyfile: Option<Vec<u8>>,
    db: Option<Database>,
    dirty: bool,
    modified_at: String,
    remote: Option<RemoteTarget>,
}

/// Combine password and/or keyfile into a `DatabaseKey`. At least one
/// component must be present.
fn build_database_key(password: &str, keyfile: Option<&[u8]>) -> Result<DatabaseKey, String> {
    if password.is_empty() && keyfile.is_none() {
        return Err("主密码不能为空".to_owned());
    }
    let mut key = DatabaseKey::new();
    if !password.is_empty() {
        key = key.with_password(password);
    }
    if let Some(bytes) = keyfile {
        let mut cursor = std::io::Cursor::new(bytes);
        key = key
            .with_keyfile(&mut cursor)
            .map_err(|e| format!("读取密钥文件失败: {e}"))?;
    }
    Ok(key)
}

impl VaultSession {
    pub fn is_open(&self) -> bool {
        self.db.is_some()
    }

    /// Decrypt and open an existing `.kdbx`, optionally with a keyfile.
    pub fn open(
        &mut self,
        path: &Path,
        password: &str,
        keyfile: Option<&Path>,
    ) -> Result<VaultState, String> {
        let keyfile_bytes = match keyfile {
            Some(keyfile_path) => {
                Some(std::fs::read(keyfile_path).map_err(|e| format!("无法读取密钥文件: {e}"))?)
            }
            None => None,
        };
        let key = build_database_key(password, keyfile_bytes.as_deref())?;
        let data = std::fs::read(path).map_err(|e| format!("无法读取数据库文件: {e}"))?;
        let db = Database::parse(&data, key).map_err(classify_open_error)?;
        self.replace(db, path, password, keyfile_bytes);
        self.snapshot()
    }

    /// Create an empty vault with the given KDF / cipher / compression and
    /// persist it immediately. A keyfile may be attached as a second factor.
    pub fn create(
        &mut self,
        path: &Path,
        password: &str,
        kdf: &str,
        cipher: &str,
        compression: &str,
        keyfile: Option<&Path>,
    ) -> Result<VaultState, String> {
        let keyfile_bytes = match keyfile {
            Some(keyfile_path) => {
                Some(std::fs::read(keyfile_path).map_err(|e| format!("无法读取密钥文件: {e}"))?)
            }
            None => None,
        };
        let key = build_database_key(password, keyfile_bytes.as_deref())?;
        let mut db = Database::new();
        apply_kdf(&mut db, kdf)?;
        apply_cipher(&mut db, cipher)?;
        apply_compression(&mut db, compression)?;
        save_database(&db, path, key)?;
        self.replace(db, path, password, keyfile_bytes);
        self.snapshot()
    }

    /// Open an existing vault stored on S3. `key` is the object key (e.g.
    /// `vaults/private.kdbx`); the display path becomes `s3://<key>`.
    #[allow(clippy::too_many_arguments)]
    pub fn open_remote(
        &mut self,
        storage: Arc<dyn RemoteStorage>,
        key: &str,
        password: &str,
        keyfile: Option<&Path>,
        mode: RemoteMode,
        local_dir: &Path,
        backup_count: usize,
    ) -> Result<VaultState, String> {
        let key = validate_remote_key(key)?;
        let keyfile_bytes = match keyfile {
            Some(keyfile_path) => {
                Some(std::fs::read(keyfile_path).map_err(|e| format!("无法读取密钥文件: {e}"))?)
            }
            None => None,
        };
        let db_key = build_database_key(password, keyfile_bytes.as_deref())?;
        let data = storage
            .get(&key)
            .map_err(|e| format!("下载远程文件失败: {e}"))?;
        let db = Database::parse(&data, db_key).map_err(classify_open_error)?;
        if mode == RemoteMode::SaveLocal {
            write_local_copy(local_dir, &remote_key_basename(&key), &data, backup_count)?;
        }
        self.remote = Some(RemoteTarget {
            storage,
            key: key.clone(),
            mode,
            local_dir: local_dir.to_path_buf(),
            backup_count,
        });
        self.path = Some(format!("{REMOTE_URI_PREFIX}{key}"));
        self.password = Some(password.to_owned());
        self.keyfile = keyfile_bytes;
        self.db = Some(db);
        self.dirty = false;
        self.modified_at = now_iso();
        self.snapshot()
    }

    /// Create an empty vault and upload it to S3 immediately.
    #[allow(clippy::too_many_arguments)]
    pub fn create_remote(
        &mut self,
        storage: Arc<dyn RemoteStorage>,
        key: &str,
        password: &str,
        kdf: &str,
        cipher: &str,
        compression: &str,
        keyfile: Option<&Path>,
        mode: RemoteMode,
        local_dir: &Path,
        backup_count: usize,
    ) -> Result<VaultState, String> {
        let key = validate_remote_key(key)?;
        let keyfile_bytes = match keyfile {
            Some(keyfile_path) => {
                Some(std::fs::read(keyfile_path).map_err(|e| format!("无法读取密钥文件: {e}"))?)
            }
            None => None,
        };
        let db_key = build_database_key(password, keyfile_bytes.as_deref())?;
        let mut db = Database::new();
        apply_kdf(&mut db, kdf)?;
        apply_cipher(&mut db, cipher)?;
        apply_compression(&mut db, compression)?;
        let mut buffer = Vec::new();
        db.save(&mut Cursor::new(&mut buffer), db_key)
            .map_err(|e| format!("序列化数据库失败: {e}"))?;
        storage
            .put(&key, &buffer)
            .map_err(|e| format!("上传远程文件失败: {e}"))?;
        if mode == RemoteMode::SaveLocal {
            write_local_copy(local_dir, &remote_key_basename(&key), &buffer, backup_count)?;
        }
        self.remote = Some(RemoteTarget {
            storage,
            key: key.clone(),
            mode,
            local_dir: local_dir.to_path_buf(),
            backup_count,
        });
        self.path = Some(format!("{REMOTE_URI_PREFIX}{key}"));
        self.password = Some(password.to_owned());
        self.keyfile = keyfile_bytes;
        self.db = Some(db);
        self.dirty = false;
        self.modified_at = now_iso();
        self.snapshot()
    }

    pub fn close(&mut self) {
        self.path = None;
        self.password = None;
        self.keyfile = None;
        self.db = None;
        self.dirty = false;
        self.modified_at.clear();
        self.remote = None;
    }

    pub fn state(&self) -> Result<Option<VaultState>, String> {
        if !self.is_open() {
            return Ok(None);
        }
        Ok(Some(self.snapshot()?))
    }

    pub fn save(&mut self) -> Result<VaultState, String> {
        let password = self.require_password()?;
        let key = build_database_key(password, self.keyfile.as_deref())?;
        self.save_with_key(&key)?;
        self.dirty = false;
        self.modified_at = now_iso();
        self.snapshot()
    }

    /// Re-encrypt and persist the vault with a new master key (password
    /// and/or keyfile). The session continues with the new key.
    pub fn change_master_key(
        &mut self,
        password: &str,
        keyfile: Option<&Path>,
    ) -> Result<VaultState, String> {
        let keyfile_bytes = match keyfile {
            Some(keyfile_path) => {
                Some(std::fs::read(keyfile_path).map_err(|e| format!("无法读取密钥文件: {e}"))?)
            }
            None => None,
        };
        let key = build_database_key(password, keyfile_bytes.as_deref())?;
        self.save_with_key(&key)?;
        self.password = Some(password.to_owned());
        self.keyfile = keyfile_bytes;
        self.dirty = false;
        self.modified_at = now_iso();
        self.snapshot()
    }

    /// Serialize the database with `key` and persist to the remote target or
    /// the local path of the current session.
    fn save_with_key(&self, key: &DatabaseKey) -> Result<(), String> {
        let db = self.require_db()?;
        let mut buffer = Vec::new();
        db.save(&mut Cursor::new(&mut buffer), key.clone())
            .map_err(|e| format!("序列化数据库失败: {e}"))?;
        if let Some(remote) = &self.remote {
            remote
                .storage
                .put(&remote.key, &buffer)
                .map_err(|e| format!("上传远程文件失败: {e}"))?;
            if remote.mode == RemoteMode::SaveLocal {
                write_local_copy(
                    &remote.local_dir,
                    &remote_key_basename(&remote.key),
                    &buffer,
                    remote.backup_count,
                )
                .map_err(|e| format!("保存本地副本失败: {e}"))?;
            }
            return Ok(());
        }
        let path = self.require_path()?.to_owned();
        write_database_bytes(Path::new(&path), &buffer)
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

    /// Move an entry to the recycle bin (or permanently delete it when it is
    /// already inside the recycle bin).
    pub fn delete_entry(&mut self, uuid: &str) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let bin_id = ensure_recycle_bin(db)?;
            let in_bin = {
                let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
                entry.parent().id() == bin_id
            };
            if in_bin {
                let entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
                entry.remove();
            } else {
                let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
                if entry.get(FIELD_ORIGINAL_GROUP).is_none() {
                    let original = entry.parent_mut().id().uuid().to_string();
                    entry.set(FIELD_ORIGINAL_GROUP, Value::unprotected(original));
                }
                entry
                    .move_to(bin_id)
                    .map_err(|e| format!("移入回收站失败: {e}"))?;
            }
        }
        self.mark_dirty();
        self.snapshot()
    }

    /// Restore a recycled entry to its original group (or root when the
    /// original group no longer exists).
    pub fn restore_entry(&mut self, uuid: &str) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let bin_id = recycle_bin_id(db).ok_or_else(|| "回收站不存在".to_owned())?;
            let (in_bin, original_group) = {
                let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
                (
                    entry.parent().id() == bin_id,
                    entry
                        .get(FIELD_ORIGINAL_GROUP)
                        .map(|value| value.to_owned())
                        .and_then(|uuid| parse_group_id(&uuid).ok()),
                )
            };
            if !in_bin {
                return Err("只有回收站中的条目可以恢复".to_owned());
            }
            let target = match original_group {
                Some(group_id) if db.group(group_id).is_some() => group_id,
                _ => db.root().id(),
            };
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            entry
                .move_to(target)
                .map_err(|e| format!("恢复条目失败: {e}"))?;
            entry.fields.remove(FIELD_ORIGINAL_GROUP);
        }
        self.mark_dirty();
        self.snapshot()
    }

    /// Delete a group: move the whole subtree to the recycle bin, or
    /// permanently delete it when it is already inside the recycle bin.
    pub fn delete_group(&mut self, uuid: &str) -> Result<VaultState, String> {
        if uuid == ROOT_GROUP_UUID {
            return Err("不能删除根分组".to_owned());
        }
        {
            let db = self.require_db_mut()?;
            let id = parse_group_id(uuid)?;
            let bin_id = ensure_recycle_bin(db)?;
            if id == bin_id {
                return Err("请先清空或移动回收站内容,再删除回收站".to_owned());
            }
            if group_contains(db, bin_id, id) {
                let group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
                group.remove();
            } else {
                let mut group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
                group
                    .move_to(bin_id)
                    .map_err(|e| format!("移入回收站失败: {e}"))?;
            }
        }
        self.mark_dirty();
        self.snapshot()
    }

    /// Restore a recycled group back to the root.
    pub fn restore_group(&mut self, uuid: &str) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let id = parse_group_id(uuid)?;
            let bin_id = recycle_bin_id(db).ok_or_else(|| "回收站不存在".to_owned())?;
            if !group_contains(db, bin_id, id) {
                return Err("只有回收站中的分组可以恢复".to_owned());
            }
            if id == bin_id {
                return Err("回收站本身不能恢复".to_owned());
            }
            let root_id = db.root().id();
            let mut group = db.group_mut(id).ok_or_else(|| "分组不存在".to_owned())?;
            group
                .move_to(root_id)
                .map_err(|e| format!("恢复分组失败: {e}"))?;
        }
        self.mark_dirty();
        self.snapshot()
    }

    /// Permanently delete every entry and group inside the recycle bin,
    /// keeping the empty recycle bin group itself.
    pub fn empty_recycle_bin(&mut self) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let bin_id = recycle_bin_id(db).ok_or_else(|| "回收站不存在".to_owned())?;
            let (entries, children) = {
                let bin = db.group(bin_id).ok_or_else(|| "回收站不存在".to_owned())?;
                (
                    bin.entries().map(|e| e.id()).collect::<Vec<EntryId>>(),
                    bin.groups().map(|g| g.id()).collect::<Vec<GroupId>>(),
                )
            };
            for entry_id in entries {
                if let Some(entry) = db.entry_mut(entry_id) {
                    entry.remove();
                }
            }
            for child_id in children {
                if let Some(group) = db.group_mut(child_id) {
                    group.remove();
                }
            }
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

    // -- internals ----------------------------------------------------------

    fn replace(&mut self, db: Database, path: &Path, password: &str, keyfile: Option<Vec<u8>>) {
        self.path = Some(path.to_string_lossy().into_owned());
        self.password = Some(password.to_owned());
        self.keyfile = keyfile;
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
        Ok(VaultState {
            path: path.to_owned(),
            file_name: Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(path)
                .to_owned(),
            root: build_group_tree(db),
            dirty: self.dirty,
            modified_at: self.modified_at.clone(),
        })
    }

    /// Fetch a single entry's password on demand (never part of `VaultState`).
    pub fn get_entry_password(&self, uuid: &str) -> Result<String, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        Ok(entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned())
    }

    /// Analyze all entries server-side; no passwords leave the session.
    pub fn security_report(&self) -> Result<SecurityReport, String> {
        let db = self.require_db()?;
        let mut total = 0usize;
        let mut empty: Vec<String> = Vec::new();
        let mut weak: Vec<WeakEntry> = Vec::new();
        let mut by_password: HashMap<String, Vec<String>> = HashMap::new();

        fn scan(
            group: &keepass::db::GroupRef<'_>,
            total: &mut usize,
            empty: &mut Vec<String>,
            weak: &mut Vec<WeakEntry>,
            by_password: &mut HashMap<String, Vec<String>>,
        ) {
            for entry in group.entries() {
                *total += 1;
                let password = entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned();
                if password.is_empty() {
                    empty.push(entry.id().uuid().to_string());
                    continue;
                }
                let bits = estimate_entropy(&password);
                if bits < 72 {
                    weak.push(WeakEntry {
                        uuid: entry.id().uuid().to_string(),
                        bits,
                    });
                }
                by_password
                    .entry(password)
                    .or_default()
                    .push(entry.id().uuid().to_string());
            }
            for child in group.groups() {
                scan(&child, total, empty, weak, by_password);
            }
        }
        scan(
            &db.root(),
            &mut total,
            &mut empty,
            &mut weak,
            &mut by_password,
        );

        weak.sort_by_key(|w| w.bits);
        let mut duplicates: Vec<DuplicatePasswords> = by_password
            .into_iter()
            .filter(|(_, uuids)| uuids.len() > 1)
            .map(|(_, uuids)| {
                let count = uuids.len();
                DuplicatePasswords { count, uuids }
            })
            .collect();
        duplicates.sort_by_key(|d| std::cmp::Reverse(d.count));

        Ok(SecurityReport {
            total,
            empty,
            weak,
            duplicates,
        })
    }

    /// Export all entries as CSV (passwords included) straight to a file.
    pub fn export_csv(&self, path: &str) -> Result<(), String> {
        let db = self.require_db()?;
        let mut lines = vec!["Group,Title,Username,Password,URL,Notes,TOTP,Favorite".to_owned()];

        fn walk(group: &keepass::db::GroupRef<'_>, group_path: &str, lines: &mut Vec<String>) {
            for entry in group.entries() {
                let favorite = if entry.get(FIELD_FAVORITE) == Some(FIELD_FAVORITE_TRUE) {
                    "true"
                } else {
                    "false"
                };
                let row = [
                    escape_csv(group_path),
                    escape_csv(entry.get_title().unwrap_or_default()),
                    escape_csv(entry.get(FIELD_USERNAME).unwrap_or_default()),
                    escape_csv(entry.get(FIELD_PASSWORD).unwrap_or_default()),
                    escape_csv(entry.get(FIELD_URL).unwrap_or_default()),
                    escape_csv(entry.get(FIELD_NOTES).unwrap_or_default()),
                    escape_csv(entry.get_raw_otp_value().unwrap_or_default()),
                    escape_csv(favorite),
                ];
                lines.push(row.join(","));
            }
            for child in group.groups() {
                let child_path = if group_path.is_empty() {
                    child.name.clone()
                } else {
                    format!("{group_path} / {}", child.name)
                };
                walk(&child, &child_path, lines);
            }
        }
        walk(&db.root(), "", &mut lines);

        let content = format!("\u{FEFF}{}\r\n", lines.join("\r\n"));
        std::fs::write(path, content).map_err(|e| format!("写入文件失败: {e}"))
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
        is_recycle_bin: false,
        children: root_ref
            .groups()
            .map(|g| build_group(&g, ROOT_GROUP_UUID, db.meta.recyclebin_uuid))
            .collect(),
        entries: root_ref
            .entries()
            .map(|e| build_entry(&e, ROOT_GROUP_UUID))
            .collect(),
    }
}

fn build_group(
    group: &GroupRef<'_>,
    parent_uuid: &str,
    recyclebin_uuid: Option<Uuid>,
) -> VaultGroup {
    let uuid = group.id().uuid().to_string();
    VaultGroup {
        uuid: uuid.clone(),
        parent_uuid: Some(parent_uuid.to_owned()),
        name: group.name.clone(),
        icon: None,
        is_recycle_bin: Some(group.id().uuid()) == recyclebin_uuid,
        children: group
            .groups()
            .map(|g| build_group(&g, &uuid, recyclebin_uuid))
            .collect(),
        entries: group.entries().map(|e| build_entry(&e, &uuid)).collect(),
    }
}

fn build_entry(entry: &EntryRef<'_>, group_uuid: &str) -> VaultEntry {
    VaultEntry {
        uuid: entry.id().uuid().to_string(),
        group_uuid: group_uuid.to_owned(),
        title: entry.get_title().unwrap_or_default().to_owned(),
        username: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
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
        expires: entry.times.expiry.map(format_iso),
        expired: entry
            .times
            .expiry
            .is_some_and(|expiry| expiry < chrono::Utc::now().naive_utc()),
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

/// RFC 4180 cell escaping: quote when the value contains separators or quotes.
fn escape_csv(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Mirror of the frontend `estimateEntropy` (`src/lib/utils/password.ts`).
fn estimate_entropy(password: &str) -> u32 {
    let mut pool = 0u32;
    if password.chars().any(|c| c.is_ascii_uppercase()) {
        pool += 26;
    }
    if password.chars().any(|c| c.is_ascii_lowercase()) {
        pool += 26;
    }
    if password.chars().any(|c| c.is_ascii_digit()) {
        pool += 10;
    }
    if password.chars().any(|c| !c.is_ascii_alphanumeric()) {
        pool += 32;
    }
    if pool == 0 {
        return 0;
    }
    let length = password.chars().count() as f64;
    (length * (pool as f64).log2()).round() as u32
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
    // Expiry: an ISO datetime enables expiry; an empty/absent value clears it.
    match parse_expiry(input.expires.as_deref()) {
        Some(expiry) => {
            entry.times.expiry = Some(expiry);
            entry.times.expires = Some(true);
        }
        None => {
            entry.times.expiry = None;
            entry.times.expires = Some(false);
        }
    }
}

/// Parse an ISO-8601 expiry string (optionally with `Z` suffix) into a
/// `NaiveDateTime`. Returns `None` for empty input; rejects invalid formats.
fn parse_expiry(value: Option<&str>) -> Option<NaiveDateTime> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    let normalized = raw.strip_suffix('Z').unwrap_or(raw);
    NaiveDateTime::parse_from_str(normalized, "%Y-%m-%dT%H:%M:%S").ok()
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

/// The recycle bin group id, when the database has one.
fn recycle_bin_id(db: &Database) -> Option<GroupId> {
    db.meta.recyclebin_uuid.map(GroupId::from_uuid)
}

/// Return the recycle bin group id, creating the group under root on first use.
fn ensure_recycle_bin(db: &mut Database) -> Result<GroupId, String> {
    if let Some(id) = recycle_bin_id(db) {
        if db.group(id).is_some() {
            return Ok(id);
        }
    }
    let bin_id = {
        let mut root = db.root_mut();
        let mut bin = root.add_group();
        bin.name = "回收站".to_owned();
        bin.id()
    };
    db.meta.recyclebin_uuid = Some(bin_id.uuid());
    Ok(bin_id)
}

/// Whether `group_id` is `ancestor` itself or nested inside it.
fn group_contains(db: &Database, ancestor: GroupId, group_id: GroupId) -> bool {
    let mut current = Some(group_id);
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        current = db
            .group(id)
            .and_then(|group| group.parent().map(|p| p.id()));
    }
    false
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

fn save_database(db: &Database, path: &Path, key: DatabaseKey) -> Result<(), String> {
    let mut buffer = Vec::new();
    db.save(&mut Cursor::new(&mut buffer), key)
        .map_err(|e| format!("序列化数据库失败: {e}"))?;
    write_database_bytes(path, &buffer)
}

/// Atomic write of already-serialized KDBX bytes (local vault save).
fn write_database_bytes(path: &Path, buffer: &[u8]) -> Result<(), String> {
    let tmp = path.with_extension("kdbx.tmp");
    std::fs::write(&tmp, buffer).map_err(|e| format!("写入数据库失败: {e}"))?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("保存数据库失败: {e}"));
    }
    Ok(())
}

/// Validate an S3 object key for a vault file.
fn validate_remote_key(key: &str) -> Result<String, String> {
    let key = key.trim().trim_start_matches('/').to_owned();
    if key.is_empty() {
        return Err("远程文件 Key 不能为空".to_owned());
    }
    if !key.ends_with(".kdbx") {
        return Err("远程文件必须以 .kdbx 结尾".to_owned());
    }
    Ok(key)
}

/// Basename of an S3 object key, used as the local mirror file name.
fn remote_key_basename(key: &str) -> String {
    key.rsplit('/').next().unwrap_or(key).to_owned()
}

/// Write the local mirror of a remote vault under `dir`, rotating up to
/// `backup_count` timestamped `.bak` copies of the previous file first.
fn write_local_copy(
    dir: &Path,
    name: &str,
    bytes: &[u8],
    backup_count: usize,
) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("创建本地目录失败: {e}"))?;
    let dest = dir.join(name);
    if backup_count > 0 && dest.exists() {
        let stem = dest
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(name)
            .to_owned();
        let ext = dest
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("kdbx")
            .to_owned();
        let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S%.3f");
        let backup = dir.join(format!("{stem}.{stamp}.{ext}.bak"));
        std::fs::rename(&dest, &backup).map_err(|e| format!("创建本地备份失败: {e}"))?;
        prune_local_backups(dir, &stem, &ext, backup_count)?;
    }
    std::fs::write(&dest, bytes).map_err(|e| format!("写入本地副本失败: {e}"))
}

/// Keep only the newest `keep` backup files matching `<stem>.<ts>.<ext>.bak`.
fn prune_local_backups(dir: &Path, stem: &str, ext: &str, keep: usize) -> Result<(), String> {
    let mut backups: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("读取本地备份目录失败: {e}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_owned();
            name.starts_with(&format!("{stem}.")) && name.ends_with(&format!(".{ext}.bak"))
        })
        .collect();
    backups.sort();
    let total = backups.len();
    for path in backups.into_iter().take(total.saturating_sub(keep)) {
        let _ = std::fs::remove_file(path);
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
            .create(&path, "master-password", "Aes", "Aes256", "None", None)
            .unwrap();
        (session, path)
    }

    /// Write a KeePass-style binary keyfile and return its path.
    fn write_keyfile(dir: &TempDir) -> std::path::PathBuf {
        let keyfile = dir.path().join("test.key");
        let mut bytes = Vec::new();
        for i in 0..64u8 {
            bytes.push(i.wrapping_mul(7).wrapping_add(3));
        }
        std::fs::write(&keyfile, bytes).unwrap();
        keyfile
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
        let state = reopened.open(&path, "master-password", None).unwrap();
        assert_eq!(state.root.children.len(), 0);
        assert!(reopened.is_open());
    }

    #[test]
    fn wrong_password_is_rejected() {
        let dir = TempDir::new().unwrap();
        let (session, path) = create_session(&dir);
        drop(session);
        let mut reopened = VaultSession::default();
        let err = reopened.open(&path, "wrong", None).unwrap_err();
        assert!(err.contains("密码"), "unexpected error: {err}");
    }

    #[test]
    fn empty_password_is_rejected() {
        let dir = TempDir::new().unwrap();
        let (_session, path) = create_session(&dir);
        let err = VaultSession::default().open(&path, "", None).unwrap_err();
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
                expires: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let group = &state.root.children[0];
        assert_eq!(group.entries.len(), 1);
        let entry = &group.entries[0];
        assert_eq!(entry.title, "GitHub");
        assert_eq!(session.get_entry_password(&entry.uuid).unwrap(), "s3cret");
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
                    expires: None,
                    custom_fields: vec![],
                    attachments: vec![],
                },
            )
            .unwrap();
        let entry = &state.root.children[0].entries[0];
        assert_eq!(entry.title, "GitHub (work)");
        assert_eq!(session.get_entry_password(&entry_uuid).unwrap(), "s3cret2");
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
        let state = reopened.open(&path, "master-password", None).unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].name, "Mail");
    }

    #[test]
    fn entry_expiry_roundtrip_and_clear() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Expiring".into(),
                username: "u".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: Some("2020-01-01T00:00:00Z".to_owned()),
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let entry = &state.root.entries[0];
        assert_eq!(entry.expires.as_deref(), Some("2020-01-01T00:00:00Z"));
        assert!(entry.expired, "past expiry should be flagged");

        // Clearing the expiry marks the entry as not expired.
        let state = session
            .update_entry(
                &entry.uuid,
                &EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: "Expiring".into(),
                    username: "u".into(),
                    password: "p".into(),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    expires: None,
                    custom_fields: vec![],
                    attachments: vec![],
                },
            )
            .unwrap();
        let entry = &state.root.entries[0];
        assert!(entry.expires.is_none());
        assert!(!entry.expired);

        // A future expiry persists across save/reopen.
        session
            .update_entry(
                &entry.uuid,
                &EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: "Expiring".into(),
                    username: "u".into(),
                    password: "p".into(),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    expires: Some("2099-12-31T23:59:59Z".to_owned()),
                    custom_fields: vec![],
                    attachments: vec![],
                },
            )
            .unwrap();
        session.save().unwrap();
        drop(session);

        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password", None).unwrap();
        let entry = &state.root.entries[0];
        assert_eq!(entry.expires.as_deref(), Some("2099-12-31T23:59:59Z"));
        assert!(!entry.expired);
    }

    #[test]
    fn change_master_key_reencrypts_and_reopens_with_new_credentials() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Loopback".into(),
                username: "root".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let state = session.change_master_key("new-password", None).unwrap();
        assert!(!state.dirty);
        assert_eq!(state.root.entries.len(), 1);
        drop(session);

        // The old password no longer opens the vault.
        let mut wrong = VaultSession::default();
        assert!(wrong.open(&path, "master-password", None).is_err());
        // The new password does, and the entry is intact.
        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "new-password", None).unwrap();
        assert_eq!(state.root.entries.len(), 1);
        assert_eq!(state.root.entries[0].title, "Loopback");
    }

    #[test]
    fn change_master_key_supports_keyfile_only_and_keeps_session_alive() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        let keyfile = dir.path().join("keyfile.key");
        std::fs::write(&keyfile, b"0123456789abcdef0123456789abcdef").unwrap();

        let state = session
            .change_master_key("", Some(&keyfile))
            .expect("keyfile-only vault should save");
        assert!(!state.dirty);
        // Session continues to work with the new key in memory.
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "After".into(),
                username: "u".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        assert_eq!(state.root.entries.len(), 1);
        let saved = session.save().unwrap();
        assert!(!saved.dirty);
        drop(session);

        // Reopens with the keyfile only.
        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "", Some(&keyfile)).unwrap();
        assert_eq!(state.root.entries.len(), 1);
        assert_eq!(state.root.entries[0].title, "After");
    }

    #[test]
    fn delete_group_moves_whole_subtree_to_recycle_bin_and_restores() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "Parent".into(),
            })
            .unwrap();
        let parent_uuid = state.root.children[0].uuid.clone();

        session
            .add_group(&GroupInput {
                parent_uuid: Some(parent_uuid.clone()),
                name: "Child".into(),
            })
            .unwrap();

        session
            .add_entry(&EntryInput {
                group_uuid: parent_uuid.clone(),
                title: "Loopback".into(),
                username: "root".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let entry_uuid = session.state().unwrap().unwrap().root.children[0].entries[0]
            .uuid
            .clone();

        session.delete_group(&parent_uuid).unwrap();
        let root = session.state().unwrap().unwrap().root;
        // The whole subtree now lives under the recycle bin.
        let bin = root
            .children
            .iter()
            .find(|g| g.is_recycle_bin)
            .expect("recycle bin should exist");
        assert_eq!(bin.children.len(), 1);
        assert_eq!(bin.children[0].name, "Parent");
        assert_eq!(bin.children[0].children[0].name, "Child");
        assert_eq!(bin.children[0].entries[0].uuid, entry_uuid);
        assert!(!root.children.iter().any(|g| g.uuid == parent_uuid));

        // Restoring brings the group (with its subtree) back to root.
        session.restore_group(&parent_uuid).unwrap();
        let root = session.state().unwrap().unwrap().root;
        let parent = root
            .children
            .iter()
            .find(|g| g.uuid == parent_uuid)
            .unwrap();
        assert_eq!(parent.children[0].name, "Child");
        assert_eq!(parent.entries[0].uuid, entry_uuid);
    }

    #[test]
    fn recycle_bin_deletes_entry_then_restores_and_empties() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Loopback".into(),
                username: "root".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let entry_uuid = session.state().unwrap().unwrap().root.entries[0]
            .uuid
            .clone();

        // Deleting an entry moves it to the recycle bin.
        session.delete_entry(&entry_uuid).unwrap();
        let root = session.state().unwrap().unwrap().root;
        assert!(root.entries.is_empty());
        let bin = root.children.iter().find(|g| g.is_recycle_bin).unwrap();
        assert_eq!(bin.entries.len(), 1);
        assert_eq!(bin.entries[0].uuid, entry_uuid);

        // Restoring returns it to its original group.
        session.restore_entry(&entry_uuid).unwrap();
        let root = session.state().unwrap().unwrap().root;
        assert_eq!(root.entries[0].uuid, entry_uuid);

        // Deleting again, then emptying the bin permanently removes it.
        session.delete_entry(&entry_uuid).unwrap();
        session.empty_recycle_bin().unwrap();
        let state = session.state().unwrap().unwrap();
        let bin = state
            .root
            .children
            .iter()
            .find(|g| g.is_recycle_bin)
            .unwrap();
        assert!(bin.entries.is_empty());
        session.save().unwrap();
        drop(session);

        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password", None).unwrap();
        assert!(state.root.entries.is_empty());
        let bin = state
            .root
            .children
            .iter()
            .find(|g| g.is_recycle_bin)
            .unwrap();
        assert!(bin.entries.is_empty());
    }

    #[test]
    fn recycle_bin_is_persisted_across_reopen() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Loopback".into(),
                username: "root".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let entry_uuid = session.state().unwrap().unwrap().root.entries[0]
            .uuid
            .clone();
        session.delete_entry(&entry_uuid).unwrap();
        session.save().unwrap();
        drop(session);

        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password", None).unwrap();
        let bin = state
            .root
            .children
            .iter()
            .find(|g| g.is_recycle_bin)
            .unwrap();
        assert_eq!(bin.entries.len(), 1);
        // The recycled entry is still restorable after reopen.
        reopened.restore_entry(&entry_uuid).unwrap();
        let state = reopened.state().unwrap().unwrap();
        assert!(state.root.entries.iter().any(|e| e.uuid == entry_uuid));
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
                expires: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap_err();
        assert!(err.contains("UUID"));

        // Unknown kdf/cipher/compression rejected at create time.
        let path = dir.path().join("bad.kdbx");
        let err = VaultSession::default()
            .create(&path, "pw", "scrypt", "Aes256", "None", None)
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
                expires: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();

        let json = serde_json::to_value(&state).unwrap();
        let obj = json.as_object().unwrap();
        for key in ["path", "fileName", "root", "dirty", "modifiedAt"] {
            assert!(obj.contains_key(key), "missing VaultState key {key}");
        }
        assert!(
            !obj.contains_key("password"),
            "master password leaked in VaultState"
        );
        let root = json["root"].as_object().unwrap();
        for key in ["uuid", "parentUuid", "name", "children", "entries"] {
            assert!(root.contains_key(key), "missing VaultGroup key {key}");
        }
        let group = &json["root"]["children"][0];
        assert_eq!(group["parentUuid"].as_str(), Some(ROOT_GROUP_UUID));
        let entry = &group["entries"][0];
        for key in ["uuid", "groupUuid", "title", "username", "url", "notes"] {
            assert!(entry.get(key).is_some(), "missing VaultEntry key {key}");
        }
        assert!(
            entry.get("password").is_none(),
            "entry password leaked in VaultEntry"
        );
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
                expires: None,
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
                expires: None,
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
                expires: None,
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
                expires: None,
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
            .open(&dir.path().join("test.kdbx"), "master-password", None)
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
                expires: None,
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
                    expires: None,
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
            .open(&dir.path().join("test.kdbx"), "master-password", None)
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
                expires: None,
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
                expires: None,
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

    #[test]
    fn keyfile_round_trip_requires_the_keyfile() {
        let dir = TempDir::new().unwrap();
        let keyfile = write_keyfile(&dir);
        let path = dir.path().join("secured.kdbx");
        let mut session = VaultSession::default();
        session
            .create(
                &path,
                "master-password",
                "Aes",
                "Aes256",
                "None",
                Some(&keyfile),
            )
            .unwrap();
        drop(session);

        let mut reopened = VaultSession::default();
        let err = reopened.open(&path, "master-password", None).unwrap_err();
        assert!(err.contains("密码"), "unexpected error: {err}");

        let mut reopened = VaultSession::default();
        let state = reopened
            .open(&path, "master-password", Some(&keyfile))
            .unwrap();
        assert!(reopened.is_open());
        assert_eq!(state.root.name, "Root");

        let mut session = reopened;
        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "Mail".into(),
            })
            .unwrap();
        session.save().unwrap();
        drop(session);

        let mut reopened = VaultSession::default();
        let state = reopened
            .open(&path, "master-password", Some(&keyfile))
            .unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].name, "Mail");
    }

    #[test]
    fn keyfile_only_database_opens_without_password() {
        let dir = TempDir::new().unwrap();
        let keyfile = write_keyfile(&dir);
        let path = dir.path().join("keyonly.kdbx");
        let mut session = VaultSession::default();
        session
            .create(&path, "", "Aes", "Aes256", "None", Some(&keyfile))
            .unwrap();
        drop(session);

        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "", Some(&keyfile)).unwrap();
        assert!(reopened.is_open());
        assert_eq!(state.root.name, "Root");
        let err = reopened.open(&path, "anything", None).unwrap_err();
        assert!(err.contains("密码"), "unexpected error: {err}");
    }

    #[test]
    fn missing_keyfile_path_is_rejected() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope.key");
        let path = dir.path().join("x.kdbx");
        let err = VaultSession::default()
            .open(&path, "pw", Some(&missing))
            .unwrap_err();
        assert!(err.contains("密钥文件"), "unexpected error: {err}");
    }

    fn add_entry_with_password(
        session: &mut VaultSession,
        group_uuid: &str,
        title: &str,
        password: &str,
    ) -> String {
        let state = session
            .add_entry(&EntryInput {
                group_uuid: group_uuid.to_owned(),
                title: title.into(),
                username: "u".into(),
                password: password.into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        state.root.children[0].entries.last().unwrap().uuid.clone()
    }

    #[test]
    fn get_entry_password_returns_field_on_demand() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "G".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        let uuid = add_entry_with_password(&mut session, &group_uuid, "E", "hunter2");
        assert_eq!(session.get_entry_password(&uuid).unwrap(), "hunter2");
        let err = session.get_entry_password("not-a-uuid").unwrap_err();
        assert!(err.contains("UUID"));
    }

    #[test]
    fn security_report_flags_empty_weak_and_duplicate_passwords() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "G".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();

        let empty_uuid = add_entry_with_password(&mut session, &group_uuid, "Empty", "");
        let weak_uuid = add_entry_with_password(&mut session, &group_uuid, "Weak", "abc");
        let strong_pw = "StrongPass#1!";
        let dup_a = add_entry_with_password(&mut session, &group_uuid, "DupA", strong_pw);
        let dup_b = add_entry_with_password(&mut session, &group_uuid, "DupB", strong_pw);

        let report = session.security_report().unwrap();
        assert_eq!(report.total, 4);
        assert_eq!(report.empty, vec![empty_uuid]);
        assert_eq!(
            report.weak,
            vec![WeakEntry {
                uuid: weak_uuid,
                bits: 14
            }]
        );
        assert_eq!(report.duplicates.len(), 1);
        assert_eq!(report.duplicates[0].count, 2);
        assert!(report.duplicates[0].uuids.contains(&dup_a));
        assert!(report.duplicates[0].uuids.contains(&dup_b));
    }

    #[test]
    fn export_csv_writes_escaped_rows_and_bom() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "Web".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        session
            .add_entry(&EntryInput {
                group_uuid,
                title: "Git,Hub".into(),
                username: "alice".into(),
                password: "s3cret".into(),
                url: "https://x".into(),
                notes: "line1\nline2".into(),
                totp: Some("JBSWY3DPEHPK3PXP".into()),
                expires: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();

        let dest = dir.path().join("export.csv");
        session.export_csv(dest.to_str().unwrap()).unwrap();
        let text = std::fs::read_to_string(&dest).unwrap();

        assert!(text.starts_with('\u{FEFF}'));
        assert!(text.contains("Group,Title,Username,Password,URL,Notes,TOTP,Favorite\r\n"));
        assert!(text.contains("\"Git,Hub\""));
        assert!(text.contains("\"line1\nline2\""));
        assert!(text.contains("Web,\"Git,Hub\",alice,s3cret,https://x"));
        assert!(text.contains("JBSWY3DPEHPK3PXP"));
    }

    /// Create a local vault and seed it into an in-memory remote storage.
    fn seed_remote_storage(dir: &TempDir) -> (crate::remote::MemoryStorage, std::path::PathBuf) {
        let seed_path = dir.path().join("seed.kdbx");
        {
            let mut session = VaultSession::default();
            session
                .create(&seed_path, "pw", "Aes", "Aes256", "None", None)
                .unwrap();
        }
        let storage = crate::remote::MemoryStorage::default();
        storage.seed("vaults/seed.kdbx", std::fs::read(&seed_path).unwrap());
        (storage, seed_path)
    }

    #[test]
    fn remote_open_save_round_trip_via_memory_storage() {
        let dir = TempDir::new().unwrap();
        let (storage, _) = seed_remote_storage(&dir);
        let local = dir.path().join("local");

        let mut session = VaultSession::default();
        let state = session
            .open_remote(
                Arc::new(storage.clone()),
                "vaults/seed.kdbx",
                "pw",
                None,
                RemoteMode::InMemory,
                &local,
                3,
            )
            .unwrap();
        assert_eq!(state.path, "s3://vaults/seed.kdbx");
        assert_eq!(state.file_name, "seed.kdbx");

        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "Web".into(),
            })
            .unwrap();
        let saved = session.save().unwrap();
        assert!(!saved.dirty);

        let mut reopened = VaultSession::default();
        let state = reopened
            .open_remote(
                Arc::new(storage.clone()),
                "vaults/seed.kdbx",
                "pw",
                None,
                RemoteMode::InMemory,
                &local,
                3,
            )
            .unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].name, "Web");
    }

    #[test]
    fn remote_save_local_writes_mirror_and_rotates_backups() {
        let dir = TempDir::new().unwrap();
        let (storage, _) = seed_remote_storage(&dir);
        let local = dir.path().join("mirror");

        let mut session = VaultSession::default();
        session
            .open_remote(
                Arc::new(storage.clone()),
                "vaults/seed.kdbx",
                "pw",
                None,
                RemoteMode::SaveLocal,
                &local,
                1,
            )
            .unwrap();
        assert!(local.join("seed.kdbx").exists());

        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "Mail".into(),
            })
            .unwrap();
        session.save().unwrap();
        session.save().unwrap();
        session.save().unwrap();

        let bytes = std::fs::read(local.join("seed.kdbx")).unwrap();
        let key = DatabaseKey::new().with_password("pw");
        let db = Database::parse(&bytes, key).unwrap();
        assert_eq!(db.root().groups().count(), 1);

        let backups: Vec<_> = std::fs::read_dir(&local)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".kdbx.bak"))
            .collect();
        assert_eq!(backups.len(), 1);
    }

    #[test]
    fn remote_rejects_invalid_key_or_mode() {
        let dir = TempDir::new().unwrap();
        let (storage, _) = seed_remote_storage(&dir);
        let local = dir.path().join("local");
        let mut session = VaultSession::default();

        let err = session
            .open_remote(
                Arc::new(storage.clone()),
                "  /  ",
                "pw",
                None,
                RemoteMode::InMemory,
                &local,
                3,
            )
            .unwrap_err();
        assert!(err.contains("Key"));

        let err = session
            .open_remote(
                Arc::new(storage.clone()),
                "vaults/seed.txt",
                "pw",
                None,
                RemoteMode::InMemory,
                &local,
                3,
            )
            .unwrap_err();
        assert!(err.contains("kdbx"));

        let err = session
            .open_remote(
                Arc::new(storage.clone()),
                "vaults/missing.kdbx",
                "pw",
                None,
                RemoteMode::InMemory,
                &local,
                3,
            )
            .unwrap_err();
        assert!(err.contains("下载"));

        let err = RemoteMode::parse("cloud").unwrap_err();
        assert!(err.contains("模式"));
        assert_eq!(RemoteMode::parse("memory").unwrap(), RemoteMode::InMemory);
        assert_eq!(RemoteMode::parse("local").unwrap(), RemoteMode::SaveLocal);
    }

    #[test]
    fn remote_create_uploads_and_saves_back() {
        let storage = crate::remote::MemoryStorage::default();
        let dir = TempDir::new().unwrap();
        let local = dir.path().join("local");

        let mut session = VaultSession::default();
        let state = session
            .create_remote(
                Arc::new(storage.clone()),
                "new/vault.kdbx",
                "pw",
                "Aes",
                "Aes256",
                "None",
                None,
                RemoteMode::InMemory,
                &local,
                3,
            )
            .unwrap();
        assert_eq!(state.path, "s3://new/vault.kdbx");
        assert!(storage.get("new/vault.kdbx").is_ok());

        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "Web".into(),
            })
            .unwrap();
        session.save().unwrap();

        let mut reopened = VaultSession::default();
        let state = reopened
            .open_remote(
                Arc::new(storage.clone()),
                "new/vault.kdbx",
                "pw",
                None,
                RemoteMode::InMemory,
                &local,
                3,
            )
            .unwrap();
        assert_eq!(state.root.children[0].name, "Web");
    }

    #[test]
    fn remote_close_clears_session_and_storage() {
        let dir = TempDir::new().unwrap();
        let (storage, _) = seed_remote_storage(&dir);
        let local = dir.path().join("local");

        let mut session = VaultSession::default();
        session
            .open_remote(
                Arc::new(storage.clone()),
                "vaults/seed.kdbx",
                "pw",
                None,
                RemoteMode::InMemory,
                &local,
                3,
            )
            .unwrap();
        session.close();
        assert!(!session.is_open());
        assert!(session.state().unwrap().is_none());
    }
}
