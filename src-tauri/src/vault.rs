//! Vault session: keep the decrypted `keepass::Database` in memory and expose
//! the IPC-facing commands as testable methods. Serialized shapes mirror
//! `src/lib/types/vault.ts`.

use crate::autotype::{self, AutotypeContext};
use crate::remote::{RemoteStorage, REMOTE_URI_PREFIX};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::NaiveDateTime;
use keepass::config::{CompressionConfig, KdfConfig, OuterCipherConfig};
use keepass::db::{
    Color, Entry, EntryId, EntryMut, EntryRef, GroupId, GroupRef, History, Icon, Times, Value, TOTP,
};
use keepass::{Database, DatabaseKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use zeroize::Zeroize;

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
    /// Whether the entry carries a TOTP seed. The seed itself is never part
    /// of the snapshot: the renderer fetches codes via `totp_code` or the
    /// seed on demand via `get_entry_totp`.
    pub has_totp: bool,
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
    /// Built-in KeePass icon index (0-68); absent = default icon.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
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
    /// Built-in KeePass icon index; absent = default icon.
    #[serde(default)]
    pub icon: Option<u32>,
    /// `#RRGGBB` background color; empty/absent clears it.
    #[serde(default)]
    pub color: Option<String>,
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
    /// Built-in KeePass icon index; absent = default icon.
    #[serde(default)]
    pub icon: Option<u32>,
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

/// A single historical snapshot of an entry (see `Entry.history`). Passwords
/// are never serialized: restoring a version happens server-side by index.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryVersion {
    /// Position in the history list (0 = most recent snapshot).
    pub index: usize,
    pub modified: Option<String>,
    pub title: String,
    pub username: String,
    pub url: String,
    pub notes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    pub custom_fields: Vec<CustomField>,
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
#[derive(Clone)]
pub struct RemoteTarget {
    pub storage: Arc<dyn RemoteStorage>,
    pub key: String,
    pub mode: RemoteMode,
    pub local_dir: PathBuf,
    pub backup_count: usize,
}

/// The currently open vault. `db` holds the decrypted database; `password`
/// and `keyfile` are kept only for save and cleared on close. `remote`
/// is set when the vault came from S3. `revision` counts edits so a save
/// completing after a concurrent edit does not clear the dirty flag, and so
/// `snapshot` can reuse a cached tree instead of rebuilding it every call.
#[derive(Default)]
pub struct VaultSession {
    path: Option<String>,
    password: Option<String>,
    keyfile: Option<Vec<u8>>,
    db: Option<Database>,
    dirty: bool,
    modified_at: String,
    remote: Option<RemoteTarget>,
    revision: u64,
    cached_snapshot: Option<(u64, VaultState)>,
}

/// Wipe a secret `String` in place, then drop it (buffer is zeroed before
/// the heap allocation is freed). Best-effort: only this owned copy is
/// cleared — copies made by the OS, IPC, or `DatabaseKey` internals are
/// outside our control (`keepass` zeroizes its own key material on drop).
fn wipe_secret_string(secret: &mut String) {
    secret.zeroize();
}

/// Wipe a secret byte buffer in place (see `wipe_secret_string`).
fn wipe_secret_bytes(secret: &mut Vec<u8>) {
    secret.zeroize();
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

/// Where a save should land. Captured cheaply under the session lock; the
/// slow work (KDF, serialization, network, disk) happens outside it.
pub(crate) enum SaveTarget {
    Local(PathBuf),
    Remote {
        storage: Arc<dyn RemoteStorage>,
        key: String,
        mode: RemoteMode,
        local_dir: PathBuf,
        backup_count: usize,
    },
}

/// Everything `persist_save` needs. `password`/`keyfile` are clones so the
/// KDF can run without the lock; both are zeroized after use. `revision`
/// records the session's edit counter so the completion step can tell whether
/// edits landed while the save ran.
pub(crate) struct SaveJob {
    pub db: Database,
    pub password: String,
    pub keyfile: Option<Vec<u8>>,
    pub target: SaveTarget,
    pub revision: u64,
}

/// Read an optional keyfile into memory (called without the session lock).
pub(crate) fn read_keyfile(keyfile: Option<&Path>) -> Result<Option<Vec<u8>>, String> {
    match keyfile {
        Some(keyfile_path) => std::fs::read(keyfile_path)
            .map(Some)
            .map_err(|e| format!("无法读取密钥文件: {e}")),
        None => Ok(None),
    }
}

/// Lock-free half of `open`: read the file, build the key (KDF) and parse
/// the database. Returns the decrypted `Database` plus the keyfile bytes.
pub(crate) fn prepare_local_open(
    path: &Path,
    password: &str,
    keyfile: Option<&Path>,
) -> Result<(Database, Option<Vec<u8>>), String> {
    let keyfile_bytes = read_keyfile(keyfile)?;
    let key = build_database_key(password, keyfile_bytes.as_deref())?;
    let data = std::fs::read(path).map_err(|e| format!("无法读取数据库文件: {e}"))?;
    let db = Database::parse(&data, key).map_err(classify_open_error)?;
    Ok((db, keyfile_bytes))
}

/// Lock-free half of `create`: build the database, run the KDF and write the
/// new vault file. Returns the database plus the keyfile bytes.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_local_create(
    path: &Path,
    password: &str,
    kdf: &str,
    cipher: &str,
    compression: &str,
    keyfile: Option<&Path>,
) -> Result<(Database, Option<Vec<u8>>), String> {
    let keyfile_bytes = read_keyfile(keyfile)?;
    let key = build_database_key(password, keyfile_bytes.as_deref())?;
    let mut db = Database::new();
    apply_kdf(&mut db, kdf)?;
    apply_cipher(&mut db, cipher)?;
    apply_compression(&mut db, compression)?;
    save_database(&db, path, key)?;
    Ok((db, keyfile_bytes))
}

/// Lock-free half of `open_remote`: validate the key, download from S3
/// (network), build the key (KDF), parse, and mirror locally if requested.
/// Returns the database, keyfile bytes and the normalized object key.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_remote_open(
    storage: Arc<dyn RemoteStorage>,
    key: &str,
    password: &str,
    keyfile: Option<&Path>,
    mode: RemoteMode,
    local_dir: &Path,
    backup_count: usize,
) -> Result<(Database, Option<Vec<u8>>, String), String> {
    let key = validate_remote_key(key)?;
    let keyfile_bytes = read_keyfile(keyfile)?;
    let db_key = build_database_key(password, keyfile_bytes.as_deref())?;
    let data = storage
        .get(&key)
        .map_err(|e| format!("下载远程文件失败: {e}"))?;
    let db = Database::parse(&data, db_key).map_err(classify_open_error)?;
    if mode == RemoteMode::SaveLocal {
        write_local_copy(local_dir, &remote_key_basename(&key), &data, backup_count)?;
    }
    Ok((db, keyfile_bytes, key))
}

/// Lock-free half of `create_remote`: build the database, run the KDF,
/// serialize, upload to S3 (network) and mirror locally if requested.
/// Returns the database, keyfile bytes and the normalized object key.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare_remote_create(
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
) -> Result<(Database, Option<Vec<u8>>, String), String> {
    let key = validate_remote_key(key)?;
    let keyfile_bytes = read_keyfile(keyfile)?;
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
    Ok((db, keyfile_bytes, key))
}

/// Serialize `db` with `key` and persist it to the given target. Runs
/// entirely outside the session lock.
pub(crate) fn persist_snapshot(
    db: &Database,
    key: &DatabaseKey,
    target: &SaveTarget,
) -> Result<(), String> {
    let mut buffer = Vec::new();
    db.save(&mut Cursor::new(&mut buffer), key.clone())
        .map_err(|e| format!("序列化数据库失败: {e}"))?;
    match target {
        SaveTarget::Remote {
            storage,
            key,
            mode,
            local_dir,
            backup_count,
        } => {
            storage
                .put(key, &buffer)
                .map_err(|e| format!("上传远程文件失败: {e}"))?;
            if *mode == RemoteMode::SaveLocal {
                write_local_copy(local_dir, &remote_key_basename(key), &buffer, *backup_count)
                    .map_err(|e| format!("保存本地副本失败: {e}"))?;
            }
            Ok(())
        }
        SaveTarget::Local(path) => write_database_bytes(path, &buffer),
    }
}

/// Full lock-free save: derive the session key (KDF), then serialize and
/// persist. Secret clones are zeroized afterwards.
pub(crate) fn persist_save(job: SaveJob) -> Result<(), String> {
    let key = build_database_key(&job.password, job.keyfile.as_deref())?;
    let result = persist_snapshot(&job.db, &key, &job.target);
    let mut password = job.password;
    wipe_secret_string(&mut password);
    if let Some(mut keyfile) = job.keyfile {
        wipe_secret_bytes(&mut keyfile);
    }
    result
}

/// Lock-free persist with an externally supplied master key (used by
/// `change_master_key`). The caller owns the new password/keyfile.
pub(crate) fn persist_change(
    db: &Database,
    password: &str,
    keyfile: Option<&[u8]>,
    target: &SaveTarget,
) -> Result<(), String> {
    let key = build_database_key(password, keyfile)?;
    persist_snapshot(db, &key, target)
}

/// Write attachment bytes extracted under the lock (file I/O outside it).
pub(crate) fn write_attachment_file(data: &[u8], dest: &str) -> Result<(), String> {
    std::fs::write(dest, data).map_err(|e| format!("写入附件失败: {e}"))
}

/// Write CSV content built under the lock (file I/O outside it).
pub(crate) fn write_csv_file(path: &str, content: &str) -> Result<(), String> {
    std::fs::write(path, content).map_err(|e| format!("写入文件失败: {e}"))
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
        let (db, keyfile_bytes) = prepare_local_open(path, password, keyfile)?;
        self.adopt_local(db, path, password, keyfile_bytes)
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
        let (db, keyfile_bytes) =
            prepare_local_create(path, password, kdf, cipher, compression, keyfile)?;
        self.adopt_local(db, path, password, keyfile_bytes)
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
        let (db, keyfile_bytes, key) = prepare_remote_open(
            storage.clone(),
            key,
            password,
            keyfile,
            mode,
            local_dir,
            backup_count,
        )?;
        self.adopt_remote(
            db,
            storage,
            &key,
            password,
            keyfile_bytes,
            mode,
            local_dir,
            backup_count,
        )
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
        let (db, keyfile_bytes, key) = prepare_remote_create(
            storage.clone(),
            key,
            password,
            kdf,
            cipher,
            compression,
            keyfile,
            mode,
            local_dir,
            backup_count,
        )?;
        self.adopt_remote(
            db,
            storage,
            &key,
            password,
            keyfile_bytes,
            mode,
            local_dir,
            backup_count,
        )
    }

    pub fn close(&mut self) {
        self.path = None;
        // Wipe secret material before dropping the buffers: setting `None`
        // alone leaves the master password and keyfile contents on the heap.
        if let Some(mut password) = self.password.take() {
            wipe_secret_string(&mut password);
        }
        if let Some(mut keyfile) = self.keyfile.take() {
            wipe_secret_bytes(&mut keyfile);
        }
        self.db = None;
        self.dirty = false;
        self.modified_at.clear();
        self.remote = None;
        self.cached_snapshot = None;
    }

    pub fn state(&mut self) -> Result<Option<VaultState>, String> {
        if !self.is_open() {
            return Ok(None);
        }
        Ok(Some(self.snapshot()?))
    }

    pub fn save(&mut self) -> Result<VaultState, String> {
        let job = self.prepare_save()?;
        let revision = job.revision;
        persist_save(job)?;
        self.complete_save(revision)
    }

    /// Re-encrypt and persist the vault with a new master key (password
    /// and/or keyfile). The session continues with the new key.
    pub fn change_master_key(
        &mut self,
        password: &str,
        keyfile: Option<&Path>,
    ) -> Result<VaultState, String> {
        let keyfile_bytes = read_keyfile(keyfile)?;
        let (db, target, revision) = self.prepare_change()?;
        persist_change(&db, password, keyfile_bytes.as_deref(), &target)?;
        self.complete_change(password.to_owned(), keyfile_bytes, revision)
    }

    pub fn add_entry(&mut self, input: &EntryInput) -> Result<VaultState, String> {
        // Decode all attachment payloads before touching the database so a
        // bad payload aborts the whole mutation (no half-applied entry).
        let payloads = decode_attachments(&input.attachments)?;
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
            sync_attachments(&mut entry, &input.attachments, &payloads);
        }
        self.mark_dirty();
        self.snapshot()
    }

    pub fn update_entry(&mut self, uuid: &str, input: &EntryInput) -> Result<VaultState, String> {
        let id = parse_entry_id(uuid)?;
        let target_group = resolve_group_id(self.require_db()?, &input.group_uuid)?;
        // Decode attachment payloads up-front; a decode failure must not
        // leave a half-applied update (fields written, history snapshotted).
        let payloads = decode_attachments(&input.attachments)?;
        {
            let db = self.require_db_mut()?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            if entry.parent_mut().id() != target_group {
                entry
                    .move_to(target_group)
                    .map_err(|e| format!("移动条目失败: {e}"))?;
            }
            {
                // Snapshots the pre-change state into the entry's history on drop.
                let mut tracked = entry.track_changes();
                {
                    let mut current = tracked.as_mut();
                    write_fields(&mut current, input);
                    sync_custom_fields(&mut current, &input.custom_fields);
                    sync_attachments(&mut current, &input.attachments, &payloads);
                }
                tracked.times.last_modification = Some(Times::now());
            }
            trim_entry_history(&mut entry);
        }
        self.mark_dirty();
        self.snapshot()
    }

    /// Move an entry into another group (used by drag-and-drop).
    pub fn move_entry(&mut self, uuid: &str, group_uuid: &str) -> Result<VaultState, String> {
        let id = parse_entry_id(uuid)?;
        let target = resolve_group_id(self.require_db()?, group_uuid)?;
        {
            let db = self.require_db_mut()?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            if entry.parent_mut().id() != target {
                entry
                    .move_to(target)
                    .map_err(|e| format!("移动条目失败: {e}"))?;
            }
        }
        self.mark_dirty();
        self.snapshot()
    }

    /// Move several entries into the recycle bin (or permanently delete them
    /// when they are already inside the recycle bin).
    pub fn delete_entries(&mut self, uuids: &[String]) -> Result<VaultState, String> {
        {
            let db = self.require_db_mut()?;
            let bin_id = ensure_recycle_bin(db)?;
            for uuid in uuids {
                let id = parse_entry_id(uuid)?;
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

    /// List the historical snapshots of an entry, newest first. Passwords are
    /// intentionally excluded from the payload — the renderer restores by
    /// index, and the plaintext must not leave the backend.
    pub fn get_entry_history(&self, uuid: &str) -> Result<Vec<HistoryVersion>, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        let Some(history) = entry.history.as_ref() else {
            return Ok(Vec::new());
        };
        Ok(history
            .get_entries()
            .iter()
            .enumerate()
            .map(|(index, historical)| HistoryVersion {
                index,
                modified: historical.times.last_modification.map(format_iso),
                title: historical.get_title().unwrap_or_default().to_owned(),
                username: historical
                    .get(FIELD_USERNAME)
                    .unwrap_or_default()
                    .to_owned(),
                url: historical.get(FIELD_URL).unwrap_or_default().to_owned(),
                notes: historical.get(FIELD_NOTES).unwrap_or_default().to_owned(),
                expires: historical.times.expiry.map(format_iso),
                custom_fields: {
                    let mut fields: Vec<CustomField> = historical
                        .fields
                        .iter()
                        .filter(|(name, _)| {
                            !name.is_empty() && !RESERVED_FIELDS.contains(&name.as_str())
                        })
                        .map(|(name, value)| CustomField {
                            name: name.clone(),
                            value: value.get().clone(),
                        })
                        .collect();
                    fields.sort_by(|a, b| a.name.cmp(&b.name));
                    fields
                },
            })
            .collect())
    }

    /// Overwrite an entry with a historical snapshot. The current state is
    /// itself pushed into the history first, so the restore can be undone.
    pub fn restore_entry_version(
        &mut self,
        uuid: &str,
        index: usize,
    ) -> Result<VaultState, String> {
        let id = parse_entry_id(uuid)?;
        let version = {
            let db = self.require_db()?;
            let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
            let history = entry
                .history
                .as_ref()
                .ok_or_else(|| "该条目没有历史版本".to_owned())?;
            history
                .get_entries()
                .get(index)
                .ok_or_else(|| "历史版本不存在".to_owned())?
                .clone()
        };
        {
            let db = self.require_db_mut()?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            {
                let mut tracked = entry.track_changes();
                {
                    let mut current = tracked.as_mut();
                    current.fields.clear();
                    for (name, value) in &version.fields {
                        current.fields.insert(name.clone(), value.clone());
                    }
                    current.tags = version.tags.clone();
                    current.times.expiry = version.times.expiry;
                    current.times.expires = version.times.expires;
                    match version.icon() {
                        Some(Icon::BuiltIn(icon_id)) => current.set_icon_builtin(*icon_id),
                        _ => current.set_icon_none(),
                    }
                }
                tracked.times.last_modification = Some(Times::now());
            }
            trim_entry_history(&mut entry);
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
    /// Extract one attachment's bytes under the lock; the caller writes them
    /// outside it (see `write_attachment_file`).
    pub(crate) fn attachment_data(&self, uuid: &str, name: &str) -> Result<Vec<u8>, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        let attachment = entry
            .attachment_by_name(name)
            .ok_or_else(|| "附件不存在".to_owned())?;
        Ok(attachment.data.get().to_vec())
    }

    /// Convenience used by tests and callers that may hold the lock anyway.
    pub fn save_attachment(&self, uuid: &str, name: &str, dest: &str) -> Result<(), String> {
        let data = self.attachment_data(uuid, name)?;
        write_attachment_file(&data, dest)
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

    /// Expand `{REF:...}` field references in an auto-type sequence against
    /// the database. Entries inside the recycle bin are not referenceable.
    pub fn expand_autotype_sequence(&self, sequence: &str) -> Result<String, String> {
        let db = self.require_db()?;
        let bin_id = recycle_bin_id(db);
        autotype::expand_refs(sequence, |spec| {
            let mut found: Option<String> = None;
            walk_ref_match(db.root(), bin_id, spec, &mut found);
            found
        })
        .map_err(|e| e.to_string())
    }

    /// Best-matching entry for global auto-type given the title of the window
    /// in focus. Matches the URL host or the entry title against the window
    /// title (case-insensitive); entries inside the recycle bin are skipped.
    /// Returns the entry UUID.
    pub fn autotype_match(&self, window_title: &str) -> Result<String, String> {
        let db = self.require_db()?;
        let lower = window_title.to_lowercase();
        if lower.trim().is_empty() {
            return Err("目标窗口标题为空".to_owned());
        }
        let bin_id = recycle_bin_id(db);
        let mut best: Option<(i32, String)> = None;
        walk_match(db.root(), bin_id, &lower, &mut best);
        best.map(|(_, uuid)| uuid)
            .ok_or_else(|| "没有找到匹配的条目".to_owned())
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
            match input.icon {
                Some(icon_id) => group.set_icon_builtin(icon_id as usize),
                None => group.set_icon_none(),
            }
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

    /// Locked half of `open`/`create`: adopt a decrypted database prepared
    /// outside the lock. Cheap — no KDF, no file I/O.
    pub(crate) fn adopt_local(
        &mut self,
        db: Database,
        path: &Path,
        password: &str,
        keyfile: Option<Vec<u8>>,
    ) -> Result<VaultState, String> {
        self.replace(db, path, password, keyfile);
        self.snapshot()
    }

    /// Locked half of `open_remote`/`create_remote`: adopt a decrypted
    /// database prepared outside the lock. Cheap — no KDF, no network I/O.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn adopt_remote(
        &mut self,
        db: Database,
        storage: Arc<dyn RemoteStorage>,
        key: &str,
        password: &str,
        keyfile: Option<Vec<u8>>,
        mode: RemoteMode,
        local_dir: &Path,
        backup_count: usize,
    ) -> Result<VaultState, String> {
        self.remote = Some(RemoteTarget {
            storage,
            key: key.to_owned(),
            mode,
            local_dir: local_dir.to_path_buf(),
            backup_count,
        });
        self.path = Some(format!("{REMOTE_URI_PREFIX}{key}"));
        self.password = Some(password.to_owned());
        self.keyfile = keyfile;
        self.db = Some(db);
        self.dirty = false;
        self.modified_at = now_iso();
        self.cached_snapshot = None;
        self.snapshot()
    }

    /// Replace the session with a freshly opened/created local database. Any
    /// remote target from a previous session is dropped: saving a local vault
    /// must never upload to a stale S3 target.
    fn replace(&mut self, db: Database, path: &Path, password: &str, keyfile: Option<Vec<u8>>) {
        self.path = Some(path.to_string_lossy().into_owned());
        self.password = Some(password.to_owned());
        self.keyfile = keyfile;
        self.db = Some(db);
        self.dirty = false;
        self.modified_at = now_iso();
        self.remote = None;
        self.cached_snapshot = None;
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
        self.revision += 1;
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

    fn snapshot(&mut self) -> Result<VaultState, String> {
        // Rebuild the tree only when the database changed; repeated reads
        // (e.g. get_vault_state polling) reuse the cached snapshot.
        if let Some((revision, cached)) = &self.cached_snapshot {
            if *revision == self.revision {
                return Ok(cached.clone());
            }
        }
        let db = self.require_db()?;
        let path = self.require_path()?;
        let state = VaultState {
            path: path.to_owned(),
            file_name: Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(path)
                .to_owned(),
            root: build_group_tree(db),
            dirty: self.dirty,
            modified_at: self.modified_at.clone(),
        };
        self.cached_snapshot = Some((self.revision, state.clone()));
        Ok(state)
    }

    /// Capture everything `persist_save` needs. Cheap (no KDF, no I/O): a
    /// database clone plus the target info, so the slow work can run outside
    /// the session lock.
    pub(crate) fn prepare_save(&self) -> Result<SaveJob, String> {
        let (db, target, revision) = self.prepare_change()?;
        Ok(SaveJob {
            db,
            password: self.require_password()?.to_owned(),
            keyfile: self.keyfile.clone(),
            target,
            revision,
        })
    }

    /// Cheap snapshot of the database and save target, used when the caller
    /// supplies a fresh master key (`change_master_key`). Returns the current
    /// edit revision so the completion step can detect concurrent edits.
    pub(crate) fn prepare_change(&self) -> Result<(Database, SaveTarget, u64), String> {
        let db = self.require_db()?.clone();
        let target = match &self.remote {
            Some(remote) => SaveTarget::Remote {
                storage: remote.storage.clone(),
                key: remote.key.clone(),
                mode: remote.mode,
                local_dir: remote.local_dir.clone(),
                backup_count: remote.backup_count,
            },
            None => SaveTarget::Local(PathBuf::from(self.require_path()?.to_owned())),
        };
        Ok((db, target, self.revision))
    }

    /// Locked completion of `save`: mark the session clean (unless edits
    /// landed while the save ran) and re-snapshot.
    pub(crate) fn complete_save(&mut self, revision: u64) -> Result<VaultState, String> {
        if self.revision == revision {
            self.dirty = false;
            self.modified_at = now_iso();
            self.cached_snapshot = None;
        }
        self.snapshot()
    }

    /// Locked completion of `change_master_key`: adopt the new credentials
    /// and re-snapshot. Only called after the persist succeeded.
    pub(crate) fn complete_change(
        &mut self,
        password: String,
        keyfile: Option<Vec<u8>>,
        revision: u64,
    ) -> Result<VaultState, String> {
        self.password = Some(password);
        self.keyfile = keyfile;
        if self.revision == revision {
            self.dirty = false;
            self.modified_at = now_iso();
            self.cached_snapshot = None;
        }
        self.snapshot()
    }

    /// Fetch a single entry's password on demand (never part of `VaultState`).
    pub fn get_entry_password(&self, uuid: &str) -> Result<String, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        Ok(entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned())
    }

    /// Fetch a single entry's TOTP seed on demand (never part of `VaultState`).
    /// `None` means the entry has no seed configured.
    pub fn get_entry_totp(&self, uuid: &str) -> Result<Option<String>, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
        Ok(entry.get_raw_otp_value().map(str::to_owned))
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
        let content = self.export_csv_content()?;
        write_csv_file(path, &content)
    }

    /// Build the CSV payload under the lock; the caller writes it outside
    /// the lock (see `write_csv_file`).
    pub(crate) fn export_csv_content(&self) -> Result<String, String> {
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

        Ok(format!("\u{FEFF}{}\r\n", lines.join("\r\n")))
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
        icon: match group.icon() {
            Some(Icon::BuiltIn(id)) => Some(*id as u32),
            _ => None,
        },
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
        has_totp: entry.get_raw_otp_value().is_some(),
        icon: match entry.icon() {
            Some(Icon::BuiltIn(id)) => Some(*id as u32),
            _ => None,
        },
        created: entry.times.creation.map(format_iso),
        modified: entry.times.last_modification.map(format_iso),
        tags: if entry.tags.is_empty() {
            None
        } else {
            Some(entry.tags.join(", "))
        },
        favorite: entry.get(FIELD_FAVORITE) == Some(FIELD_FAVORITE_TRUE),
        color: entry.background_color.as_ref().map(ToString::to_string),
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

/// Maximum number of historical snapshots kept per entry (KeePass default).
const MAX_HISTORY_VERSIONS: usize = 10;

/// Drop the oldest snapshots until the history fits within the cap. The
/// crate exposes no mutable access to the history, so it is rebuilt with the
/// newest `MAX_HISTORY_VERSIONS` snapshots preserved in their original order.
fn trim_entry_history(entry: &mut Entry) {
    if let Some(history) = entry.history.as_mut() {
        let current = history.get_entries();
        if current.len() <= MAX_HISTORY_VERSIONS {
            return;
        }
        let kept: Vec<Entry> = current.iter().take(MAX_HISTORY_VERSIONS).cloned().collect();
        let mut trimmed = History::default();
        for snapshot in kept.into_iter().rev() {
            trimmed.add_entry(snapshot);
        }
        entry.history = Some(trimmed);
    }
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
    // Icon: a built-in KeePass index; absent = default icon.
    match input.icon {
        Some(icon_id) => entry.set_icon_builtin(icon_id as usize),
        None => entry.set_icon_none(),
    }
    // Background color tags the entry row; foreground is left unset.
    entry.background_color = parse_color(input.color.as_deref());
    entry.foreground_color = None;
}

/// Parse a `#RRGGBB` color string; `None` for empty/absent or invalid input.
fn parse_color(value: Option<&str>) -> Option<Color> {
    value?.trim().parse().ok()
}

/// Parse an ISO-8601 expiry string into a UTC `NaiveDateTime`. Accepts the
/// frontend's `toISOString()` output (with milliseconds and `Z` suffix) as
/// well as legacy `%Y-%m-%dT%H:%M:%S` values. Returns `None` for empty input;
/// rejects invalid formats.
fn parse_expiry(value: Option<&str>) -> Option<NaiveDateTime> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(raw) {
        return Some(dt.naive_utc());
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

/// A pre-decoded attachment payload, ready to write.
struct AttachmentPayload {
    name: String,
    data: Vec<u8>,
}

/// Decode all attachment payloads up-front so a bad base64 payload aborts the
/// whole entry mutation before anything is written (no partial commit, no
/// history snapshot pollution, dirty flag stays untouched).
fn decode_attachments(input: &[AttachmentInput]) -> Result<Vec<AttachmentPayload>, String> {
    let mut payloads = Vec::new();
    for attachment in input {
        if attachment.name.trim().is_empty() {
            continue;
        }
        if let Some(data) = &attachment.data {
            let bytes = BASE64
                .decode(data.trim())
                .map_err(|e| format!("附件数据解码失败: {e}"))?;
            payloads.push(AttachmentPayload {
                name: attachment.name.clone(),
                data: bytes,
            });
        }
    }
    Ok(payloads)
}

/// Make the entry's attachment set match the given list. Names that no longer
/// appear are removed; entries carrying payloads are added or replaced. All
/// payloads are already decoded (see [`decode_attachments`]).
fn sync_attachments(
    entry: &mut EntryMut<'_>,
    attachments: &[AttachmentInput],
    payloads: &[AttachmentPayload],
) {
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
    for payload in payloads {
        entry.add_attachment(payload.name.clone(), Value::protected(payload.data.clone()));
    }
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

/// Lower-cased host part of a URL, without scheme, port, or path.
fn url_host(url: &str) -> String {
    let rest = url.split("://").nth(1).unwrap_or(url);
    rest.split(['/', ':', '?', '#'])
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase()
}

/// Depth-first scan of `group`'s subtree for a `{REF:...}` match. First
/// matching entry wins (KeePass semantics); the recycle bin is skipped.
fn walk_ref_match(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    spec: autotype::RefSpec<'_>,
    out: &mut Option<String>,
) {
    if bin_id == Some(group.id()) {
        return;
    }
    let needle = spec.text.to_lowercase();
    for entry in group.entries() {
        let matched = match spec.search.to_ascii_uppercase().as_str() {
            "T" => entry
                .get_title()
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle),
            "U" => entry
                .get(FIELD_USERNAME)
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle),
            "P" => entry
                .get(FIELD_PASSWORD)
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle),
            "A" => entry
                .get(FIELD_URL)
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle),
            "N" => entry
                .get(FIELD_NOTES)
                .unwrap_or_default()
                .to_lowercase()
                .contains(&needle),
            "I" => entry
                .id()
                .uuid()
                .to_string()
                .replace('-', "")
                .to_lowercase()
                .contains(&needle.replace('-', "")),
            "O" => entry.fields.keys().any(|name| {
                !RESERVED_FIELDS.contains(&name.as_str())
                    && name.to_lowercase() == spec.text.to_lowercase()
            }),
            _ => false,
        };
        if matched {
            let value = match spec.field.to_ascii_uppercase().as_str() {
                "T" => entry.get_title().unwrap_or_default().to_owned(),
                "U" => entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                "P" => entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
                "A" => entry.get(FIELD_URL).unwrap_or_default().to_owned(),
                "N" => entry.get(FIELD_NOTES).unwrap_or_default().to_owned(),
                "I" => entry.id().uuid().to_string(),
                _ => return,
            };
            *out = Some(value);
            return;
        }
    }
    for child in group.groups() {
        walk_ref_match(child, bin_id, spec, out);
    }
}

/// Depth-first scan of `group`'s subtree for an auto-type match; the recycle
/// bin subtree is skipped entirely.
fn walk_match(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    window_title: &str,
    best: &mut Option<(i32, String)>,
) {
    if bin_id == Some(group.id()) {
        return;
    }
    for entry in group.entries() {
        let mut score = 0;
        let host = url_host(entry.get(FIELD_URL).unwrap_or_default());
        if !host.is_empty() && window_title.contains(&host) {
            score += 2;
        }
        let title = entry.get_title().unwrap_or_default().to_lowercase();
        if !title.is_empty() && window_title.contains(&title) {
            score += 1;
        }
        if score > 0 && best.as_ref().is_none_or(|(s, _)| score > *s) {
            *best = Some((score, entry.id().uuid().to_string()));
        }
    }
    for child in group.groups() {
        walk_match(child, bin_id, window_title, best);
    }
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
        let (mut session, path) = create_session(&dir);
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
                icon: None,
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
                icon: None,
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let group = &state.root.children[0];
        assert_eq!(group.entries.len(), 1);
        let entry = &group.entries[0];
        assert_eq!(entry.title, "GitHub");
        assert_eq!(session.get_entry_password(&entry.uuid).unwrap(), "s3cret");
        assert!(entry.has_totp);
        assert_eq!(
            session.get_entry_totp(&entry.uuid).unwrap().as_deref(),
            Some("JBSWY3DPEHPK3PXP")
        );
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
                    icon: None,
                    color: None,
                    custom_fields: vec![],
                    attachments: vec![],
                },
            )
            .unwrap();
        let entry = &state.root.children[0].entries[0];
        assert_eq!(entry.title, "GitHub (work)");
        assert_eq!(session.get_entry_password(&entry_uuid).unwrap(), "s3cret2");
        assert!(!entry.has_totp);
        assert_eq!(session.get_entry_totp(&entry_uuid).unwrap(), None);

        let state = session.rename_group(&group.uuid, "Accounts").unwrap();
        assert_eq!(state.root.children[0].name, "Accounts");

        let state = session.delete_entry(&entry_uuid).unwrap();
        assert_eq!(state.root.children[0].entries.len(), 0);
    }

    #[test]
    fn entry_history_tracks_versions_and_restores() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "A".into(),
                username: "u".into(),
                password: "p1".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: None,
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.entries[0].uuid.clone();

        let input = |title: &str, password: &str| EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: title.into(),
            username: "u".into(),
            password: password.into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: None,
            color: None,
            custom_fields: vec![],
            attachments: vec![],
        };

        assert!(session.get_entry_history(&uuid).unwrap().is_empty());
        session.update_entry(&uuid, &input("B", "p2")).unwrap();
        let history = session.get_entry_history(&uuid).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].title, "A");
        // Passwords never leave the backend in history payloads.
        assert_eq!(session.get_entry_password(&uuid).unwrap(), "p2");

        session.update_entry(&uuid, &input("C", "p3")).unwrap();
        let history = session.get_entry_history(&uuid).unwrap();
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].title, "B");

        // Restoring the snapshot replaces fields and pushes the pre-restore
        // state into the history itself.
        let state = session.restore_entry_version(&uuid, 0).unwrap();
        assert_eq!(state.root.entries[0].title, "B");
        assert_eq!(session.get_entry_password(&uuid).unwrap(), "p2");
        let history = session.get_entry_history(&uuid).unwrap();
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].title, "C");

        assert!(session
            .restore_entry_version(&uuid, 99)
            .is_err_and(|err| err.contains("历史版本不存在")));
    }

    #[test]
    fn entry_history_caps_at_ten_versions() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "v0".into(),
                username: "".into(),
                password: "p0".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: None,
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.entries[0].uuid.clone();
        for i in 1..=14 {
            session
                .update_entry(
                    &uuid,
                    &EntryInput {
                        group_uuid: ROOT_GROUP_UUID.to_owned(),
                        title: format!("v{i}"),
                        username: "".into(),
                        password: format!("p{i}"),
                        url: "".into(),
                        notes: "".into(),
                        totp: None,
                        expires: None,
                        icon: None,
                        color: None,
                        custom_fields: vec![],
                        attachments: vec![],
                    },
                )
                .unwrap();
        }
        let history = session.get_entry_history(&uuid).unwrap();
        assert_eq!(history.len(), 10);
        assert_eq!(history[0].title, "v13");
        assert_eq!(history[9].title, "v4");
    }

    #[test]
    fn entry_icon_and_color_round_trip_and_clear() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Iconic".into(),
                username: "".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(1),
                color: Some("#FF8800".into()),
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let entry = &state.root.entries[0];
        assert_eq!(entry.icon, Some(1));
        assert_eq!(entry.color.as_deref(), Some("#FF8800"));

        // Clearing icon/color reverts to defaults.
        let state = session
            .update_entry(
                &entry.uuid,
                &EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: "Iconic".into(),
                    username: "".into(),
                    password: "pw".into(),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    expires: None,
                    icon: None,
                    color: None,
                    custom_fields: vec![],
                    attachments: vec![],
                },
            )
            .unwrap();
        assert_eq!(state.root.entries[0].icon, None);
        assert_eq!(state.root.entries[0].color, None);

        // Icon survives a save/reopen round trip.
        let state = session
            .update_entry(
                &entry.uuid,
                &EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: "Iconic".into(),
                    username: "".into(),
                    password: "pw".into(),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    expires: None,
                    icon: Some(3),
                    color: Some("#2288FF".into()),
                    custom_fields: vec![],
                    attachments: vec![],
                },
            )
            .unwrap();
        assert_eq!(state.root.entries[0].icon, Some(3));
        session.save().unwrap();
        drop(session);
        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password", None).unwrap();
        assert_eq!(state.root.entries[0].icon, Some(3));
        assert_eq!(state.root.entries[0].color.as_deref(), Some("#2288FF"));
    }

    #[test]
    fn group_icon_round_trip() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "Mail".into(),
                icon: Some(4),
            })
            .unwrap();
        assert_eq!(state.root.children[0].icon, Some(4));
    }

    #[test]
    fn move_entry_between_groups() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "A".into(),
                icon: None,
            })
            .unwrap();
        let group_a = state.root.children[0].uuid.clone();
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                name: "B".into(),
                icon: None,
            })
            .unwrap();
        let group_b = state.root.children[1].uuid.clone();
        let state = session
            .add_entry(&EntryInput {
                group_uuid: group_a.clone(),
                title: "E".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: None,
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let entry_uuid = state.root.children[0].entries[0].uuid.clone();

        let state = session.move_entry(&entry_uuid, &group_b).unwrap();
        assert_eq!(state.root.children[0].entries.len(), 0);
        assert_eq!(state.root.children[1].entries.len(), 1);
        assert_eq!(state.root.children[1].entries[0].uuid, entry_uuid);
        assert_eq!(state.root.children[1].entries[0].group_uuid, group_b);

        // Moving into the same group is a no-op.
        let state = session.move_entry(&entry_uuid, &group_b).unwrap();
        assert_eq!(state.root.children[1].entries.len(), 1);
    }

    #[test]
    fn delete_entries_moves_all_to_recycle_bin() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let mut uuids = Vec::new();
        for i in 0..3 {
            let state = session
                .add_entry(&EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: format!("E{i}"),
                    username: "".into(),
                    password: "pw".into(),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    expires: None,
                    icon: None,
                    color: None,
                    custom_fields: vec![],
                    attachments: vec![],
                })
                .unwrap();
            uuids.push(state.root.entries.last().unwrap().uuid.clone());
        }
        let state = session.delete_entries(&uuids).unwrap();
        assert!(state.root.entries.is_empty());
        assert_eq!(state.root.children[0].entries.len(), 3);
        assert!(state.root.children[0].is_recycle_bin);

        // Second pass permanently deletes the recycled entries.
        let state = session.delete_entries(&uuids).unwrap();
        assert_eq!(state.root.children[0].entries.len(), 0);
    }

    #[test]
    fn save_clears_dirty_and_persists() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
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
    fn snapshot_cache_serves_unchanged_state_without_rebuild() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Mail".into(),
            })
            .unwrap();
        // Repeated reads must be consistent even with the cache: same tree,
        // same dirty flag, and edits invalidate the cache.
        let first = session.state().unwrap().unwrap();
        assert_eq!(first.root.children.len(), 1);
        let second = session.state().unwrap().unwrap();
        assert_eq!(second.root.children.len(), 1);
        assert_eq!(second.root.children[0].name, "Mail");
        assert_eq!(second.dirty, first.dirty);

        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Work".into(),
            })
            .unwrap();
        let third = session.state().unwrap().unwrap();
        assert_eq!(third.root.children.len(), 2);
        assert!(third.dirty, "edit must be reflected and keep dirty=true");
    }

    #[test]
    fn concurrent_edit_during_save_keeps_dirty_flag() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Mail".into(),
            })
            .unwrap();
        // An edit lands between the save's prepare (locked) and completion
        // (locked again): the completion must not clear the new dirty state.
        let job = session.prepare_save().unwrap();
        let revision = job.revision;
        persist_save(job).unwrap();
        session.mark_dirty();
        let state = session.complete_save(revision).unwrap();
        assert!(state.dirty, "edit during save must stay dirty");
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
                icon: None,
                color: None,
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
                    icon: None,
                    color: None,
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
                    icon: None,
                    color: None,
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
                icon: None,
                color: None,
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
                icon: None,
                color: None,
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
                icon: None,
                name: "Parent".into(),
            })
            .unwrap();
        let parent_uuid = state.root.children[0].uuid.clone();

        session
            .add_group(&GroupInput {
                parent_uuid: Some(parent_uuid.clone()),
                icon: None,
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
                icon: None,
                color: None,
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
                icon: None,
                color: None,
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
                icon: None,
                color: None,
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
                icon: None,
                name: "   ".into(),
            })
            .unwrap_err();
        assert!(err.contains("分组名称"));

        let err = session
            .add_group(&GroupInput {
                parent_uuid: Some("not-a-uuid".into()),
                icon: None,
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
                icon: None,
                color: None,
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
                icon: None,
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
                icon: None,
                color: None,
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
        // The TOTP seed must never leave the backend in a snapshot; only the
        // presence flag is serialized.
        assert!(
            entry.get("totp").is_none(),
            "TOTP seed leaked in VaultEntry"
        );
        assert!(entry["hasTotp"].is_boolean());
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
                icon: None,
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
                icon: None,
                color: None,
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
                icon: None,
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
                icon: None,
                color: None,
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

    /// TOTP seeds never serialize into `VaultState` snapshots: the renderer
    /// learns only `hasTotp` and fetches the seed (or a code) on demand.
    #[test]
    fn totp_seed_never_serializes_into_snapshot() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let mut input = EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "2FA".into(),
            username: "u".into(),
            password: "pw".into(),
            url: "".into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: None,
            color: None,
            custom_fields: vec![],
            attachments: vec![],
        };
        input.totp = Some("JBSWY3DPEHPK3PXP".into());
        let state = session.add_entry(&input).unwrap();
        let entry = &state.root.entries[0];
        assert!(entry.has_totp);
        let json = serde_json::to_value(&state).unwrap();
        let serialized = serde_json::to_string(&json["root"]["entries"][0]).unwrap();
        assert!(
            !serialized.contains("JBSWY3DPEHPK3PXP"),
            "TOTP seed leaked into snapshot JSON: {serialized}"
        );
        assert!(serialized.contains("hasTotp"));
    }

    #[test]
    fn toggle_favorite_round_trips_field() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
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
                icon: None,
                color: None,
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
                icon: None,
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
                icon: None,
                color: None,
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
                icon: None,
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
                icon: None,
                color: None,
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
                    icon: None,
                    color: None,
                    custom_fields: vec![CustomField {
                        name: "PIN".into(),
                        value: "9999".into(),
                    }],
                    attachments: vec![
                        AttachmentInput {
                            name: "note.txt".into(),
                            data: None,
                        },
                        AttachmentInput {
                            name: "second.bin".into(),
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
                icon: None,
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
                icon: None,
                color: None,
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
                icon: None,
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
                icon: None,
                color: None,
                custom_fields: vec![],
                attachments: vec![AttachmentInput {
                    name: "blob.bin".into(),
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
                icon: None,
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
                icon: None,
                color: None,
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
                icon: None,
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
                icon: None,
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
                icon: None,
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
                icon: None,
                color: None,
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
                icon: None,
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
                icon: None,
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
    fn opening_local_vault_clears_stale_remote_target() {
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

        let local_path = dir.path().join("local.kdbx");
        session
            .create(&local_path, "pw", "Aes", "Aes256", "None", None)
            .unwrap();
        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "LocalOnly".into(),
            })
            .unwrap();
        session.save().unwrap();

        let remote_bytes = storage.get("vaults/seed.kdbx").unwrap();
        let remote_db =
            Database::parse(&remote_bytes, DatabaseKey::new().with_password("pw")).unwrap();
        assert_eq!(
            remote_db.root().groups().count(),
            0,
            "remote target must not receive local vault data"
        );

        let local_bytes = std::fs::read(&local_path).unwrap();
        let local_db =
            Database::parse(&local_bytes, DatabaseKey::new().with_password("pw")).unwrap();
        assert_eq!(local_db.root().groups().count(), 1);
    }

    #[test]
    fn add_entry_with_invalid_attachment_does_not_partially_commit() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("v.kdbx");
        let mut session = VaultSession::default();
        session
            .create(&path, "pw", "Aes", "Aes256", "None", None)
            .unwrap();

        let err = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Bad".into(),
                username: "u".into(),
                password: "p".into(),
                url: String::new(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: None,
                color: None,
                custom_fields: vec![],
                attachments: vec![AttachmentInput {
                    name: "a.bin".into(),
                    data: Some("!!!not-base64!!!".into()),
                }],
            })
            .unwrap_err();
        assert!(err.contains("附件数据解码失败"));

        let state = session.state().unwrap().unwrap();
        assert!(state.root.entries.is_empty(), "no entry must be committed");
        assert!(!state.dirty, "dirty must not be set after a failed add");
    }

    #[test]
    fn update_entry_with_invalid_attachment_keeps_original_and_history() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("v.kdbx");
        let mut session = VaultSession::default();
        session
            .create(&path, "pw", "Aes", "Aes256", "None", None)
            .unwrap();
        let added = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Original".into(),
                username: "u".into(),
                password: "p".into(),
                url: String::new(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: None,
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = &added.root.entries[0].uuid;

        let bad = EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Rewritten".into(),
            username: "u".into(),
            password: "p".into(),
            url: String::new(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: None,
            color: None,
            custom_fields: vec![],
            attachments: vec![AttachmentInput {
                name: "a.bin".into(),
                data: Some("@@@".into()),
            }],
        };
        let err = session.update_entry(uuid, &bad).unwrap_err();
        assert!(err.contains("附件数据解码失败"));

        let state = session.state().unwrap().unwrap();
        assert_eq!(
            state.root.entries[0].title, "Original",
            "title must be unchanged"
        );
        let history = session.get_entry_history(uuid).unwrap();
        assert!(history.is_empty(), "history must not be polluted");
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
                icon: None,
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

    /// The wipe helpers must zero the *heap* bytes of a secret before it is
    /// dropped, not just replace the logical value with an empty string.
    /// Read the allocation through a raw pointer while it is still alive.
    #[test]
    fn wipe_helpers_zero_heap_bytes() {
        let mut password = String::from("master-password-123");
        let p_ptr = password.as_mut_ptr();
        let p_len = password.len();
        wipe_secret_string(&mut password);
        let p_bytes = unsafe { std::slice::from_raw_parts(p_ptr, p_len) };
        assert!(p_bytes.iter().all(|&b| b == 0));

        let mut keyfile = Vec::from("keyfile-bytes");
        let k_ptr = keyfile.as_mut_ptr();
        let k_len = keyfile.len();
        wipe_secret_bytes(&mut keyfile);
        let k_bytes = unsafe { std::slice::from_raw_parts(k_ptr, k_len) };
        assert!(k_bytes.iter().all(|&b| b == 0));
    }

    /// `close` must clear the master password and keyfile from the session
    /// (the wipe itself is covered by `wipe_helpers_zero_heap_bytes`).
    #[test]
    fn close_clears_stored_secrets() {
        let dir = TempDir::new().unwrap();
        let keyfile = write_keyfile(&dir);
        let path = dir.path().join("test.kdbx");
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
        assert!(session.password.is_some());
        assert!(session.keyfile.is_some());
        session.close();
        assert!(session.password.is_none());
        assert!(session.keyfile.is_none());
        assert!(!session.is_open());
    }

    #[test]
    fn autotype_match_ranks_url_host_above_title_and_skips_recycle_bin() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let entry = |title: &str, url: &str| EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: title.into(),
            username: "u".into(),
            password: "p".into(),
            url: url.into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: None,
            color: None,
            custom_fields: vec![],
            attachments: vec![],
        };
        let state = session
            .add_entry(&entry("GitHub", "https://github.com"))
            .unwrap();
        let github = state.root.entries[0].uuid.clone();
        session
            .add_entry(&entry("GitHub", "https://example.com"))
            .unwrap();
        session
            .add_entry(&entry("Notebook", "https://notes.dev"))
            .unwrap();

        // URL host wins over title when both match.
        assert_eq!(
            session
                .autotype_match("GitHub - Home · github.com")
                .unwrap(),
            github
        );
        // Title-only match still works.
        assert_eq!(
            session.autotype_match("Log in to Notebook").unwrap().len(),
            36
        );
        // No match.
        let err = session.autotype_match("Random app").unwrap_err();
        assert!(err.contains("没有找到匹配"));

        // Trashed entries are never matched.
        let trash = session.delete_entry(&github).unwrap();
        let bin = &trash.root.children[0];
        assert_eq!(bin.name, "回收站");
        session
            .add_entry(&EntryInput {
                group_uuid: bin.uuid.clone(),
                title: "Trashy".into(),
                username: "u".into(),
                password: "p".into(),
                url: "https://github.com".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: None,
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let err = session.autotype_match("Trashy dashboard").unwrap_err();
        assert!(err.contains("没有找到匹配"));
    }

    #[test]
    fn url_host_strips_scheme_port_and_path() {
        assert_eq!(url_host("https://github.com/login"), "github.com");
        assert_eq!(url_host("http://a.b.c:8080/x?y=1"), "a.b.c");
        assert_eq!(url_host("plain-host"), "plain-host");
        assert_eq!(url_host(""), "");
    }

    #[test]
    fn expand_autotype_sequence_resolves_refs_across_entries() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let entry = |title: &str, username: &str, password: &str, url: &str| EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: title.into(),
            username: username.into(),
            password: password.into(),
            url: url.into(),
            notes: "".into(),
            totp: None,
            expires: None,
            icon: None,
            color: None,
            custom_fields: vec![],
            attachments: vec![],
        };
        let state = session
            .add_entry(&entry("Bank", "alice", "secret123", "https://bank.example"))
            .unwrap();
        let bank_uuid = state.root.entries[0].uuid.clone();
        session
            .add_entry(&entry(
                "Mail",
                "mail-bot",
                "mail-pass",
                "https://mail.example",
            ))
            .unwrap();

        // By UUID (case-insensitive, dashes tolerated).
        let expanded = session
            .expand_autotype_sequence(&format!(
                "{{REF:U@I:{bank_uuid}}}{{TAB}}{{REF:P@I:{bank_uuid}}}"
            ))
            .unwrap();
        assert_eq!(expanded, format!("alice{{TAB}}secret123"));
        // By title / URL substring.
        assert_eq!(
            session.expand_autotype_sequence("{REF:P@T:bank}").unwrap(),
            "secret123"
        );
        assert_eq!(
            session
                .expand_autotype_sequence("{REF:U@A:mail.example}")
                .unwrap(),
            "mail-bot"
        );
        // UUID as wanted field.
        assert_eq!(
            session.expand_autotype_sequence("{REF:I@T:Bank}").unwrap(),
            bank_uuid
        );
        // Custom-string name as search (O), standard field as target.
        session
            .update_entry(
                &bank_uuid,
                &EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: "Bank".into(),
                    username: "alice".into(),
                    password: "secret123".into(),
                    url: "https://bank.example".into(),
                    notes: "".into(),
                    totp: None,
                    expires: None,
                    icon: None,
                    color: None,
                    custom_fields: vec![CustomField {
                        name: "Customer Id".into(),
                        value: "CUST-42".into(),
                    }],
                    attachments: vec![],
                },
            )
            .unwrap();
        assert_eq!(
            session
                .expand_autotype_sequence("{REF:U@O:Customer Id}")
                .unwrap(),
            "alice"
        );
        // Unresolvable reference fails with a Chinese message.
        let err = session
            .expand_autotype_sequence("{REF:P@T:missing}")
            .unwrap_err();
        assert!(err.contains("未找到匹配条目"));
    }

    #[test]
    fn expand_autotype_sequence_skips_recycle_bin() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Old".into(),
                username: "u".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: None,
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let old_uuid = state.root.entries[0].uuid.clone();
        session.delete_entry(&old_uuid).unwrap();
        let err = session
            .expand_autotype_sequence(&format!("{{REF:P@I:{old_uuid}}}"))
            .unwrap_err();
        assert!(err.contains("未找到匹配条目"));
    }

    #[test]
    fn parse_expiry_accepts_frontend_iso_with_milliseconds() {
        assert_eq!(
            parse_expiry(Some("2026-08-01T12:34:56.000Z")),
            Some(
                chrono::NaiveDate::from_ymd_opt(2026, 8, 1)
                    .unwrap()
                    .and_hms_opt(12, 34, 56)
                    .unwrap()
            )
        );
        assert_eq!(
            parse_expiry(Some("2099-12-31T23:59:59.500Z")).map(|d| d.and_utc().timestamp_millis()),
            Some(
                chrono::NaiveDate::from_ymd_opt(2099, 12, 31)
                    .unwrap()
                    .and_hms_opt(23, 59, 59)
                    .unwrap()
                    .and_utc()
                    .timestamp_millis()
                    + 500
            )
        );
        assert_eq!(
            parse_expiry(Some("2020-01-01T00:00:00Z")),
            Some(
                chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            )
        );
    }

    #[test]
    fn parse_expiry_accepts_legacy_naive_and_rejects_garbage() {
        assert_eq!(
            parse_expiry(Some("2020-01-01T00:00:00")),
            Some(
                chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
                    .unwrap()
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
            )
        );
        assert_eq!(parse_expiry(Some("")), None);
        assert_eq!(parse_expiry(None), None);
        assert_eq!(parse_expiry(Some("not-a-date")), None);
    }
}
