//! Serde DTOs that cross the Tauri IPC boundary (camelCase on the wire),
//! extracted from `vault.rs`. Pure type definitions plus the tri-state icon
//! deserializer; no vault-session logic.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// One Auto-Type window association (KeePass `Association`): when the focused
/// window matches `window` (with `*` wildcards), `sequence` is used.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoTypeAssociationDto {
    pub window: String,
    pub sequence: String,
}

/// Entry-level Auto-Type settings, mirrored from KDBX `Entry/AutoType`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryAutoTypeConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_sequence: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub associations: Vec<AutoTypeAssociationDto>,
}

/// Group-level Auto-Type settings (inherited by descendants).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupAutoTypeConfig {
    /// `None` = inherit (KeePass group default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_sequence: Option<String>,
}

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
    /// UUID of the referenced database custom icon (a favicon stored in the
    /// KDBX `Meta/CustomIcons` section); the image bytes travel once in
    /// `VaultState::custom_icons`. Mutually exclusive with `icon`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_icon: Option<String>,
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
    /// KeePass `OverrideURL`: honored for matching only (bridge/RPC/auto-type),
    /// never surfaced as the display URL. Editable via `update_entry_flags`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub override_url: Option<String>,
    /// KeePass `ForegroundColor` (`#RRGGBB`); editable via `update_entry_flags`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub foreground_color: Option<String>,
    pub favorite: bool,
    /// KeePass per-entry password-quality check flag. When false, the entry is
    /// excluded from the security report's weak-password findings.
    pub quality_check: bool,
    /// KDBX `CustomData` map items, sorted by key. Read-only — SecPivot never
    /// writes these, they must survive edits and saves untouched.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom_data: Vec<CustomDataEntry>,
    pub custom_fields: Vec<CustomField>,
    pub attachments: Vec<AttachmentInfo>,
    /// Entry-level Auto-Type config; absent when the entry has none stored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autotype: Option<EntryAutoTypeConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomField {
    pub name: String,
    pub value: String,
    /// Whether the value is a KDBX protected string (masked in the UI,
    /// never part of the `VaultEntry` snapshot value — it is resolved on
    /// demand via `get_custom_field_value`).
    #[serde(default)]
    pub protected: bool,
}

/// One item of a KDBX `CustomData` map (entry, group, or database-meta
/// level). SecPivot never writes these — they are plugin metadata written by
/// other KeePass clients — but they must round-trip intact through edits and
/// saves, so they are exposed read-only.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomDataEntry {
    pub key: String,
    /// String value; absent when the item holds binary data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    /// Base64-encoded binary value; present only for binary items.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
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
    /// UUID of the database custom icon used by this group, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_icon: Option<String>,
    pub is_recycle_bin: bool,
    /// KeePass group option: whether entries **of this group** participate in
    /// search. Per-group (KeePass semantics) — it excludes this group's own
    /// entries but not its descendants, which each carry their own flag.
    /// `None` in the KDBX means enabled (default).
    pub enable_searching: bool,
    /// KeePass group notes (`Group.notes`). Read-only for now — SecPivot
    /// surfaces them but does not edit them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    /// KeePass group tags (`Group.tags`), comma-separated for display parity
    /// with entry tags. Read-only for now.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// KeePass group expand flag (`Group.is_expanded`). Persisted by
    /// `set_group_expanded` / `set_groups_expanded` so the tree restores its
    /// open state across sessions.
    pub is_expanded: bool,
    /// KDBX `CustomData` map items, sorted by key. Read-only — SecPivot never
    /// writes these, they must survive edits and saves untouched.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom_data: Vec<CustomDataEntry>,
    /// Group-level Auto-Type config; absent when the group stores none
    /// (inherits from ancestors / global default).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autotype: Option<GroupAutoTypeConfig>,
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
    /// Monotonic session edit revision; incremented by every mutation and
    /// exposed so the renderer can order/merge snapshots and later apply
    /// delta results.
    pub revision: u64,
    /// Database custom icons (favicons) that live in the KDBX Meta section,
    /// keyed by custom-icon UUID; values are `data:` URLs for direct display.
    /// `Some(map)` is authoritative (open/create/full refresh, including an
    /// empty map); `None` means "unchanged" so mutation snapshots can omit
    /// the image payload while the renderer keeps its icon cache.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_icons: Option<HashMap<String, String>>,
    /// Database-meta-level KDBX `CustomData` map items, sorted by key.
    /// Read-only — SecPivot never writes these, they must survive saves
    /// untouched.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub meta_custom_data: Vec<CustomDataEntry>,
    /// KDBX `Meta.DatabaseName`. Editable via `update_db_meta`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_name: Option<String>,
    /// KDBX `Meta.DatabaseDescription`. Editable via `update_db_meta`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_description: Option<String>,
}

/// Result of an open/create command: the registry id of the newly active
/// session plus its authoritative `VaultState`. The renderer keeps the id so
/// later commands can address this session (tabs); commands that omit the id
/// default to the active session.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultOpenResult {
    pub session_id: String,
    pub state: VaultState,
}

/// One open session shown by the tab bar (active first, then parked in park
/// order). `dirty` is the server-side flag, so parked tabs keep their marker
/// after switching.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub session_id: String,
    pub file_name: String,
    pub path: String,
    pub dirty: bool,
}

/// In-memory attachment preview: `kind` is `"text"` (utf8 content in `data`),
/// `"image"` (`data:` URL) or `"binary"` (no content). `truncated` marks
/// previews capped at the 2 MiB preview limit; the attachment itself is never
/// written to disk by previewing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentPreview {
    pub kind: String,
    pub data: String,
    pub size: usize,
    pub truncated: bool,
}

/// Reference to an attachment extracted into the controlled temp directory
/// for external viewing; `token` removes the file on discard/close.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TempAttachmentRef {
    pub token: String,
    pub path: String,
    pub name: String,
}

/// Lightweight mutation result for small state changes that do not need a
/// rebuilt tree. The renderer applies the delta locally against its cached
/// `VaultState`; `revision` lets it order/merge results and detect stale
/// responses.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MutationDelta {
    Favorite {
        revision: u64,
        uuid: String,
        favorite: bool,
    },
    GroupsExpanded {
        revision: u64,
        groups: HashMap<String, bool>,
    },
}

/// One entry offered by the global-hotkey picker when several entries match
/// the focused window.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutotypeCandidate {
    pub uuid: String,
    pub title: String,
    pub username: String,
}

/// Read-only view of the open database's storage settings.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSettings {
    /// `"Aes"` / `"Argon2"` / `"Argon2id"`.
    pub kdf: String,
    /// `"Aes256"` / `"Twofish"` / `"ChaCha20"`.
    pub cipher: String,
    /// `"None"` / `"Gzip"`.
    pub compression: String,
    /// KeePass `Meta.history_max_items`; `None` = unlimited/default.
    pub history_max_items: Option<i64>,
    /// KeePass `Meta.history_max_size`; `None` = unlimited/default.
    pub history_max_size: Option<i64>,
    /// KeePass recycle-bin flag; `None` in KDBX reads as enabled.
    pub recycle_bin_enabled: bool,
    /// UUID of the KDBX entry-templates group; `None` = not configured.
    pub entry_templates_group: Option<String>,
}

/// Partial write for database-level settings. An absent field keeps the
/// current value; an explicit `null` resets it to the KeePass default
/// (`historyMaxItems` cleared = default cap, `recycleBinEnabled` cleared =
/// enabled).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseSettingsPatch {
    /// `"Aes"` / `"Argon2"` / `"Argon2id"`; present value re-encrypts.
    #[serde(default)]
    pub kdf: Option<String>,
    /// `"Aes256"` / `"Twofish"` / `"ChaCha20"`; present value re-encrypts.
    #[serde(default)]
    pub cipher: Option<String>,
    /// `"None"` / `"Gzip"`; present value re-encrypts.
    #[serde(default)]
    pub compression: Option<String>,
    #[serde(default)]
    pub history_max_items: Option<Option<i64>>,
    #[serde(default)]
    pub history_max_size: Option<Option<i64>>,
    #[serde(default)]
    pub recycle_bin_enabled: Option<Option<bool>>,
    /// Present string sets the templates-group UUID; `null` clears it.
    #[serde(default)]
    pub entry_templates_group: Option<Option<String>>,
}

/// Deserialize `EntryInput.icon` tri-state: a number sets the built-in
/// index, JSON `null` explicitly resets to the default icon, and an absent
/// field keeps the entry's current icon. Plain `Option<Option<u32>>` serde
/// would collapse `null` into "absent", silently wiping favicon icons on
/// content-only edits.
pub(crate) fn de_entry_icon<'de, D>(deserializer: D) -> Result<Option<Option<u32>>, D::Error>
where
    D: Deserializer<'de>,
{
    // `serde_json::Value` (not `Option<Value>`) so a present JSON `null`
    // stays distinct from an absent field: `Option<Value>` collapses `null`
    // into `None`, silently turning an explicit "reset to default" into
    // "keep the current icon".
    let value = serde_json::Value::deserialize(deserializer)?;
    match value {
        serde_json::Value::Null => Ok(Some(None)),
        serde_json::Value::Number(n) => {
            let icon = n
                .as_u64()
                .and_then(|v| u32::try_from(v).ok())
                .ok_or_else(|| serde::de::Error::custom("icon must be a u32 index"))?;
            Ok(Some(Some(icon)))
        }
        _ => Err(serde::de::Error::custom("icon must be a number or null")),
    }
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
    /// Built-in KeePass icon index; `null` resets to the default icon, and an
    /// absent value keeps the entry's current icon (custom favicons survive
    /// content-only edits).
    #[serde(default, deserialize_with = "de_entry_icon")]
    pub icon: Option<Option<u32>>,
    /// `#RRGGBB` background color; empty/absent clears it.
    #[serde(default)]
    pub color: Option<String>,
    /// Comma-separated tags; absent keeps the current tags, an empty string
    /// clears them.
    #[serde(default)]
    pub tags: Option<String>,
    #[serde(default)]
    pub custom_fields: Vec<CustomField>,
    #[serde(default)]
    pub attachments: Vec<AttachmentInput>,
}

/// Partial entry update applied to several entries at once (batch editing).
/// An absent field leaves the entry untouched; the `clear_*` flags explicitly
/// clear optional values. Passwords never enter the log or config.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryPatch {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// TOTP seed to set; an empty string clears the existing seed.
    #[serde(default)]
    pub totp: Option<String>,
    /// New ISO-8601 expiry; an empty string clears the existing expiry.
    #[serde(default)]
    pub expires: Option<String>,
    #[serde(default)]
    pub clear_expires: bool,
    /// Built-in KeePass icon index.
    #[serde(default)]
    pub icon: Option<u32>,
    #[serde(default)]
    pub clear_icon: bool,
    /// `#RRGGBB` background color; an empty string clears the existing color.
    #[serde(default)]
    pub color: Option<String>,
    #[serde(default)]
    pub clear_color: bool,
    /// Comma-separated tags to set; an empty string clears all tags.
    #[serde(default)]
    pub tags: Option<String>,
}

/// Full replacement for an entry's Auto-Type config. `enabled` defaults to
/// true (KeePass default); an empty `defaultSequence` clears it; `associations`
/// replaces the stored window-association list.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryAutoTypeInput {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub default_sequence: Option<String>,
    #[serde(default)]
    pub associations: Vec<AutoTypeAssociationDto>,
}

/// Partial update for a group's Auto-Type config. Absent fields keep the
/// current value; an empty `defaultSequence` clears the group default.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupAutoTypeInput {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub default_sequence: Option<String>,
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

fn default_true() -> bool {
    true
}

/// A computed one-time password for display. `kind` is `"totp"` / `"hotp"` /
/// `"steam"`. Time-based kinds carry a live `valid_for`/`period` countdown;
/// HOTP has neither (0/0) and instead reports the current `counter`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotpCode {
    pub code: String,
    pub kind: String,
    /// Seconds until this code expires (1..=period; 0 for HOTP).
    pub valid_for: u64,
    /// Total period in seconds (usually 30; 0 for HOTP).
    pub period: u64,
    /// The moving factor that produced this code (HOTP only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub counter: Option<u64>,
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
    /// Whether the snapshot carried a TOTP seed. The seed itself never leaves
    /// the backend, mirroring `VaultEntry.has_totp`.
    pub has_totp: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<u32>,
    /// UUID of the referenced database custom icon, if the snapshot used one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_icon: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    /// `#RRGGBB` background color the snapshot carried, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub favorite: bool,
    pub quality_check: bool,
    /// KDBX `CustomData` map items of the snapshot, sorted by key. Read-only.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom_data: Vec<CustomDataEntry>,
    pub custom_fields: Vec<CustomField>,
    pub attachments: Vec<AttachmentInfo>,
}

/// Byte-size breakdown of everything an entry holds: field text, attachments,
/// and its historical snapshots (fields + attachments of every version).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryStorage {
    /// Bytes of the entry's own field values (including the password).
    pub fields: usize,
    /// Bytes of the entry's own attachment data.
    pub attachments: usize,
    /// Bytes of all historical snapshots (their fields + attachments).
    pub history: usize,
    /// `fields + attachments + history`.
    pub total: usize,
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

/// One favicon job: a URL host plus every entry UUID that references it.
#[derive(Debug, Clone)]
pub struct FaviconJob {
    pub host: String,
    pub entry_uuids: Vec<String>,
}

/// A successfully fetched favicon for one host, ready to be written back.
#[derive(Debug, Clone)]
pub struct FaviconFetch {
    pub host: String,
    pub bytes: Vec<u8>,
}

/// Result of a "Download Favicons" run, surfaced to the renderer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaviconReport {
    /// Number of distinct hosts whose entries were examined.
    pub attempted: usize,
    /// Number of favicons actually fetched and stored.
    pub downloaded: usize,
}

/// Progress of a "Download Favicons" run, emitted after each host finishes.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FaviconProgress {
    /// Hosts finished so far.
    pub done: usize,
    /// Distinct hosts to process.
    pub total: usize,
}
