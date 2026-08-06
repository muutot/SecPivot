//! Serde DTOs that cross the Tauri IPC boundary (camelCase on the wire),
//! extracted from `vault.rs`. Pure type definitions plus the tri-state icon
//! deserializer; no vault-session logic.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

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
    /// `set_group_expanded` so the tree restores its open state across
    /// sessions.
    pub is_expanded: bool,
    /// KDBX `CustomData` map items, sorted by key. Read-only — SecPivot never
    /// writes these, they must survive edits and saves untouched.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub custom_data: Vec<CustomDataEntry>,
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
    /// Database custom icons (favicons) that live in the KDBX Meta section,
    /// keyed by custom-icon UUID; values are `data:` URLs for direct display.
    /// Only present when the database carries at least one custom icon.
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub custom_icons: HashMap<String, String>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInput {
    pub parent_uuid: Option<String>,
    pub name: String,
    /// Built-in KeePass icon index; absent = default icon.
    #[serde(default)]
    pub icon: Option<u32>,
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
