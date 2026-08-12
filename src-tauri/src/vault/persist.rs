//! Lock-free save/persist primitives: build the decrypted `Database`, run the
//! KDF, serialize, and write to local disk or remote storage (extracted from
//! mod.rs).

use super::helpers::{
    apply_cipher, apply_compression, apply_kdf, build_database_key, classify_open_error,
    probe_vault, save_database, wipe_secret_bytes, wipe_secret_string, write_database_bytes,
};
use super::RemoteMode;
use crate::remote::backup::{remote_key_basename, validate_remote_key, write_local_copy};
use crate::remote::RemoteStorage;
use keepass::{Database, DatabaseKey};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub(crate) enum SaveTarget {
    Local(PathBuf),
    Remote {
        storage: Arc<dyn RemoteStorage>,
        key: String,
        mode: RemoteMode,
        local_dir: PathBuf,
        backup_count: usize,
        backup_template: String,
        base_hash: [u8; 32],
    },
}

/// Error prefix for a save that detected the remote file changed elsewhere.
/// The frontend matches this marker to offer 覆盖远程/下载远程/保留本地.
pub(crate) const REMOTE_CHANGED_MARKER: &str = "REMOTE_CHANGED\n";

/// Result of a remote open/create: database, keyfile bytes, normalized key and
/// the SHA-256 of the remote file bytes (the sync base hash).
type PreparedRemote = (Database, Option<Vec<u8>>, String, [u8; 32]);

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
    /// Skip the remote conflict check (explicit "覆盖远程" user choice).
    pub force: bool,
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
    let probe = probe_vault(path)?;
    if probe.kind == "unknown" {
        return Err(probe.note);
    }
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
) -> Result<PreparedRemote, String> {
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
    Ok((db, keyfile_bytes, key, crate::crypto::sha256_bytes(&data)))
}

/// Lock-free half of `create_remote`: build the database, run the KDF,
/// serialize, upload through the configured remote transport and mirror locally
/// if requested.
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
) -> Result<PreparedRemote, String> {
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
    Ok((db, keyfile_bytes, key, crate::crypto::sha256_bytes(&buffer)))
}

/// Serialize `db` with `key` and persist it to the given target. Runs
/// entirely outside the session lock.
pub(crate) fn persist_snapshot(
    db: &Database,
    key: &DatabaseKey,
    target: &SaveTarget,
    force: bool,
) -> Result<[u8; 32], String> {
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
            base_hash,
        } => {
            if !force {
                // Conflict check: refuse to overwrite a remote file that
                // changed since it was opened/last saved.
                let current = storage
                    .get(key)
                    .map_err(|e| format!("读取远程当前版本失败: {e}"))?;
                if crate::crypto::sha256_bytes(&current) != *base_hash {
                    return Err(format!(
                        "{REMOTE_CHANGED_MARKER}远程库已被其他设备修改（远程 {} 字节 / 本地 {} 字节），请选择覆盖远程、下载远程或保留本地",
                        current.len(),
                        buffer.len(),
                    ));
                }
            }
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
            Ok(crate::crypto::sha256_bytes(&buffer))
        }
        SaveTarget::Local(path) => {
            write_database_bytes(path, &buffer)?;
            Ok(crate::crypto::sha256_bytes(&buffer))
        }
    }
}

/// Full lock-free save: derive the session key (KDF), then serialize and
/// persist. Secret clones are zeroized afterwards.
pub(crate) fn persist_save(job: SaveJob) -> Result<[u8; 32], String> {
    let key = build_database_key(&job.password, job.keyfile.as_deref())?;
    let result = persist_snapshot(&job.db, &key, &job.target, job.force);
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
) -> Result<[u8; 32], String> {
    let key = build_database_key(password, keyfile)?;
    persist_snapshot(db, &key, target, false)
}

/// Write attachment bytes extracted under the lock (file I/O outside it).
pub(crate) fn write_attachment_file(data: &[u8], dest: &str) -> Result<(), String> {
    std::fs::write(dest, data).map_err(|e| format!("写入附件失败: {e}"))
}

/// Write CSV content built under the lock (file I/O outside it).
pub(crate) fn write_csv_file(path: &str, content: &str) -> Result<(), String> {
    std::fs::write(path, content).map_err(|e| format!("写入文件失败: {e}"))
}
