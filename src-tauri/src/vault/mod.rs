//! Vault session: keep the decrypted `keepass::Database` in memory and expose
//! the IPC-facing commands as testable methods. Serialized shapes mirror
//! `src/lib/types/vault.ts`.

pub mod dto;
mod entries;
pub(crate) mod helpers;
mod hosts;
mod persist;
mod security;
mod serialize;
mod session;
mod sessions;
#[cfg(test)]
mod tests;

use keepass::Database;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::remote::RemoteStorage;

pub(crate) use self::helpers::{
    entry_has_otp, entry_match_urls, kprpc_matches_url, parse_entry_id, recycle_bin_id,
};
pub(crate) use self::persist::{
    persist_change, persist_save, prepare_local_create, prepare_local_open, prepare_remote_create,
    prepare_remote_open, read_keyfile, write_attachment_file, write_csv_file,
};

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
pub(crate) const FIELD_FAVORITE: &str = "SecPivot.Favorite";
pub(crate) const FIELD_FAVORITE_TRUE: &str = "true";
/// Custom field recording the group an entry lived in before being recycled,
/// so it can be restored to its original location.
const FIELD_ORIGINAL_GROUP: &str = "SecPivot.OriginalGroup";
/// KeePassRPC per-entry configuration (JSON: `altURLs`, `blockedURLs`,
/// `regExURLs`, `regExBlockedURLs`, `blockHostnameOnlyMatch`, …), written by
/// the Kee browser extension. SecPivot reads the full config so entries
/// edited in Kee match against their extra/custom URLs, regex rules and block
/// lists at the configured accuracy (see `helpers::kprpc_matches_url`).
pub(crate) const FIELD_KPRPC_CONFIG: &str = "KPRPC JSON";

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
// Type definitions live in `vault::dto`; re-exported here so the session
// code and `lib.rs` keep referencing them via `vault::*`.

pub use self::dto::{
    AttachmentInfo, AttachmentInput, AutoTypeAssociationDto, AutotypeCandidate, CustomField,
    DatabaseSettings, DatabaseSettingsPatch, DuplicatePasswords, EntryAutoTypeConfig,
    EntryAutoTypeInput, EntryInput, EntryPatch, EntryStorage, FaviconFetch, FaviconJob,
    FaviconProgress, FaviconReport, GroupAutoTypeConfig, GroupAutoTypeInput, GroupInput,
    HistoryVersion, MutationDelta, SecurityReport, SessionInfo, TotpCode, VaultEntry, VaultGroup,
    VaultOpenResult, VaultState, WeakEntry,
};

pub use self::sessions::VaultSessions;

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// How a remote vault is persisted. `InMemory` uploads through the configured
/// remote transport only; `SaveLocal` also mirrors the file under
/// `<app_data>/Storage/remote/<kind>/<profile_name>/`.
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
    pub(crate) path: Option<String>,
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
    pub(crate) bridge_keys: HashMap<String, Vec<u8>>,
    /// KeePassRPC session keys (client username → 32-byte SRP-derived key).
    /// Same lifecycle as `bridge_keys`: in-memory only, wiped on close.
    pub(crate) rpc_keys: HashMap<String, Vec<u8>>,
    /// URL matching mode for the bridge/RPC Domain tier, driven by the config
    /// `rpc.matchByRegistrableDomain` (applied via `set_config`): `false`
    /// matches strict host/subdomain, `true` matches registrable domain (PSL,
    /// so `account.aliyun.com` and `passport.aliyun.com` both match under
    /// `aliyun.com`, mirroring KeePassRPC).
    pub(crate) match_registrable_domain: bool,
    /// Window title captured by a global-hotkey multi-match request; consumed
    /// by `autotype_pick` when the user chooses an entry from the picker.
    pub(crate) pending_autotype_window: Option<String>,
}
