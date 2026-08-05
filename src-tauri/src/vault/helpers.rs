//! Free functions shared by the vault session: entry/group id parsing,
//! recycle-bin helpers, OTP field resolution, auto-type match walking, and
//! database write / KDF / cipher / compression application (extracted from
//! mod.rs).

use super::{
    AES_KDF_ROUNDS, ARGON2_ITERATIONS, ARGON2_MEMORY_KIB, ARGON2_PARALLELISM, FIELD_HMAC_OTP,
    FIELD_KPRPC_CONFIG, FIELD_NOTES, FIELD_OTP, FIELD_PASSWORD, FIELD_STEAM_OTP,
    FIELD_STEAM_OTP_ALT, FIELD_TIME_OTP, FIELD_URL, FIELD_USERNAME, RESERVED_FIELDS,
    ROOT_GROUP_UUID,
};
use crate::crypto::otp;
use crate::platform::autotype;
use crate::util::url_host;
use keepass::config::{CompressionConfig, KdfConfig, OuterCipherConfig};
use keepass::db::{Entry, EntryId, GroupId, GroupRef};
use keepass::{Database, DatabaseKey};
use std::io::Cursor;
use std::path::Path;
use uuid::Uuid;
use zeroize::Zeroize;
/// Wipe a secret `String` in place, then drop it (buffer is zeroed before
/// the heap allocation is freed). Best-effort: only this owned copy is
/// cleared — copies made by the OS, IPC, or `DatabaseKey` internals are
/// outside our control (`keepass` zeroizes its own key material on drop).
pub(crate) fn wipe_secret_string(secret: &mut String) {
    secret.zeroize();
}

/// Wipe a secret byte buffer in place (see `wipe_secret_string`).
pub(crate) fn wipe_secret_bytes(secret: &mut Vec<u8>) {
    secret.zeroize();
}

/// Combine password and/or keyfile into a `DatabaseKey`. At least one
/// component must be present.
pub(crate) fn build_database_key(
    password: &str,
    keyfile: Option<&[u8]>,
) -> Result<DatabaseKey, String> {
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn parse_entry_id(s: &str) -> Result<EntryId, String> {
    Uuid::parse_str(s)
        .map(EntryId::from_uuid)
        .map_err(|_| format!("无效的条目 UUID: {s}"))
}

pub(crate) fn parse_group_id(s: &str) -> Result<GroupId, String> {
    Uuid::parse_str(s)
        .map(GroupId::from_uuid)
        .map_err(|_| format!("无效的分组 UUID: {s}"))
}

/// The recycle bin group id, when the database has one.
pub(crate) fn recycle_bin_id(db: &Database) -> Option<GroupId> {
    db.meta.recyclebin_uuid.map(GroupId::from_uuid)
}

/// Parse the KeePassRPC per-entry config (`KPRPC JSON` custom field) and
/// return its `altURLs` array — the extra/custom URLs a Kee browser extension
/// uses to match an entry. Missing or malformed config degrades to an empty
/// list (entry then matches on its primary URL only).
fn kprpc_alt_urls(entry: &keepass::db::EntryRef<'_>) -> Vec<String> {
    let Some(raw) = entry.get(FIELD_KPRPC_CONFIG) else {
        return Vec::new();
    };
    let value: serde_json::Value = match serde_json::from_str(raw) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    value
        .get("altURLs")
        .and_then(serde_json::Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|u| u.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Every URL an entry should match against, for browser-bridge and auto-type
/// matching: the primary `URL` field (space-separated list) plus any
/// KeePassRPC `altURLs` custom URLs. Entries edited in Kee match their
/// alternative URLs too; empty entries (no URL, no altURLs) never match.
pub(crate) fn entry_match_urls(entry: &keepass::db::EntryRef<'_>) -> Vec<String> {
    let mut urls: Vec<String> = entry
        .get(FIELD_URL)
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    urls.extend(kprpc_alt_urls(entry));
    urls
}

/// Depth-first scan of `group`'s subtree for a `{REF:...}` match. First
/// matching entry wins (KeePass semantics); the recycle bin is skipped.
pub(crate) fn walk_ref_match(
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
pub(crate) fn walk_match(
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
        if entry_match_urls(&entry).iter().any(|u| {
            let host = url_host(u).unwrap_or_default();
            !host.is_empty() && window_title.contains(&host)
        }) {
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
pub(crate) fn ensure_recycle_bin(db: &mut Database) -> Result<GroupId, String> {
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
pub(crate) fn group_contains(db: &Database, ancestor: GroupId, group_id: GroupId) -> bool {
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
pub(crate) fn resolve_group_id(db: &Database, uuid: &str) -> Result<GroupId, String> {
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
pub(crate) fn parse_entry_otp_spec(entry: &Entry) -> Result<otp::OtpSpec, String> {
    let (field, value) = entry_otp_field(entry).ok_or_else(|| "该条目没有 OTP 种子".to_owned())?;
    match field {
        FIELD_HMAC_OTP => otp::parse_hotp_seed(value),
        FIELD_STEAM_OTP | FIELD_STEAM_OTP_ALT => otp::parse_steam_seed(value),
        _ => otp::parse_totp_seed(value),
    }
}

pub(crate) fn otp_kind_name(kind: otp::OtpKind) -> &'static str {
    match kind {
        otp::OtpKind::Totp => "totp",
        otp::OtpKind::Hotp => "hotp",
        otp::OtpKind::Steam => "steam",
    }
}

pub(crate) fn classify_open_error<E: std::fmt::Display>(e: E) -> String {
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

pub(crate) fn save_database(db: &Database, path: &Path, key: DatabaseKey) -> Result<(), String> {
    let mut buffer = Vec::new();
    db.save(&mut Cursor::new(&mut buffer), key)
        .map_err(|e| format!("序列化数据库失败: {e}"))?;
    write_database_bytes(path, &buffer)
}

/// Atomic write of already-serialized KDBX bytes (local vault save).
pub(crate) fn write_database_bytes(path: &Path, buffer: &[u8]) -> Result<(), String> {
    crate::util::atomic_write(path, buffer, "数据库")
}

pub(crate) fn apply_kdf(db: &mut Database, kdf: &str) -> Result<(), String> {
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

pub(crate) fn apply_cipher(db: &mut Database, cipher: &str) -> Result<(), String> {
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

pub(crate) fn apply_compression(db: &mut Database, compression: &str) -> Result<(), String> {
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
