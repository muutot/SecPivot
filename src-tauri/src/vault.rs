//! Vault session: keep the decrypted `keepass::Database` in memory and expose
//! the IPC-facing commands as testable methods. Serialized shapes mirror
//! `src/lib/types/vault.ts`.

use crate::autotype::{self, AutotypeContext};
use crate::bridge::{BridgeHost, BridgeLogin};
use crate::otp;
use crate::remote::{RemoteStorage, REMOTE_URI_PREFIX};
use crate::remote_backup::{remote_key_basename, validate_remote_key, write_local_copy};
use crate::rpc::{
    merge_urls, write_custom_fields, write_password, write_username, RpcDatabase, RpcError,
    RpcGroup, RpcGroupRef, RpcHost, RpcLogin, RpcLoginWrite,
};
use crate::util::url_host;
use crate::vault_serialize::{
    apply_patch_fields, build_group_tree, collect_favicon_hosts, decode_attachments, escape_csv,
    estimate_entropy, extract_host, format_iso, icon_to_data_url, now_iso, sync_attachments,
    sync_custom_fields, trim_entry_history, write_fields,
};
use keepass::config::{CompressionConfig, KdfConfig, OuterCipherConfig};
use keepass::db::{Entry, EntryId, EntryMut, GroupId, GroupRef, Icon, Times, Value};
use keepass::{Database, DatabaseKey};
use std::collections::HashMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use zeroize::Zeroize;

/// Virtual root group id used by the frontend; maps to the DB root group.
pub const ROOT_GROUP_UUID: &str = "root";
pub const ROOT_GROUP_NAME: &str = "Root";

/// Default backup file name template. `{name}` = file stem, `{timestamp}` =
/// `YYYYMMDDHHmmssSSS`, `{ext}` = original extension. Single source lives in
/// `config`; re-exported here for callers that reference it via `vault::`.
pub(crate) use crate::config::DEFAULT_BACKUP_TEMPLATE;

/// Standard KeePass field names shared with `vault_serialize`.
pub(crate) const FIELD_TITLE: &str = "Title";
pub(crate) const FIELD_USERNAME: &str = "UserName";
pub(crate) const FIELD_PASSWORD: &str = "Password";
pub(crate) const FIELD_URL: &str = "URL";
pub(crate) const FIELD_NOTES: &str = "Notes";
pub(crate) const FIELD_OTP: &str = "otp";
/// KeeOtp-compatible OTP custom-field names, checked in priority order by the
/// OTP resolver (HOTP and Steam have dedicated fields; `otp`/`TimeOtp` are the
/// TOTP forms KeePassXC / KeeWeb understand).
const FIELD_TIME_OTP: &str = "TimeOtp";
const FIELD_HMAC_OTP: &str = "HmacOtp";
const FIELD_STEAM_OTP: &str = "SteamOtp";
const FIELD_STEAM_OTP_ALT: &str = "steam";
/// Custom field used to mark an entry as pinned/favorite.
pub(crate) const FIELD_FAVORITE: &str = "KeyVault.Favorite";
pub(crate) const FIELD_FAVORITE_TRUE: &str = "true";
/// Custom field recording the group an entry lived in before being recycled,
/// so it can be restored to its original location.
const FIELD_ORIGINAL_GROUP: &str = "KeyVault.OriginalGroup";

/// Standard fields that are surfaced through the entry's own columns and must
/// not leak into the generic custom-fields list.
pub(crate) const RESERVED_FIELDS: [&str; 8] = [
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
// Type definitions live in `crate::vault_dto`; re-exported here so the session
// code and `lib.rs` keep referencing them via `vault::*`.

pub use crate::vault_dto::{
    AttachmentInfo, AttachmentInput, CustomField, DuplicatePasswords, EntryInput, EntryPatch,
    FaviconFetch, FaviconJob, FaviconProgress, FaviconReport, GroupInput, HistoryVersion,
    SecurityReport, TotpCode, VaultEntry, VaultGroup, VaultState, WeakEntry,
};

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
    pub backup_template: String,
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
    /// Browser-bridge client keys (KeePassHttp `Id` → AES key). Session-held
    /// only, never persisted: `close()` wipes them so the loopback server
    /// cannot serve credentials while the vault is locked.
    bridge_keys: HashMap<String, Vec<u8>>,
    /// KeePassRPC session keys (client username → 32-byte SRP-derived key).
    /// Same lifecycle as `bridge_keys`: in-memory only, wiped on close.
    rpc_keys: HashMap<String, Vec<u8>>,
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
        backup_template: String,
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
    backup_template: &str,
) -> Result<(Database, Option<Vec<u8>>, String), String> {
    let key = validate_remote_key(key)?;
    let keyfile_bytes = read_keyfile(keyfile)?;
    let db_key = build_database_key(password, keyfile_bytes.as_deref())?;
    let data = storage
        .get(&key)
        .map_err(|e| format!("下载远程文件失败: {e}"))?;
    let db = Database::parse(&data, db_key).map_err(classify_open_error)?;
    if mode == RemoteMode::SaveLocal {
        write_local_copy(
            local_dir,
            &remote_key_basename(&key),
            &data,
            backup_count,
            backup_template,
        )?;
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
    backup_template: &str,
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
        write_local_copy(
            local_dir,
            &remote_key_basename(&key),
            &buffer,
            backup_count,
            backup_template,
        )?;
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
            backup_template,
        } => {
            storage
                .put(key, &buffer)
                .map_err(|e| format!("上传远程文件失败: {e}"))?;
            if *mode == RemoteMode::SaveLocal {
                write_local_copy(
                    local_dir,
                    &remote_key_basename(key),
                    &buffer,
                    *backup_count,
                    backup_template,
                )
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
        backup_template: &str,
    ) -> Result<VaultState, String> {
        let (db, keyfile_bytes, key) = prepare_remote_open(
            storage.clone(),
            key,
            password,
            keyfile,
            mode,
            local_dir,
            backup_count,
            backup_template,
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
            backup_template,
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
        backup_template: &str,
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
            backup_template,
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
            backup_template,
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
        for (_, mut key) in self.bridge_keys.drain() {
            wipe_secret_bytes(&mut key);
        }
        for (_, mut key) in self.rpc_keys.drain() {
            wipe_secret_bytes(&mut key);
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

    /// Save As: persist the current database (same master key) to a new local
    /// path, then switch the session to that path. On success a remote
    /// session becomes a plain local one — later saves go to the new file,
    /// never back to S3. On failure the session is left untouched.
    pub fn save_as(&mut self, path: &Path) -> Result<VaultState, String> {
        let (db, _, revision) = self.prepare_change()?;
        let password = self.require_password()?.to_owned();
        let keyfile = self.keyfile.clone();
        persist_change(
            &db,
            &password,
            keyfile.as_deref(),
            &SaveTarget::Local(path.to_path_buf()),
        )?;
        self.replace(db, path, &password, keyfile);
        if self.revision == revision {
            self.dirty = false;
            self.modified_at = now_iso();
            self.cached_snapshot = None;
        }
        self.snapshot()
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

    /// Apply one partial patch to several entries in a single transaction.
    /// Only the fields present in the patch are written; the rest of each
    /// entry (including the password) is left untouched. All uuids are
    /// validated up-front so a bad id aborts the whole batch before any
    /// entry is modified; the snapshot is built once afterwards.
    pub fn update_entries(
        &mut self,
        uuids: &[String],
        patch: &EntryPatch,
    ) -> Result<VaultState, String> {
        if uuids.is_empty() {
            return self.snapshot();
        }
        let ids: Vec<EntryId> = uuids
            .iter()
            .map(|uuid| parse_entry_id(uuid))
            .collect::<Result<_, _>>()?;
        {
            let db = self.require_db_mut()?;
            for id in &ids {
                db.entry(*id).ok_or_else(|| "条目不存在".to_owned())?;
            }
            for id in ids {
                let mut entry = db.entry_mut(id).expect("validated entry must exist");
                {
                    let mut tracked = entry.track_changes();
                    {
                        let mut current = tracked.as_mut();
                        apply_patch_fields(&mut current, patch);
                    }
                    tracked.times.last_modification = Some(Times::now());
                }
                trim_entry_history(&mut entry);
            }
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
                expires: match historical.times.expires {
                    Some(true) => historical.times.expiry.map(format_iso),
                    _ => None,
                },
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

    /// Compute the one-time password for an entry that carries an OTP seed
    /// field. Detects the kind from the field name: `otp`/`TimeOtp` = TOTP,
    /// `HmacOtp` = HOTP, `SteamOtp`/`steam` = Steam Guard. HOTP advances its
    /// counter on every request and rewrites the seed field server-side (no
    /// history snapshot), so the next code uses the new counter.
    pub fn totp_code(&mut self, uuid: &str) -> Result<TotpCode, String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("读取系统时间失败: {e}"))?
            .as_secs();
        let id = parse_entry_id(uuid)?;
        let (is_hotp, spec) = {
            let db = self.require_db()?;
            let entry = db.entry(id).ok_or_else(|| "条目不存在".to_owned())?;
            let spec = parse_entry_otp_spec(&entry)?;
            (spec.kind == otp::OtpKind::Hotp, spec)
        };
        let code = otp::compute(&spec, now)?;
        if is_hotp {
            self.advance_hotp_counter(id, &spec)?;
        }
        Ok(TotpCode {
            code: code.code,
            kind: otp_kind_name(spec.kind).to_owned(),
            valid_for: code.valid_for,
            period: code.period,
            counter: code.counter,
        })
    }

    /// Advance an `HmacOtp` entry's counter by rewriting the seed field with
    /// `counter+1`. Mutates without `track_changes` so showing a code does not
    /// pollute the entry's history; the vault is left dirty so the next save
    /// persists the new counter.
    fn advance_hotp_counter(&mut self, id: EntryId, spec: &otp::OtpSpec) -> Result<(), String> {
        let next = {
            let mut next = spec.clone();
            next.counter = spec.counter + 1;
            otp::render_hotp_seed(&next)
        };
        {
            let db = self.require_db_mut()?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            entry.set(FIELD_HMAC_OTP, Value::unprotected(next));
        }
        self.mark_dirty();
        Ok(())
    }

    /// Distinct URL hosts referenced by entry URLs, with the entries per host
    /// (KeePass "Download Favicons" job list). Non-http(s) URLs are skipped.
    pub fn favicon_jobs(&self) -> Result<Vec<FaviconJob>, String> {
        let db = self.require_db()?;
        let mut map: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        collect_favicon_hosts(&db.root(), &mut map);
        Ok(map
            .into_iter()
            .map(|(host, entry_uuids)| FaviconJob { host, entry_uuids })
            .collect())
    }

    /// Favicon jobs restricted to the given entry UUIDs (multi-select download);
    /// only those entries get icons, never their same-host siblings. Entries
    /// without a parseable http(s) URL are skipped; unknown uuids are ignored.
    pub fn favicon_jobs_selected(&self, uuids: &[String]) -> Result<Vec<FaviconJob>, String> {
        let db = self.require_db()?;
        let mut map: std::collections::BTreeMap<String, Vec<String>> = Default::default();
        for uuid in uuids {
            let id = parse_entry_id(uuid)?;
            let Some(entry) = db.entry(id) else {
                continue;
            };
            if let Some(host) = extract_host(entry.get(FIELD_URL).unwrap_or_default()) {
                map.entry(host)
                    .or_default()
                    .push(entry.id().uuid().to_string());
            }
        }
        Ok(map
            .into_iter()
            .map(|(host, entry_uuids)| FaviconJob { host, entry_uuids })
            .collect())
    }

    /// Store fetched favicon bytes as database custom icons and point every
    /// entry of the same host at that icon. An entry that already references
    /// an identical icon keeps it; otherwise the icon data is replaced (or a
    /// new custom icon is created). Persisting is the caller's job.
    pub fn apply_favicons(
        &mut self,
        jobs: &[FaviconJob],
        fetched: Vec<FaviconFetch>,
    ) -> Result<(), String> {
        let db = self.require_db_mut()?;
        let jobs: HashMap<&str, &FaviconJob> = jobs.iter().map(|j| (j.host.as_str(), j)).collect();
        for item in fetched {
            let Some(job) = jobs.get(item.host.as_str()) else {
                continue;
            };
            let Some(first) = job.entry_uuids.first() else {
                continue;
            };
            let first_id = parse_entry_id(first)?;
            let existing = {
                let Some(first_entry) = db.entry_mut(first_id) else {
                    continue;
                };
                first_entry.icon().cloned()
            };
            let icon_id = match existing {
                Some(Icon::Custom(id)) => {
                    let identical = db
                        .custom_icon(id)
                        .is_some_and(|icon| icon.data == item.bytes);
                    if !identical {
                        if let Some(mut icon) = db.custom_icon_mut(id) {
                            icon.data = item.bytes.clone();
                        }
                    }
                    id
                }
                _ => {
                    let Some(mut first_entry) = db.entry_mut(first_id) else {
                        continue;
                    };
                    first_entry.set_icon_custom_new(item.bytes.clone()).id()
                }
            };
            for uuid in job.entry_uuids.iter().skip(1) {
                let Some(mut entry) = db.entry_mut(parse_entry_id(uuid)?) else {
                    continue;
                };
                let _ = entry.set_icon_custom(icon_id);
            }
        }
        Ok(())
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
        backup_template: &str,
    ) -> Result<VaultState, String> {
        self.remote = Some(RemoteTarget {
            storage,
            key: key.to_owned(),
            mode,
            local_dir: local_dir.to_path_buf(),
            backup_count,
            backup_template: backup_template.to_owned(),
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
            custom_icons: db
                .iter_all_custom_icons()
                .map(|icon| (icon.id().uuid().to_string(), icon_to_data_url(&icon.data)))
                .collect(),
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
                backup_template: remote.backup_template.clone(),
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
// Browser bridge (KeePassHttp) host
// ---------------------------------------------------------------------------

impl BridgeHost for VaultSession {
    fn is_open(&self) -> bool {
        self.is_open()
    }

    fn client_key(&self, id: &str) -> Option<Vec<u8>> {
        self.bridge_keys.get(id).cloned()
    }

    fn register_client(&mut self, id: &str, key: Vec<u8>) {
        self.bridge_keys.insert(id.to_owned(), key);
    }

    fn list_clients(&self) -> Vec<String> {
        self.bridge_keys.keys().cloned().collect()
    }

    fn remove_client(&mut self, id: &str) -> bool {
        self.bridge_keys.remove(id).is_some()
    }

    fn logins_for(&self, url: &str, submit_url: Option<&str>) -> Vec<BridgeLogin> {
        let Ok(db) = self.require_db() else {
            return Vec::new();
        };
        let bin_id = recycle_bin_id(db);
        let mut out = Vec::new();
        collect_bridge_logins(db.root(), bin_id, url, submit_url, &mut out);
        out
    }

    fn db_hash(&self) -> String {
        let Ok(db) = self.require_db() else {
            return String::new();
        };
        bridge_db_hash(db)
    }

    fn set_login(
        &mut self,
        login: &str,
        password: &str,
        url: &str,
        uuid: Option<&str>,
    ) -> Result<(), String> {
        let uuid = uuid.unwrap_or_default();
        {
            let db = self.require_db_mut()?;
            let id = parse_entry_id(uuid)?;
            let mut entry = db.entry_mut(id).ok_or_else(|| "条目不存在".to_owned())?;
            entry.set(FIELD_USERNAME, Value::unprotected(login.to_owned()));
            entry.set(FIELD_PASSWORD, Value::protected(password.to_owned()));
            entry.set(FIELD_URL, Value::unprotected(url.to_owned()));
        }
        self.mark_dirty();
        Ok(())
    }

    fn create_login(&mut self, login: &str, password: &str, url: &str) -> Result<(), String> {
        let title = bridge_entry_title(url);
        {
            let db = self.require_db_mut()?;
            let mut root = db.root_mut();
            let mut entry = root.add_entry();
            entry.set(FIELD_TITLE, Value::unprotected(title));
            entry.set(FIELD_USERNAME, Value::unprotected(login.to_owned()));
            entry.set(FIELD_PASSWORD, Value::protected(password.to_owned()));
            entry.set(FIELD_URL, Value::unprotected(url.to_owned()));
            entry.set_icon_none();
        }
        self.mark_dirty();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// KeePassRPC host
// ---------------------------------------------------------------------------

impl RpcHost for VaultSession {
    fn is_open(&self) -> bool {
        self.is_open()
    }

    /// The 32-byte SRP-derived session key for a Kee client username. Held
    /// only in memory; wiped by `close()` along with the master key.
    fn rpc_key(&self, username: &str) -> Option<Vec<u8>> {
        self.rpc_keys.get(username).cloned()
    }

    fn register_rpc_key(&mut self, username: &str, key: Vec<u8>) {
        self.rpc_keys.insert(username.to_owned(), key);
    }

    fn database(&self) -> Option<RpcDatabase> {
        let db = self.require_db().ok()?;
        let bin_id = recycle_bin_id(db);
        let file_name = self
            .path
            .as_deref()
            .map(|p| p.rsplit(['/', '\\']).next().unwrap_or(p))
            .unwrap_or_default()
            .to_owned();
        Some(RpcDatabase {
            name: db
                .meta
                .database_name
                .clone()
                .unwrap_or_else(|| file_name.clone()),
            file_name,
            icon_image_data: String::new(),
            root: build_rpc_group(db.root(), bin_id, ROOT_GROUP_NAME, ""),
            active: true,
        })
    }

    fn find_logins(
        &self,
        urls: &[String],
        uuid: Option<&str>,
        free_text: Option<&str>,
        username: Option<&str>,
    ) -> Vec<RpcLogin> {
        let Ok(db) = self.require_db() else {
            return Vec::new();
        };
        let bin_id = recycle_bin_id(db);
        let filter = RpcLoginFilter {
            urls,
            uuid,
            free_text,
            username,
        };
        let mut out = Vec::new();
        collect_rpc_logins(db.root(), bin_id, &filter, ROOT_GROUP_NAME, "", &mut out);
        out
    }

    fn add_login(
        &mut self,
        login: &RpcLoginWrite,
        parent_uuid: &str,
    ) -> Result<RpcLogin, RpcError> {
        if !self.is_open() {
            return Err(RpcError::Locked);
        }
        // Resolve the parent group up front (immutable pass): unknown or
        // invalid uuids fall back to the root group, mirroring the plugin's
        // `AddLogin`; a parent inside the recycle bin is rejected like the
        // plugin's `GetRootPwGroup`.
        let parent_id = {
            let db = self.require_db().map_err(rpc_write_error)?;
            let bin_id = recycle_bin_id(db);
            Uuid::parse_str(parent_uuid)
                .ok()
                .map(GroupId::from_uuid)
                .filter(|id| find_rpc_group_id(db.root(), *id, bin_id))
        };
        let created_uuid = {
            let db = self.require_db_mut().map_err(rpc_write_error)?;
            let mut parent_group = match parent_id {
                Some(id) => match db.group_mut(id) {
                    Some(group) => group,
                    None => db.root_mut(),
                },
                None => db.root_mut(),
            };
            let mut entry = parent_group.add_entry();
            apply_login_write(&mut entry, login, &login.urls.join(" "));
            entry.set_icon_none();
            entry.id().uuid().to_string()
        };
        self.mark_dirty();
        // The extension assumes a successful AddLogin/UpdateLogin is durable
        // (KeePassRPC persists after every write); nothing in the desktop UI
        // saves on its behalf, so flush here.
        persist_after_rpc_write(self)?;
        rpc_login_by_uuid(self, &created_uuid)
            .ok_or(RpcError::InvalidMessage("新建条目读取失败".to_owned()))
    }

    fn update_login(
        &mut self,
        login: &RpcLoginWrite,
        old_uuid: &str,
        url_merge_mode: u8,
    ) -> Result<RpcLogin, RpcError> {
        if !self.is_open() {
            return Err(RpcError::Locked);
        }
        let id = parse_entry_id(old_uuid).map_err(|_| RpcError::EntryNotFound)?;
        // Resolve + merge URLs on the immutable snapshot first.
        let merged_urls = {
            let db = self.require_db().map_err(rpc_write_error)?;
            let bin_id = recycle_bin_id(db);
            let current = match find_rpc_entry_urls(db.root(), id, bin_id, false) {
                FindEntryOutcome::NotFound => return Err(RpcError::EntryNotFound),
                FindEntryOutcome::InRecycleBin => return Err(RpcError::InRecycleBin),
                FindEntryOutcome::Found(urls) => urls,
            };
            merge_urls(&current, &login.urls, url_merge_mode)
        };
        {
            let db = self.require_db_mut().map_err(rpc_write_error)?;
            let mut entry = db.entry_mut(id).ok_or(RpcError::EntryNotFound)?;
            // `edit_tracking` snapshots the pre-edit entry into its history on
            // drop — the KDBX equivalent of the plugin's `CreateBackup`.
            entry.edit_tracking(|tracked| {
                let mut this = tracked.as_mut();
                apply_login_write(&mut this, login, &merged_urls.join(" "));
            });
        }
        self.mark_dirty();
        persist_after_rpc_write(self)?;
        rpc_login_by_uuid(self, old_uuid).ok_or(RpcError::EntryNotFound)
    }
}

/// Persist the vault right after a browser-originated write (Add/UpdateLogin)
/// so the change survives a restart even when the desktop UI never saves.
fn persist_after_rpc_write(session: &mut VaultSession) -> Result<(), RpcError> {
    session
        .save()
        .map_err(|e| RpcError::InvalidMessage(format!("保存失败: {e}")))?;
    Ok(())
}

/// Read one entry by uuid as an `RpcLogin` (recycle bin skipped, like the
/// read paths); the plugin returns the updated entry the same way.
fn rpc_login_by_uuid(session: &VaultSession, uuid: &str) -> Option<RpcLogin> {
    let db = session.require_db().ok()?;
    let bin_id = recycle_bin_id(db);
    let filter = RpcLoginFilter {
        urls: &[],
        uuid: Some(uuid),
        free_text: None,
        username: None,
    };
    let mut out = Vec::new();
    collect_rpc_logins(db.root(), bin_id, &filter, ROOT_GROUP_NAME, "", &mut out);
    out.into_iter().next()
}

/// FindLogins filter criteria (mirrors the KeePassRPC parameter list).
struct RpcLoginFilter<'a> {
    urls: &'a [String],
    uuid: Option<&'a str>,
    free_text: Option<&'a str>,
    username: Option<&'a str>,
}

/// Build the full group tree DTO, root included. The recycle bin subtree is
/// excluded so credentials in it stay invisible to browsers.
fn build_rpc_group(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    title: &str,
    parent_path: &str,
) -> RpcGroup {
    let path = if parent_path.is_empty() {
        title.to_owned()
    } else {
        format!("{parent_path}/{title}")
    };
    RpcGroup {
        uuid: group.id().uuid().to_string(),
        title: title.to_owned(),
        path: path.clone(),
        icon_image_data: String::new(),
        entries: group
            .entries()
            .map(|entry| {
                let urls: Vec<String> = entry
                    .get(FIELD_URL)
                    .unwrap_or_default()
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect();
                RpcLogin {
                    uuid: entry.id().uuid().to_string(),
                    title: entry.get_title().unwrap_or_default().to_owned(),
                    username: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                    // childLightEntries carry no credentials; keep them out of
                    // the tree snapshot to avoid secrets entering the browser.
                    password: String::new(),
                    urls,
                    http_realm: String::new(),
                    icon_image_data: String::new(),
                    parent_group: RpcGroupRef {
                        uuid: group.id().uuid().to_string(),
                        title: title.to_owned(),
                        path: path.clone(),
                        icon_image_data: String::new(),
                    },
                    match_accuracy: 1,
                }
            })
            .collect(),
        children: group
            .groups()
            .filter(|g| bin_id != Some(g.id()))
            .map(|g| {
                let name = g.name.clone();
                build_rpc_group(g, bin_id, &name, &path)
            })
            .collect(),
    }
}

/// Depth-first scan for KeePassRPC logins. Matching follows the extension's
/// semantics: any URL-host match, exact uuid, or title/username substring
/// (`freeText`), plus username filter; the recycle bin is skipped.
fn collect_rpc_logins(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    filter: &RpcLoginFilter<'_>,
    group_title: &str,
    parent_path: &str,
    out: &mut Vec<RpcLogin>,
) {
    if bin_id == Some(group.id()) {
        return;
    }
    let group_path = if parent_path.is_empty() {
        group_title.to_owned()
    } else {
        format!("{parent_path}/{group_title}")
    };
    for entry in group.entries() {
        let entry_urls: Vec<String> = entry
            .get(FIELD_URL)
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        let by_url = filter
            .urls
            .iter()
            .any(|u| entry_urls.iter().any(|eu| bridge_host_matches(eu, u)));
        let by_uuid = filter
            .uuid
            .is_some_and(|id| entry.id().uuid().to_string() == id);
        let title = entry.get_title().unwrap_or_default();
        let entry_username = entry.get(FIELD_USERNAME).unwrap_or_default();
        let by_text = filter
            .free_text
            .is_some_and(|t| !t.is_empty() && (title.contains(t) || entry_username.contains(t)));
        let by_username = filter
            .username
            .is_some_and(|u| !u.is_empty() && entry_username.contains(u));
        if by_url || by_uuid || by_text || by_username {
            out.push(RpcLogin {
                uuid: entry.id().uuid().to_string(),
                title: title.to_owned(),
                username: entry_username.to_owned(),
                password: entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
                urls: entry_urls,
                http_realm: String::new(),
                icon_image_data: String::new(),
                parent_group: RpcGroupRef {
                    uuid: group.id().uuid().to_string(),
                    title: group_title.to_owned(),
                    path: group_path.clone(),
                    icon_image_data: String::new(),
                },
                match_accuracy: if by_url { 3 } else { 1 },
            });
        }
    }
    for child in group.groups() {
        let child_title = child.name.clone();
        collect_rpc_logins(child, bin_id, filter, &child_title, &group_path, out);
    }
}

/// Title for entries created by the browser bridge: the URL host, or the raw
/// URL when it has no parseable host.
fn bridge_entry_title(url: &str) -> String {
    let host = url_host(url).unwrap_or_default();
    if host.is_empty() {
        url.trim().to_owned()
    } else {
        host
    }
}

/// Map an internal vault error (e.g. "vault is locked") to a JSON-RPC error.
fn rpc_write_error(err: String) -> RpcError {
    RpcError::InvalidMessage(err)
}

/// True when `id` resolves to a group reachable from `group` without crossing
/// the recycle bin (the bin subtree is skipped, like every read path).
/// References only flow downward, so recursion stays borrow-safe.
fn find_rpc_group_id(group: GroupRef<'_>, id: GroupId, bin_id: Option<GroupId>) -> bool {
    if bin_id == Some(group.id()) {
        return false;
    }
    if group.id() == id {
        return true;
    }
    for child in group.groups() {
        if find_rpc_group_id(child, id, bin_id) {
            return true;
        }
    }
    false
}

/// Outcome of resolving an entry during the write pass.
enum FindEntryOutcome {
    NotFound,
    /// Found, but inside the recycle bin subtree (KeyVault rejects the write).
    InRecycleBin,
    /// Found outside the recycle bin, with its current URL list.
    Found(Vec<String>),
}

/// Resolve an entry by id and read its URL list (space-separated `URL` field).
/// References only flow downward, so recursion stays borrow-safe.
fn find_rpc_entry_urls(
    group: GroupRef<'_>,
    id: EntryId,
    bin_id: Option<GroupId>,
    in_bin: bool,
) -> FindEntryOutcome {
    let in_bin = in_bin || bin_id == Some(group.id());
    if let Some(entry) = group.entry(id) {
        let urls: Vec<String> = entry
            .get(FIELD_URL)
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        if in_bin {
            FindEntryOutcome::InRecycleBin
        } else {
            FindEntryOutcome::Found(urls)
        }
    } else {
        for child in group.groups() {
            match find_rpc_entry_urls(child, id, bin_id, in_bin) {
                FindEntryOutcome::NotFound => {}
                outcome => return outcome,
            }
        }
        FindEntryOutcome::NotFound
    }
}

/// Apply Kee's `Entry` DTO to a destination entry (the plugin's
/// `setPwEntryFromEntry`, adapted to KDBX strings): title and the URL list
/// (space-joined so the read path sees every URL), first password field →
/// Password, all username fields → UserName (last wins), remaining fields →
/// custom strings named `displayName` (fallback `name`).
fn apply_login_write(entry: &mut EntryMut<'_>, login: &RpcLoginWrite, urls: &str) {
    entry.set(FIELD_TITLE, Value::unprotected(login.title.clone()));
    entry.set(FIELD_URL, Value::unprotected(urls.to_owned()));
    entry.set(FIELD_USERNAME, Value::unprotected(write_username(login)));
    entry.set(FIELD_PASSWORD, Value::protected(write_password(login)));
    for (name, value) in write_custom_fields(login) {
        entry.set(name, Value::unprotected(value));
    }
}

/// Depth-first scan for bridge logins matching the request URL (or its
/// submit URL). The recycle bin subtree is skipped entirely.
fn collect_bridge_logins(
    group: GroupRef<'_>,
    bin_id: Option<GroupId>,
    url: &str,
    submit_url: Option<&str>,
    out: &mut Vec<BridgeLogin>,
) {
    if bin_id == Some(group.id()) {
        return;
    }
    let url = url.to_lowercase();
    let submit_url = submit_url.map(str::to_lowercase);
    for entry in group.entries() {
        let entry_url = entry.get(FIELD_URL).unwrap_or_default().to_lowercase();
        let matches = bridge_host_matches(&entry_url, &url)
            || submit_url
                .as_deref()
                .is_some_and(|s| bridge_host_matches(&entry_url, s));
        if matches {
            out.push(BridgeLogin {
                uuid: entry.id().uuid().to_string(),
                name: entry.get_title().unwrap_or_default().to_owned(),
                login: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
                password: entry.get(FIELD_PASSWORD).unwrap_or_default().to_owned(),
            });
        }
    }
    for child in group.groups() {
        collect_bridge_logins(child, bin_id, url.as_str(), submit_url.as_deref(), out);
    }
}

/// Host-level URL match: exact host equality, or one host covering the other
/// as a domain suffix (`example.com` ↔ `www.example.com`). Empty hosts never
/// match, so entries without a URL are invisible to the bridge.
fn bridge_host_matches(entry_url: &str, request_url: &str) -> bool {
    let entry_host = url_host(entry_url).unwrap_or_default();
    let request_host = url_host(request_url).unwrap_or_default();
    if entry_host.is_empty() || request_host.is_empty() {
        return false;
    }
    entry_host == request_host
        || request_host.ends_with(&format!(".{entry_host}"))
        || entry_host.ends_with(&format!(".{request_host}"))
}

/// KeePassHttp database hash: SHA1 of (root uuid bytes + recycle-bin uuid
/// bytes), hex-encoded, as a change signal for browser extensions.
fn bridge_db_hash(db: &Database) -> String {
    use crate::crypto::{hex, sha1_bytes};
    let mut data = Vec::with_capacity(20);
    data.extend_from_slice(db.root().id().uuid().as_bytes());
    if let Some(bin) = db.meta.recyclebin_uuid {
        data.extend_from_slice(bin.as_bytes());
    }
    hex(&sha1_bytes(&data))
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
        let host = url_host(entry.get(FIELD_URL).unwrap_or_default()).unwrap_or_default();
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

/// Find the first non-empty OTP seed field on an entry, in KeeOtp priority
/// order: `HmacOtp` (HOTP), `SteamOtp`/`steam` (Steam Guard), then the TOTP
/// forms `otp`/`TimeOtp`. Returns the field name and its raw value.
fn entry_otp_field(entry: &Entry) -> Option<(&'static str, &str)> {
    const ORDER: [&str; 5] = [
        FIELD_HMAC_OTP,
        FIELD_STEAM_OTP,
        FIELD_STEAM_OTP_ALT,
        FIELD_TIME_OTP,
        FIELD_OTP,
    ];
    for name in ORDER {
        if let Some(value) = entry.get(name) {
            if !value.is_empty() {
                return Some((name, value));
            }
        }
    }
    None
}

/// Whether the entry carries any OTP seed (TOTP, HOTP or Steam). Used by the
/// snapshot builder in `vault_serialize`.
pub(crate) fn entry_has_otp(entry: &Entry) -> bool {
    entry_otp_field(entry).is_some()
}

/// Resolve an entry's OTP seed into a computation spec, picking the parser by
/// the field that actually holds the seed.
fn parse_entry_otp_spec(entry: &Entry) -> Result<otp::OtpSpec, String> {
    let (field, value) = entry_otp_field(entry).ok_or_else(|| "该条目没有 OTP 种子".to_owned())?;
    match field {
        FIELD_HMAC_OTP => otp::parse_hotp_seed(value),
        FIELD_STEAM_OTP | FIELD_STEAM_OTP_ALT => otp::parse_steam_seed(value),
        _ => otp::parse_totp_seed(value),
    }
}

fn otp_kind_name(kind: otp::OtpKind) -> &'static str {
    match kind {
        otp::OtpKind::Totp => "totp",
        otp::OtpKind::Hotp => "hotp",
        otp::OtpKind::Steam => "steam",
    }
}

/// Compute a TOTP code at a specific unix timestamp (deterministic; used by
/// tests). Delegates to the shared OTP primitives in `otp.rs`.
#[cfg(test)]
fn compute_totp_at(seed: &str, unix_time: u64) -> Result<TotpCode, String> {
    let spec = otp::parse_totp_seed(seed)?;
    let code = otp::compute(&spec, unix_time)?;
    Ok(TotpCode {
        code: code.code,
        kind: otp_kind_name(code.kind).to_owned(),
        valid_for: code.valid_for,
        period: code.period,
        counter: code.counter,
    })
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
    crate::util::atomic_write(path, buffer, "数据库")
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
    use crate::vault_serialize::parse_expiry;
    use base64::engine::general_purpose::STANDARD as BASE64;
    use base64::Engine;
    use tempfile::TempDir;

    fn create_session(dir: &TempDir) -> (VaultSession, std::path::PathBuf) {
        let path = dir.path().join("test.kdbx");
        let mut session = VaultSession::default();
        session
            .create(&path, "master-password", "Aes", "Aes256", "None", None)
            .unwrap();
        (session, path)
    }

    /// KeePass "Download Favicons": jobs are grouped by URL host, fetched
    /// bytes land in the database as custom icons on every entry of that
    /// host, and survive a save + reopen round-trip.
    #[test]
    fn apply_favicons_persists_custom_icon_across_reopen() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        for (title, url) in [
            ("Login", "https://example.com/login"),
            ("Other", "https://example.com/other"),
        ] {
            session
                .add_entry(&EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: title.into(),
                    username: "u".into(),
                    password: "p".into(),
                    url: url.into(),
                    notes: String::new(),
                    totp: None,
                    expires: None,
                    icon: Some(None),
                    color: None,
                    custom_fields: Vec::new(),
                    attachments: Vec::new(),
                })
                .unwrap();
        }
        let jobs = session.favicon_jobs().unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].host, "example.com");
        assert_eq!(jobs[0].entry_uuids.len(), 2);

        let bytes = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        session
            .apply_favicons(
                &jobs,
                vec![FaviconFetch {
                    host: "example.com".into(),
                    bytes: bytes.clone(),
                }],
            )
            .unwrap();
        session.save().unwrap();
        drop(session);

        let mut session = VaultSession::default();
        session.open(&path, "master-password", None).unwrap();
        let db = session.require_db().unwrap();
        let mut icon_datas = Vec::new();
        for entry in db.root().entries() {
            match entry.icon().cloned() {
                Some(Icon::Custom(id)) => icon_datas.push(db.custom_icon(id).unwrap().data.clone()),
                _ => panic!("entry should reference a custom icon"),
            }
        }
        assert_eq!(icon_datas, vec![bytes.clone(), bytes.clone()]);
    }

    /// A content-only edit (icon omitted) must keep the entry's icon — both a
    /// built-in icon and a downloaded favicon custom icon — while an explicit
    /// `icon: null` still clears it.
    #[test]
    fn update_without_icon_keeps_existing_icon() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let input = || EntryInput {
            group_uuid: ROOT_GROUP_UUID.to_owned(),
            title: "Login".into(),
            username: "u".into(),
            password: "p".into(),
            url: "https://example.com/login".into(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: None,
            color: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        };
        let state = session.add_entry(&input()).unwrap();
        let uuid = state.root.entries.last().unwrap().uuid.clone();

        // Built-in icon survives a content-only update.
        session
            .update_entry(
                &uuid,
                &EntryInput {
                    icon: Some(Some(5)),
                    ..input()
                },
            )
            .unwrap();
        let state = session.update_entry(&uuid, &input()).unwrap();
        let entry = state.root.entries.last().unwrap();
        assert_eq!(entry.icon, Some(5));
        assert_eq!(entry.custom_icon, None);

        // A downloaded favicon (custom icon) also survives a content-only update.
        let jobs = session.favicon_jobs().unwrap();
        session
            .apply_favicons(
                &jobs,
                vec![FaviconFetch {
                    host: "example.com".into(),
                    bytes: vec![0x89, 0x50, 0x4E, 0x47],
                }],
            )
            .unwrap();
        let state = session.update_entry(&uuid, &input()).unwrap();
        let entry = state.root.entries.last().unwrap();
        assert_eq!(entry.icon, None);
        assert!(entry.custom_icon.is_some(), "custom favicon must be kept");

        // An explicit `icon: null` clears both kinds.
        session
            .update_entry(
                &uuid,
                &EntryInput {
                    icon: Some(None),
                    ..input()
                },
            )
            .unwrap();
        let state = session.update_entry(&uuid, &input()).unwrap();
        let entry = state.root.entries.last().unwrap();
        assert_eq!(entry.icon, None);
        assert_eq!(entry.custom_icon, None);
    }

    /// Multi-select "Download Favicons": `favicon_jobs_selected` scopes jobs
    /// to the given entries only — same-host entries outside the selection
    /// never share the icon, and URL-less entries are skipped.
    #[test]
    fn favicon_jobs_selected_scopes_to_given_entries() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let mut uuids = Vec::new();
        for (title, url) in [
            ("Login", "https://example.com/login"),
            ("Other", "https://example.com/other"),
            ("Elsewhere", "https://elsewhere.test"),
            ("NoUrl", ""),
        ] {
            let state = session
                .add_entry(&EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: title.into(),
                    username: "u".into(),
                    password: "p".into(),
                    url: url.into(),
                    notes: String::new(),
                    totp: None,
                    expires: None,
                    icon: Some(None),
                    color: None,
                    custom_fields: Vec::new(),
                    attachments: Vec::new(),
                })
                .unwrap();
            uuids.push(state.root.entries.last().unwrap().uuid.clone());
        }

        let jobs = session.favicon_jobs_selected(&[uuids[1].clone()]).unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].host, "example.com");
        assert_eq!(jobs[0].entry_uuids, vec![uuids[1].clone()]);

        let jobs = session
            .favicon_jobs_selected(&[uuids[0].clone(), uuids[1].clone()])
            .unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].entry_uuids.len(), 2);

        let jobs = session.favicon_jobs_selected(&[uuids[3].clone()]).unwrap();
        assert!(jobs.is_empty(), "URL-less entry yields no job");

        let jobs = session
            .favicon_jobs_selected(&[
                "00000000-0000-0000-0000-000000000000".to_owned(),
                uuids[2].clone(),
            ])
            .unwrap();
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].host, "elsewhere.test");
    }

    /// The plugin tree served through GetAllDatabases must actually contain
    /// the vault's entries — an empty `childLightEntries` made the Kee browser
    /// extension show nothing, so adds/edits could not be seen either.
    #[test]
    fn plugin_tree_includes_root_and_subgroup_entries() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let group_uuid = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Mail".into(),
            })
            .unwrap()
            .root
            .children[0]
            .uuid
            .clone();
        session
            .add_entry(&EntryInput {
                group_uuid,
                title: "Webmail".into(),
                username: "alice".into(),
                password: "s3cret".into(),
                url: "https://webmail.example.com".into(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: Vec::new(),
                attachments: Vec::new(),
            })
            .expect("group entry added");
        session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "RootEntry".into(),
                username: "bob".into(),
                password: "pw".into(),
                url: "https://root.example".into(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: Vec::new(),
                attachments: Vec::new(),
            })
            .expect("root entry added");

        use crate::rpc::RpcHost;
        let db = session
            .database()
            .expect("open session exposes plugin tree");
        assert!(
            db.root.entries.iter().any(|e| e.title == "RootEntry"),
            "root-level entries must appear in the plugin tree"
        );
        let mail = db
            .root
            .children
            .iter()
            .find(|g| g.title == "Mail")
            .expect("Mail group must appear in the plugin tree");
        assert!(
            mail.entries.iter().any(|e| e.title == "Webmail"),
            "sub-group entries must appear in the plugin tree"
        );
        assert!(
            db.root.entries.iter().all(|e| e.password.is_empty()),
            "plugin tree light entries must never carry credentials"
        );
    }

    #[test]
    fn icon_to_data_url_guesses_media_types() {
        let png = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 1, 2, 3];
        let url = icon_to_data_url(&png);
        assert!(url.starts_with("data:image/png;base64,"));
        assert_eq!(BASE64.decode(url.split_once(',').unwrap().1).unwrap(), png,);

        assert!(
            icon_to_data_url(&[0x00, 0x00, 0x01, 0x00, 1]).starts_with("data:image/x-icon;base64,")
        );
        assert!(icon_to_data_url(&[0xFF, 0xD8, 0xFF]).starts_with("data:image/jpeg;base64,"));
        assert!(icon_to_data_url(b"GIF89a").starts_with("data:image/gif;base64,"));
        assert!(icon_to_data_url(b"BMXXXX").starts_with("data:image/bmp;base64,"));
        assert!(
            icon_to_data_url(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>")
                .starts_with("data:image/svg+xml;base64,")
        );
        let unknown = b"binary payload";
        assert!(icon_to_data_url(unknown).starts_with("data:image/png;base64,"));
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
                icon: Some(None),
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
                    icon: Some(None),
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
                icon: Some(None),
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
            icon: Some(None),
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
                icon: Some(None),
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
                        icon: Some(None),
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
                icon: Some(Some(1)),
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
                    icon: Some(None),
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
                    icon: Some(Some(3)),
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
                icon: Some(None),
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
                    icon: Some(None),
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

        // Second pass permanently deletes the recycled entries; the now-empty
        // recycle bin no longer occupies a slot in the tree.
        let state = session.delete_entries(&uuids).unwrap();
        assert!(state.root.children.iter().all(|g| !g.is_recycle_bin));
    }

    #[test]
    fn update_entries_applies_patch_to_all_uuids_and_skips_absent_fields() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let mut uuids = Vec::new();
        for i in 0..3 {
            let state = session
                .add_entry(&EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: format!("E{i}"),
                    username: format!("user{i}"),
                    password: "secret".into(),
                    url: format!("https://e{i}.example"),
                    notes: "note".into(),
                    totp: None,
                    expires: None,
                    icon: Some(None),
                    color: None,
                    custom_fields: vec![],
                    attachments: vec![],
                })
                .unwrap();
            uuids.push(state.root.entries.last().unwrap().uuid.clone());
        }

        let patch = EntryPatch {
            title: Some("Renamed".into()),
            username: Some("shared".into()),
            ..EntryPatch::default()
        };
        let state = session.update_entries(&uuids, &patch).unwrap();
        assert_eq!(state.root.entries.len(), 3);
        for (i, entry) in state.root.entries.iter().enumerate() {
            assert_eq!(entry.title, "Renamed");
            assert_eq!(entry.username, "shared");
            assert_eq!(
                session.get_entry_password(&entry.uuid).unwrap(),
                "secret",
                "untouched password must survive"
            );
            assert_eq!(
                entry.url,
                format!("https://e{i}.example"),
                "absent url field must stay untouched"
            );
        }
        let history = session.get_entry_history(&uuids[0]).unwrap();
        assert!(
            !history.is_empty(),
            "each patched entry gains a history snapshot"
        );
    }

    #[test]
    fn update_entries_empty_strings_and_clear_flags_clear_optional_attributes() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "E".into(),
                username: "u".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: Some("JBSWY3DPEHPK3PXP".into()),
                expires: Some("2026-12-31T23:59:00Z".into()),
                icon: Some(Some(7)),
                color: Some("#2288FF".into()),
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.entries[0].uuid.clone();
        assert!(state.root.entries[0].has_totp);

        let patch = EntryPatch {
            totp: Some("".into()),
            clear_expires: true,
            clear_icon: true,
            clear_color: true,
            ..EntryPatch::default()
        };
        let state = session.update_entries(&[uuid], &patch).unwrap();
        let entry = &state.root.entries[0];
        assert!(!entry.has_totp, "empty totp clears the seed");
        assert_eq!(entry.expires, None, "clear_expires removes the expiry");
        assert!(
            entry.icon.is_none(),
            "clear_icon resets to the default icon"
        );
        assert!(entry.color.is_none(), "clear_color removes the tag");
        assert_eq!(entry.title, "E", "absent fields stay untouched");
        assert_eq!(
            session.get_entry_password(&entry.uuid).unwrap(),
            "p",
            "untouched password must survive"
        );
    }

    #[test]
    fn update_entries_sets_expiry_icon_and_color() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "E".into(),
                username: "u".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.entries[0].uuid.clone();

        let patch = EntryPatch {
            expires: Some("2027-06-01T12:00:00Z".into()),
            icon: Some(5),
            color: Some("#00CC66".into()),
            ..EntryPatch::default()
        };
        let state = session.update_entries(&[uuid], &patch).unwrap();
        let entry = &state.root.entries[0];
        assert_eq!(entry.expires.as_deref(), Some("2027-06-01T12:00:00Z"));
        assert_eq!(entry.icon, Some(5));
        assert_eq!(entry.color.as_deref(), Some("#00CC66"));
    }

    #[test]
    fn update_entries_unknown_uuid_errors_and_empty_list_is_a_noop() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let before = session.state().unwrap().unwrap();
        assert!(!before.dirty, "fresh session is clean");
        let patch = EntryPatch {
            title: Some("X".into()),
            ..EntryPatch::default()
        };
        let err = session
            .update_entries(&["00000000-0000-0000-0000-000000000000".into()], &patch)
            .unwrap_err();
        assert!(err.contains("条目不存在"));
        assert_eq!(
            session.state().unwrap().unwrap().dirty,
            before.dirty,
            "failed batch must not change the dirty flag"
        );

        let state = session.update_entries(&[], &patch).unwrap();
        assert_eq!(
            state.dirty, before.dirty,
            "empty batch must not mark the vault dirty"
        );
    }

    #[test]
    fn update_entries_is_atomic_on_unknown_uuid() {
        let dir = TempDir::new().unwrap();
        let (mut session, _path) = create_session(&dir);
        let mut uuids = Vec::new();
        for i in 0..2 {
            let state = session
                .add_entry(&EntryInput {
                    group_uuid: ROOT_GROUP_UUID.to_owned(),
                    title: format!("E{i}"),
                    username: "".into(),
                    password: "p".into(),
                    url: "".into(),
                    notes: "".into(),
                    totp: None,
                    expires: None,
                    icon: Some(None),
                    color: None,
                    custom_fields: vec![],
                    attachments: vec![],
                })
                .unwrap();
            uuids.push(state.root.entries.last().unwrap().uuid.clone());
        }
        // A batch with a valid entry first and an unknown uuid second must
        // abort the whole batch: no entry may change, no history recorded.
        let patch = EntryPatch {
            title: Some("Batch".into()),
            ..EntryPatch::default()
        };
        let err = session
            .update_entries(
                &[
                    uuids[0].clone(),
                    "00000000-0000-0000-0000-000000000000".into(),
                ],
                &patch,
            )
            .unwrap_err();
        assert!(err.contains("条目不存在"));
        let state = session.state().unwrap().unwrap();
        assert_eq!(state.root.entries[0].title, "E0", "no partial application");
        assert_eq!(state.root.entries[1].title, "E1");
        let history = session.get_entry_history(&uuids[0]).unwrap();
        assert!(history.is_empty(), "no history snapshot on aborted batch");
    }

    #[test]
    fn save_clears_dirty_and_persists() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Mail".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        session
            .add_entry(&EntryInput {
                group_uuid,
                title: "Inbox".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let saved = session.save().unwrap();
        assert!(!saved.dirty);
        drop(session);

        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password", None).unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].name, "Mail");
        assert_eq!(state.root.children[0].entries.len(), 1);
    }

    #[test]
    fn save_as_writes_new_file_and_switches_session_target() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        let new_path = dir.path().join("copy.kdbx");
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Mail".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        session
            .add_entry(&EntryInput {
                group_uuid,
                title: "Inbox".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();

        let state = session.save_as(&new_path).unwrap();
        assert_eq!(state.path, new_path.to_string_lossy());
        assert!(!state.dirty, "save as marks the session clean");

        // The new file holds the data; the original file was never touched.
        let mut reopened = VaultSession::default();
        let state = reopened.open(&new_path, "master-password", None).unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].name, "Mail");
        assert_eq!(state.root.children[0].entries.len(), 1);
        drop(reopened);
        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password", None).unwrap();
        assert!(
            state.root.children.is_empty(),
            "original file must keep its pre-save-as content"
        );
        drop(reopened);

        // Subsequent edits and saves go to the new target.
        session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "After".into(),
                username: "".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        session.save().unwrap();
        drop(session);
        let mut reopened = VaultSession::default();
        let state = reopened.open(&new_path, "master-password", None).unwrap();
        assert_eq!(state.root.entries.len(), 1);
        assert_eq!(state.root.entries[0].title, "After");
    }

    #[test]
    fn save_as_failure_keeps_session_untouched() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Mail".into(),
            })
            .unwrap();
        let missing_dir = dir.path().join("no-such-dir").join("v.kdbx");

        let err = session.save_as(&missing_dir).unwrap_err();
        assert!(!err.is_empty(), "saving into a missing directory must fail");
        let state = session.state().unwrap().unwrap();
        assert_eq!(
            state.path,
            path.to_string_lossy(),
            "session target unchanged"
        );
        assert_eq!(state.root.children.len(), 1);
        assert!(state.dirty, "unsaved edits remain dirty");
    }

    #[test]
    fn save_as_from_remote_session_switches_to_local() {
        let dir = TempDir::new().unwrap();
        let (storage, _) = seed_remote_storage(&dir);
        let local_dir = dir.path().join("local");
        let mut session = VaultSession::default();
        let state = session
            .open_remote(
                Arc::new(storage.clone()),
                "vaults/seed.kdbx",
                "pw",
                None,
                RemoteMode::InMemory,
                &local_dir,
                3,
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap();
        assert!(state.path.starts_with("s3://"));
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Local".into(),
            })
            .unwrap();
        session
            .add_entry(&EntryInput {
                group_uuid: state.root.children[0].uuid.clone(),
                title: "Exported".into(),
                username: "".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();

        let local_path = dir.path().join("exported.kdbx");
        let state = session.save_as(&local_path).unwrap();
        assert_eq!(state.path, local_path.to_string_lossy());
        assert!(!state.dirty);

        // Later saves are local: the S3 object must not receive the group.
        session.save().unwrap();
        let remote_db = Database::parse(
            &storage.get("vaults/seed.kdbx").unwrap(),
            DatabaseKey::new().with_password("pw"),
        )
        .unwrap();
        assert_eq!(
            remote_db.root().groups().count(),
            0,
            "remote target must not receive post-save-as changes"
        );
        let mut reopened = VaultSession::default();
        let state = reopened.open(&local_path, "pw", None).unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].name, "Local");
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
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Mail".into(),
            })
            .unwrap();
        session
            .add_entry(&EntryInput {
                group_uuid: state.root.children[0].uuid.clone(),
                title: "Inbox".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
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
        assert_eq!(state.root.children[0].entries.len(), 1);
    }

    #[test]
    fn empty_group_remains_visible_after_reopen() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);

        // A freshly created empty group is visible so the user can populate it.
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "New".into(),
            })
            .unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].name, "New");
        assert!(state.root.children[0].entries.is_empty());

        // Even without saving, re-reading the session keeps it visible.
        let again = session.state().unwrap().unwrap();
        assert_eq!(again.root.children.len(), 1);

        // After persisting and reopening, the still-empty group stays visible.
        session.save().unwrap();
        drop(session);
        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password", None).unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].name, "New");
        assert!(state.root.children[0].entries.is_empty());
    }

    #[test]
    fn empty_child_group_stays_visible_after_reopen() {
        let dir = TempDir::new().unwrap();
        let (mut session, path) = create_session(&dir);
        let parent = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Parent".into(),
            })
            .unwrap()
            .root
            .children[0]
            .uuid
            .clone();
        // A nested group inside the parent is empty; both levels stay visible.
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(parent.clone()),
                icon: None,
                name: "EmptyChild".into(),
            })
            .unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].children.len(), 1);
        session.save().unwrap();
        drop(session);

        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password", None).unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].children.len(), 1);
        assert_eq!(state.root.children[0].children[0].name, "EmptyChild");
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
                icon: Some(None),
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
                    icon: Some(None),
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
                    icon: Some(None),
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
    fn disabled_expiry_flag_never_marks_entry_expired() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("official.kdbx");
        let mut db = keepass::Database::new();
        let past = chrono::NaiveDate::from_ymd_opt(2020, 1, 1)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap();
        let mut root = db.root_mut();
        // KeePass default: expiry timestamp present, expiry disabled.
        let mut disabled = root.add_entry();
        disabled.set_unprotected(FIELD_TITLE, "Disabled");
        disabled.set_unprotected(FIELD_PASSWORD, "p");
        disabled.times.expiry = Some(past);
        disabled.times.expires = Some(false);
        // Expiry status never set: `expires = None`.
        let mut unset = root.add_entry();
        unset.set_unprotected(FIELD_TITLE, "Unset");
        unset.set_unprotected(FIELD_PASSWORD, "p");
        unset.times.expiry = Some(past);
        unset.times.expires = None;
        save_database(
            &db,
            &path,
            DatabaseKey::new().with_password("master-password"),
        )
        .unwrap();
        let mut session = VaultSession::default();
        let state = session.open(&path, "master-password", None).unwrap();
        assert_eq!(state.root.entries.len(), 2);
        for entry in &state.root.entries {
            assert!(
                entry.expires.is_none(),
                "{} should not expose expiry",
                entry.title
            );
            assert!(
                !entry.expired,
                "{} should not be flagged expired",
                entry.title
            );
        }

        // A genuinely enabled past expiry is still flagged after reopen.
        session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Genuine".into(),
                username: "u".into(),
                password: "p".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: Some("2020-01-01T00:00:00Z".to_owned()),
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let state = session.state().unwrap().unwrap();
        let genuine = state
            .root
            .entries
            .iter()
            .find(|e| e.title == "Genuine")
            .expect("added entry should be present");
        assert!(genuine.expired, "enabled past expiry should be flagged");
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
                icon: Some(None),
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
                icon: Some(None),
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
                icon: Some(None),
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
                icon: Some(None),
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
        // The recycle bin now holds nothing, so it is filtered out of the
        // tree like any other empty group.
        assert!(
            !state.root.children.iter().any(|g| g.is_recycle_bin),
            "empty recycle bin should be hidden"
        );
        session.save().unwrap();
        drop(session);

        let mut reopened = VaultSession::default();
        let state = reopened.open(&path, "master-password", None).unwrap();
        assert!(state.root.entries.is_empty());
        assert!(
            !state.root.children.iter().any(|g| g.is_recycle_bin),
            "empty recycle bin should not reappear after reopen"
        );
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
                icon: Some(None),
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
                icon: Some(None),
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
                icon: Some(None),
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

        let patch: EntryPatch = serde_json::from_value(serde_json::json!({
            "title": "Batch",
            "clearExpires": true,
            "clearIcon": true,
            "clearColor": true,
        }))
        .unwrap();
        assert_eq!(patch.title.as_deref(), Some("Batch"));
        assert_eq!(patch.username, None, "absent fields stay untouched");
        assert!(patch.clear_expires && patch.clear_icon && patch.clear_color);

        let partial: EntryPatch = serde_json::from_value(serde_json::json!({})).unwrap();
        assert!(partial.title.is_none());
        assert!(!partial.clear_expires);
    }

    #[test]
    fn entry_input_icon_tristate_deserializes_absent_null_and_index() {
        let base = |mut value: serde_json::Value| {
            let obj = value.as_object_mut().unwrap();
            obj.insert("groupUuid".into(), "g1".into());
            obj.insert("title".into(), "T".into());
            obj.insert("username".into(), "u".into());
            obj.insert("password".into(), "p".into());
            obj.insert("url".into(), "https://x".into());
            obj.insert("notes".into(), "n".into());
            value
        };
        // Absent icon (content-only edit) keeps the current icon.
        let absent: EntryInput = serde_json::from_value(base(serde_json::json!({}))).unwrap();
        assert_eq!(absent.icon, None);
        // Explicit null resets to the default icon.
        let clear: EntryInput =
            serde_json::from_value(base(serde_json::json!({"icon": null}))).unwrap();
        assert_eq!(clear.icon, Some(None));
        // A number sets the built-in index.
        let set: EntryInput = serde_json::from_value(base(serde_json::json!({"icon": 7}))).unwrap();
        assert_eq!(set.icon, Some(Some(7)));
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
    fn totp_uri_without_digits_defaults_to_six() {
        // Google Authenticator exports omit `digits`; keepass 0.13 would
        // default to 8. The RFC 6238 vector secret must still yield the
        // 6-digit code, like KeePass and the raw-seed path above.
        let seed =
            "otpauth://totp/Google:user@example.com?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&issuer=Google";
        let at_59 = compute_totp_at(seed, 59).unwrap();
        assert_eq!(at_59.code, "287082");
        assert_eq!(at_59.valid_for, 1);

        let no_query = compute_totp_at(
            "otpauth://totp/Google:user?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ",
            59,
        )
        .unwrap();
        assert_eq!(no_query.code, "287082");

        // An explicit `digits=8` is respected and unchanged.
        let explicit = compute_totp_at(
            "otpauth://totp/RFC6238:test?secret=GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ&digits=8&period=30",
            59,
        )
        .unwrap();
        assert_eq!(explicit.code, "94287082");
    }

    #[test]
    fn totp_accepts_lowercase_secret_in_uri() {
        // keepass decodes secrets with the `base32` crate whose RFC 4648
        // table only accepts A-Z / 2-7; a lowercase secret (typed by hand or
        // scraped from a QR code) must be uppercased before parsing.
        let user_uri =
            "otpauth://totp/Google:m2uyoo@gmail.com?secret=2r23njeqijx7zfia7u2b2ena4lhkkuwt&issuer=Google";
        let code = compute_totp_at(user_uri, 1_700_000_000).expect("lowercase secret must decode");
        assert_eq!(code.code.len(), 6);
        assert_eq!(code.period, 30);

        // The lowercase URI must yield the same code as its uppercased twin.
        let upper_uri =
            "otpauth://totp/Google:m2uyoo@gmail.com?secret=2R23NJEQIJX7ZFIA7U2B2ENA4LHKKUWT&issuer=Google";
        assert_eq!(
            compute_totp_at(upper_uri, 1_700_000_000).unwrap().code,
            code.code,
        );

        // Raw lowercase Base32 keys were already normalized; keep it working.
        let raw = compute_totp_at("2r23njeqijx7zfia7u2b2ena4lhkkuwt", 1_700_000_000)
            .expect("raw lowercase key must decode");
        assert_eq!(raw.code, code.code);
    }

    #[test]
    fn totp_rejects_invalid_seed() {
        let err = compute_totp_at("INVALID!", 59).unwrap_err();
        assert!(err.contains("Base32"), "unexpected error: {err}");
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
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.children[0].entries[0].uuid.clone();
        let err = session.totp_code(&uuid).unwrap_err();
        assert!(err.contains("没有 OTP"), "unexpected error: {err}");
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
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.children[0].entries[0].uuid.clone();
        let code = session.totp_code(&uuid).unwrap();
        assert_eq!(code.code.len(), 6);
        assert_eq!(code.period, 30);
        assert_eq!(code.kind, "totp");
        assert!((1..=code.period).contains(&code.valid_for));
    }

    /// HOTP reads its counter from the `HmacOtp` field and advances it on
    /// each request, writing `counter+1` back server-side.
    #[test]
    fn hotp_code_advances_counter_and_writes_back() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_entry(&EntryInput {
                group_uuid: ROOT_GROUP_UUID.to_owned(),
                title: "Hotp".into(),
                username: "".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![CustomField {
                    name: "HmacOtp".into(),
                    value: "JBSWY3DPEHPK3PXP".into(),
                }],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.entries[0].uuid.clone();
        let first = session.totp_code(&uuid).unwrap();
        assert_eq!(first.kind, "hotp");
        assert_eq!(first.period, 0);
        assert_eq!(first.counter, Some(0));
        let second = session.totp_code(&uuid).unwrap();
        assert_eq!(second.counter, Some(1));
        // A third call keeps advancing (no repeat of an earlier code).
        let third = session.totp_code(&uuid).unwrap();
        assert_eq!(third.counter, Some(2));
    }

    /// A Steam guard entry yields a 5-character code from the Steam alphabet
    /// with a live countdown (time-based).
    #[test]
    fn steam_code_is_five_chars_with_countdown() {
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
                title: "Steam".into(),
                username: "".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![CustomField {
                    name: "SteamOtp".into(),
                    value: "CNBNMZBN".into(),
                }],
                attachments: vec![],
            })
            .unwrap();
        let uuid = state.root.children[0].entries[0].uuid.clone();
        let code = session.totp_code(&uuid).unwrap();
        assert_eq!(code.kind, "steam");
        assert_eq!(code.code.len(), 5);
        assert_eq!(code.period, 30);
        assert!((1..=code.period).contains(&code.valid_for));
    }

    #[test]
    fn totp_code_wire_format_uses_camel_case() {
        let code = compute_totp_at("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ", 59).unwrap();
        let json = serde_json::to_value(&code).unwrap();
        let obj = json.as_object().unwrap();
        for key in ["code", "kind", "validFor", "period"] {
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
            icon: Some(None),
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
                icon: Some(None),
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
                icon: Some(None),
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
                icon: Some(None),
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
                    icon: Some(None),
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
                icon: Some(None),
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
                icon: Some(None),
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
        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Mail".into(),
            })
            .unwrap();
        session
            .add_entry(&EntryInput {
                group_uuid: state.root.children[0].uuid.clone(),
                title: "Inbox".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
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
                icon: Some(None),
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
                icon: Some(None),
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
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap();
        assert_eq!(state.path, "s3://vaults/seed.kdbx");
        assert_eq!(state.file_name, "seed.kdbx");

        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Web".into(),
            })
            .unwrap();
        session
            .add_entry(&EntryInput {
                group_uuid: state.root.children[0].uuid.clone(),
                title: "Site".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
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
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap();
        assert_eq!(state.root.children.len(), 1);
        assert_eq!(state.root.children[0].name, "Web");
        assert_eq!(state.root.children[0].entries.len(), 1);
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
                DEFAULT_BACKUP_TEMPLATE,
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
    fn remote_backup_uses_custom_template_and_prunes_by_shape() {
        let dir = TempDir::new().unwrap();
        let (storage, _) = seed_remote_storage(&dir);
        let local = dir.path().join("mirror");
        const TEMPLATE: &str = "{name}-backup-{timestamp}.{ext}.old";

        let mut session = VaultSession::default();
        session
            .open_remote(
                Arc::new(storage.clone()),
                "vaults/seed.kdbx",
                "pw",
                None,
                RemoteMode::SaveLocal,
                &local,
                2,
                TEMPLATE,
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

        let backups: Vec<_> = std::fs::read_dir(&local)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.starts_with("seed-backup-") && name.ends_with(".kdbx.old"))
            .collect();
        assert_eq!(backups.len(), 2, "keeps only the newest two");
        for name in &backups {
            assert!(
                !name.ends_with(".bak"),
                "custom template must shape the backup name: {name}"
            );
        }
        let old_style: Vec<_> = std::fs::read_dir(&local)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".kdbx.bak"))
            .collect();
        assert!(old_style.is_empty(), "no default-template backups");
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
                DEFAULT_BACKUP_TEMPLATE,
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
                icon: Some(None),
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
                icon: Some(None),
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
            icon: Some(None),
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
                DEFAULT_BACKUP_TEMPLATE,
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
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap_err();
        assert!(err.contains("下载"));

        let err = session
            .open_remote(
                Arc::new(storage.clone()),
                "vaults/missing.kdbx",
                "pw",
                None,
                RemoteMode::InMemory,
                &local,
                3,
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap_err();
        assert!(err.contains("下载"));

        let err = RemoteMode::parse("cloud").unwrap_err();
        assert!(err.contains("模式"));
        assert_eq!(RemoteMode::parse("memory").unwrap(), RemoteMode::InMemory);
        assert_eq!(RemoteMode::parse("local").unwrap(), RemoteMode::SaveLocal);
    }

    #[test]
    fn remote_opens_database_under_any_key_name() {
        let dir = TempDir::new().unwrap();
        let (storage, seed_path) = seed_remote_storage(&dir);
        let storage = Arc::new(storage);
        let local = dir.path().join("local");
        let seed_bytes = std::fs::read(&seed_path).unwrap();

        // A valid database under a key WITHOUT a `.kdbx` suffix opens normally.
        storage.seed("vaults/backup-noext", seed_bytes.clone());
        let mut session = VaultSession::default();
        let state = session
            .open_remote(
                storage.clone(),
                "vaults/backup-noext",
                "pw",
                None,
                RemoteMode::InMemory,
                &local,
                3,
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap();
        assert_eq!(state.path, "s3://vaults/backup-noext");

        // A non-database object under any key name fails at parse with a clear error.
        storage.seed("vaults/notes", b"not a kdbx at all".to_vec());
        let err = session
            .open_remote(
                storage.clone(),
                "vaults/notes",
                "pw",
                None,
                RemoteMode::InMemory,
                &local,
                3,
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap_err();
        assert!(err.contains("无法打开数据库"));
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
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap();
        assert_eq!(state.path, "s3://new/vault.kdbx");
        assert!(storage.get("new/vault.kdbx").is_ok());

        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Web".into(),
            })
            .unwrap();
        session
            .add_entry(&EntryInput {
                group_uuid: state.root.children[0].uuid.clone(),
                title: "Site".into(),
                username: "u".into(),
                password: "pw".into(),
                url: "".into(),
                notes: "".into(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                custom_fields: vec![],
                attachments: vec![],
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
                DEFAULT_BACKUP_TEMPLATE,
            )
            .unwrap();
        assert_eq!(state.root.children[0].name, "Web");
        assert_eq!(state.root.children[0].entries.len(), 1);
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
                DEFAULT_BACKUP_TEMPLATE,
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
            icon: Some(None),
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
                icon: Some(None),
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
        assert_eq!(
            url_host("https://github.com/login"),
            Some("github.com".into())
        );
        assert_eq!(url_host("http://a.b.c:8080/x?y=1"), Some("a.b.c".into()));
        assert_eq!(url_host("plain-host"), Some("plain-host".into()));
        assert_eq!(url_host(""), None);
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
            icon: Some(None),
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
                    icon: Some(None),
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
                icon: Some(None),
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

    // -- browser bridge (KeePassHttp) --------------------------------------

    fn entry_input(
        group_uuid: &str,
        title: &str,
        username: &str,
        password: &str,
        url: &str,
    ) -> EntryInput {
        EntryInput {
            group_uuid: group_uuid.to_owned(),
            title: title.to_owned(),
            username: username.to_owned(),
            password: password.to_owned(),
            url: url.to_owned(),
            notes: String::new(),
            totp: None,
            expires: None,
            icon: Some(None),
            color: None,
            custom_fields: Vec::new(),
            attachments: Vec::new(),
        }
    }

    #[test]
    fn bridge_client_keys_are_session_held_and_wiped_on_close() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        assert_eq!(session.list_clients(), Vec::<String>::new());
        assert!(session.client_key("browser-a").is_none());

        session.register_client("browser-a", vec![1u8; 32]);
        session.register_client("browser-b", vec![2u8; 32]);
        assert_eq!(session.client_key("browser-a").unwrap(), vec![1u8; 32]);
        assert!(session.list_clients().contains(&"browser-a".to_owned()));
        assert!(session.list_clients().contains(&"browser-b".to_owned()));

        assert!(session.remove_client("browser-a"));
        assert!(!session.remove_client("browser-a"));
        assert_eq!(session.list_clients(), vec!["browser-b".to_owned()]);

        session.close();
        assert!(!session.is_open());
        assert_eq!(session.list_clients(), Vec::<String>::new());
        assert!(session.client_key("browser-b").is_none());
    }

    #[test]
    fn bridge_logins_match_host_and_subdomains_but_not_recycle_bin() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let root = ROOT_GROUP_UUID.to_owned();

        let state = session
            .add_entry(&entry_input(
                &root,
                "主站",
                "user@example",
                "pw-1",
                "https://example.com",
            ))
            .unwrap();
        let _ = state;
        session
            .add_entry(&entry_input(
                &root,
                "子域",
                "user@www",
                "pw-2",
                "https://www.example.com",
            ))
            .unwrap();
        let state = session
            .add_entry(&entry_input(
                &root,
                "无关",
                "user@else",
                "pw-3",
                "https://elsewhere.io",
            ))
            .unwrap();
        let www_uuid = state.root.entries[1].uuid.clone();
        let other_uuid = state.root.entries[2].uuid.clone();
        let _ = other_uuid;

        // Exact host, subdomain-of, and superdomain-of all match.
        let logins = session.logins_for("https://example.com/login", None);
        assert_eq!(logins.len(), 2);
        assert!(logins.iter().any(|l| l.login == "user@www"));

        // A request subdomain matches the bare entry host only.
        let logins = session.logins_for("https://sub.example.com", None);
        assert_eq!(logins.len(), 1);
        assert_eq!(logins[0].login, "user@example");

        // Submit URL can match too (elsewhere + example.com + www.example.com).
        let logins = session.logins_for("https://elsewhere.io", Some("https://example.com"));
        assert_eq!(logins.len(), 3);

        // No URL at all matches nothing.
        let logins = session.logins_for("https://example.com", None);
        assert!(logins.iter().all(|l| !l.uuid.is_empty()));
        assert!(session.logins_for("", None).is_empty());
        assert!(session.logins_for("https://nomatch.xyz", None).is_empty());

        // Entries moved to the recycle bin are invisible to the bridge.
        session.delete_entry(&www_uuid).unwrap();
        let logins = session.logins_for("https://www.example.com", None);
        assert!(logins.iter().all(|l| l.uuid != www_uuid));
        let logins = session.logins_for("https://example.com", None);
        assert!(logins.iter().all(|l| l.uuid != www_uuid));
        assert!(logins.iter().any(|l| l.login == "user@example"));
    }

    #[test]
    fn bridge_set_login_updates_entry_fields() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let root = ROOT_GROUP_UUID.to_owned();

        let state = session
            .add_entry(&entry_input(
                &root,
                "站点",
                "old-user",
                "old-pw",
                "https://example.com",
            ))
            .unwrap();
        let uuid = state.root.entries[0].uuid.clone();

        session
            .set_login("new-user", "new-pw", "https://example.com/sso", Some(&uuid))
            .unwrap();
        assert_eq!(session.get_entry_password(&uuid).unwrap(), "new-pw");
        assert_eq!(
            session.autotype_context(&uuid).unwrap().username,
            "new-user"
        );
        assert_eq!(
            session.autotype_context(&uuid).unwrap().url,
            "https://example.com/sso"
        );

        let err = session.set_login(
            "u",
            "p",
            "https://example.com",
            Some("00000000-0000-0000-0000-000000000000"),
        );
        assert!(err.is_err());
    }

    #[test]
    fn bridge_create_login_adds_entry_with_host_title() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        session
            .create_login("fresh-user", "fresh-pw", "https://fresh.example.net/x")
            .unwrap();

        let state = session.state().unwrap().unwrap();
        assert_eq!(state.root.entries.len(), 1);
        let entry = &state.root.entries[0];
        assert_eq!(entry.title, "fresh.example.net");
        assert_eq!(entry.username, "fresh-user");

        session.create_login("u2", "p2", "not-a-url").unwrap();
        let state = session.state().unwrap().unwrap();
        assert_eq!(state.root.entries[1].title, "not-a-url");
    }

    #[test]
    fn bridge_db_hash_is_sha1_hex_of_root_and_recycle_bin() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        let hash = session.db_hash();
        assert_eq!(hash.len(), 40);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));

        // A recycled entry changes the hash (recycle-bin uuid is part of it).
        let state = session
            .add_entry(&entry_input(ROOT_GROUP_UUID, "x", "u", "p", "https://a.b"))
            .unwrap();
        let uuid = state.root.entries[0].uuid.clone();
        session.delete_entry(&uuid).unwrap();
        let after = session.db_hash();
        assert_eq!(after.len(), 40);
        assert_ne!(after, hash);

        session.close();
        assert_eq!(session.db_hash(), "");
    }

    // -- KeePassRPC host ----------------------------------------------------

    #[test]
    fn rpc_keys_are_session_held_and_wiped_on_close() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        assert!(session.rpc_key("user@browser").is_none());
        session.register_rpc_key("user@browser", vec![7u8; 32]);
        assert_eq!(session.rpc_key("user@browser").unwrap(), vec![7u8; 32]);

        session.close();
        assert!(session.rpc_key("user@browser").is_none());
    }

    #[test]
    fn rpc_database_dto_builds_group_tree_and_skips_recycle_bin() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        let state = session
            .add_group(&GroupInput {
                parent_uuid: Some(ROOT_GROUP_UUID.to_owned()),
                icon: None,
                name: "Internet".into(),
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();
        session
            .add_entry(&entry_input(
                &group_uuid,
                "Example",
                "alice",
                "s3cret",
                "https://example.com/login",
            ))
            .unwrap();
        let root_entry = session
            .add_entry(&entry_input(
                ROOT_GROUP_UUID,
                "Trash",
                "ghost",
                "pw-x",
                "https://ghost.example",
            ))
            .unwrap();
        let trash_uuid = root_entry.root.entries[0].uuid.clone();

        let db = session.database().unwrap();
        assert_eq!(db.file_name, "test.kdbx");
        assert!(db.active);
        assert_eq!(db.root.title, "Root");
        assert_eq!(db.root.children.len(), 1);
        assert_eq!(db.root.children[0].title, "Internet");
        assert_eq!(db.root.children[0].path, "Root/Internet");
        assert_eq!(db.root.children[0].children.len(), 0);

        // Moved to recycle bin: gone from the tree and from FindLogins.
        session.delete_entry(&trash_uuid).unwrap();
        let db = session.database().unwrap();
        assert!(db.root.entries.is_empty());
        let logins = session.find_logins(&["https://ghost.example".to_owned()], None, None, None);
        assert!(logins.is_empty());
    }

    #[test]
    fn rpc_find_logins_matches_url_uuid_and_free_text() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let root = ROOT_GROUP_UUID.to_owned();

        session
            .add_entry(&entry_input(
                &root,
                "Example",
                "alice",
                "s3cret",
                "https://example.com/login",
            ))
            .unwrap();
        session
            .add_entry(&entry_input(
                &root,
                "Other",
                "bob",
                "pw-2",
                "https://other.example",
            ))
            .unwrap();

        let by_url = session.find_logins(
            &["https://example.com/dashboard".to_owned()],
            None,
            None,
            None,
        );
        assert_eq!(by_url.len(), 1);
        assert_eq!(by_url[0].username, "alice");
        assert_eq!(by_url[0].password, "s3cret");
        assert_eq!(by_url[0].urls, vec!["https://example.com/login".to_owned()]);
        assert_eq!(by_url[0].parent_group.title, "Root");

        let uuid = by_url[0].uuid.clone();
        let by_uuid = session.find_logins(&[], Some(&uuid), None, None);
        assert_eq!(by_uuid.len(), 1);

        let by_text = session.find_logins(&[], None, Some("Examp"), None);
        assert_eq!(by_text.len(), 1);
        assert_eq!(by_text[0].title, "Example");

        let by_username = session.find_logins(&[], None, None, Some("bob"));
        assert_eq!(by_username.len(), 1);
        assert_eq!(by_username[0].username, "bob");

        session.close();
        assert!(session
            .find_logins(&["https://example.com".to_owned()], None, None, None)
            .is_empty());
        assert!(session.database().is_none());
    }

    // -- KeePassRPC write path (AddLogin/UpdateLogin) ----------------------

    fn rpc_login_write(
        title: &str,
        username: &str,
        password: &str,
        urls: &[&str],
    ) -> RpcLoginWrite {
        use crate::rpc::RpcFieldWrite;
        RpcLoginWrite {
            title: title.to_owned(),
            urls: urls.iter().map(|u| u.to_string()).collect(),
            http_realm: String::new(),
            icon_image_data: String::new(),
            form_field_list: vec![
                RpcFieldWrite {
                    id: "u".to_owned(),
                    name: "user".to_owned(),
                    display_name: "KeePass username".to_owned(),
                    field_type: "FFTusername".to_owned(),
                    value: username.to_owned(),
                    page: 0,
                },
                RpcFieldWrite {
                    id: "p".to_owned(),
                    name: "pass".to_owned(),
                    display_name: "KeePass password".to_owned(),
                    field_type: "FFTpassword".to_owned(),
                    value: password.to_owned(),
                    page: 0,
                },
                RpcFieldWrite {
                    id: "n".to_owned(),
                    name: "note".to_owned(),
                    display_name: "Custom note".to_owned(),
                    field_type: "FFTtext".to_owned(),
                    value: "hello".to_owned(),
                    page: 0,
                },
            ],
        }
    }

    #[test]
    fn rpc_add_login_creates_entry_with_fields_and_urls() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        let login = rpc_login_write("站点", "alice", "pw-1", &["https://rpc.example.com/login"]);
        let created = session.add_login(&login, "").unwrap();
        assert!(!created.uuid.is_empty());
        assert_eq!(created.title, "站点");
        assert_eq!(created.username, "alice");
        assert_eq!(created.password, "pw-1");
        assert_eq!(
            created.urls,
            vec!["https://rpc.example.com/login".to_owned()]
        );
        assert_eq!(created.parent_group.title, "Root");
        assert_eq!(created.parent_group.path, "Root");

        // Username/password land in the standard fields, the extra form field
        // becomes a custom string, and FindLogins sees the new entry.
        let state = session.state().unwrap().unwrap();
        let entry = &state.root.entries[0];
        assert_eq!(entry.username, "alice");
        assert_eq!(entry.url, "https://rpc.example.com/login");
        assert!(entry
            .custom_fields
            .iter()
            .any(|f| f.name == "Custom note" && f.value == "hello"));

        let by_url = session.find_logins(
            &["https://rpc.example.com/dashboard".to_owned()],
            None,
            None,
            None,
        );
        assert_eq!(by_url.len(), 1);
        assert_eq!(by_url[0].uuid, created.uuid);
    }

    #[test]
    fn rpc_add_login_lands_in_specified_group_and_skips_recycle_bin_parent() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let state = session
            .add_group(&GroupInput {
                parent_uuid: None,
                name: "Internet".to_owned(),
                icon: None,
            })
            .unwrap();
        let group_uuid = state.root.children[0].uuid.clone();

        let login = rpc_login_write("站点", "bob", "pw", &["https://grp.example.com"]);
        let created = session.add_login(&login, &group_uuid).unwrap();
        assert_eq!(created.parent_group.uuid, group_uuid);

        let state = session.state().unwrap().unwrap();
        let group = &state.root.children[0];
        assert_eq!(group.entries.len(), 1);
        assert_eq!(group.entries[0].title, "站点");

        // Unknown or invalid parent uuid falls back to the root group.
        let created = session
            .add_login(&login, "00000000-0000-0000-0000-000000000000")
            .unwrap();
        assert_eq!(created.parent_group.title, "Root");
        assert_eq!(created.parent_group.path, "Root");
    }

    #[test]
    fn rpc_update_login_merges_urls_and_snapshots_history() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);

        let login = rpc_login_write("站点", "alice", "pw-1", &["https://old.example.com"]);
        let created = session.add_login(&login, "").unwrap();

        // Mode 1: old URL kept, new one promoted to primary.
        let update = rpc_login_write("站点", "alice", "pw-2", &["https://new.example.com"]);
        let updated = session.update_login(&update, &created.uuid, 1).unwrap();
        assert_eq!(updated.username, "alice");
        assert_eq!(updated.password, "pw-2");
        assert_eq!(
            updated.urls,
            vec![
                "https://new.example.com".to_owned(),
                "https://old.example.com".to_owned(),
            ]
        );

        // The pre-edit state was snapshotted into the entry history (the
        // plugin's `CreateBackup`): old password is recoverable.
        let id = parse_entry_id(&created.uuid).unwrap();
        let entry = session.db.as_ref().unwrap().entry(id).unwrap();
        let historical = entry.historical(0).unwrap();
        assert_eq!(historical.get_password(), Some("pw-1"));
        assert_eq!(historical.get_url(), Some("https://old.example.com"));

        // Mode 5 replaces the whole list.
        let updated = session.update_login(&update, &created.uuid, 5).unwrap();
        assert_eq!(updated.urls, vec!["https://new.example.com".to_owned()]);
    }

    #[test]
    fn rpc_update_login_rejects_unknown_uuid_recycle_bin_and_locked() {
        let dir = TempDir::new().unwrap();
        let (mut session, _) = create_session(&dir);
        let root = ROOT_GROUP_UUID.to_owned();
        let state = session
            .add_entry(&entry_input(
                &root,
                "Bin",
                "u",
                "p",
                "https://bin.example.com",
            ))
            .unwrap();
        let bin_uuid = state.root.entries[0].uuid.clone();

        let login = rpc_login_write("Bin", "u", "p2", &["https://other.example.com"]);

        // Unknown entry uuid → EntryNotFound.
        assert_eq!(
            session.update_login(&login, "00000000-0000-0000-0000-000000000000", 5),
            Err(RpcError::EntryNotFound)
        );

        // Entries moved to the recycle bin are rejected.
        session.delete_entry(&bin_uuid).unwrap();
        assert_eq!(
            session.update_login(&login, &bin_uuid, 5),
            Err(RpcError::InRecycleBin)
        );

        // A locked vault rejects both write methods.
        session.close();
        assert_eq!(session.add_login(&login, ""), Err(RpcError::Locked));
        assert_eq!(
            session.update_login(&login, &bin_uuid, 5),
            Err(RpcError::Locked)
        );
    }
}
