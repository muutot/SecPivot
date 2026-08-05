//! Snapshot building + entry-field mutation for the vault session, extracted
//! from `vault.rs`. Reads `keepass::Database`/`EntryRef` into the IPC DTOs and
//! applies `EntryInput`/`EntryPatch` mutations back onto `EntryMut`.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use chrono::NaiveDateTime;
use keepass::db::{
    AttachmentRef, Color, Entry, EntryMut, EntryRef, GroupRef, History, Icon, Value,
};
use keepass::Database;
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

use crate::vault::dto::{
    AttachmentInfo, AttachmentInput, CustomField, EntryInput, EntryPatch, VaultEntry, VaultGroup,
};
use crate::vault::{
    entry_has_otp, FIELD_FAVORITE, FIELD_FAVORITE_TRUE, FIELD_NOTES, FIELD_OTP, FIELD_PASSWORD,
    FIELD_TITLE, FIELD_URL, FIELD_USERNAME, RESERVED_FIELDS, ROOT_GROUP_NAME, ROOT_GROUP_UUID,
};

/// Collect every entry URL host under `group` into `map`, keyed by host.
pub(crate) fn collect_favicon_hosts(group: &GroupRef<'_>, map: &mut BTreeMap<String, Vec<String>>) {
    for entry in group.entries() {
        if let Some(host) = extract_host(entry.get(FIELD_URL).unwrap_or_default()) {
            map.entry(host)
                .or_default()
                .push(entry.id().uuid().to_string());
        }
    }
    for child in group.groups() {
        collect_favicon_hosts(&child, map);
    }
}

/// Best-effort host extraction for favicon downloads: accepts `http(s)://`
/// URLs and scheme-less domains, returning `host[:port]`; `None` otherwise.
pub(crate) fn extract_host(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = if trimmed.contains("://") {
        trimmed.to_owned()
    } else {
        format!("https://{trimmed}")
    };
    let parsed = url::Url::parse(&candidate).ok()?;
    match parsed.scheme() {
        "http" | "https" => parsed.host_str().map(str::to_owned),
        _ => None,
    }
}

/// Encode a custom-icon's raw image bytes as a `data:` URL with a guessed
/// media type, so the renderer can drop it straight into an `<img>`.
pub(crate) fn icon_to_data_url(bytes: &[u8]) -> String {
    let mime = if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00])
        || bytes.starts_with(&[0x00, 0x00, 0x02, 0x00])
    {
        "image/x-icon"
    } else if bytes.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.starts_with(b"BM") {
        "image/bmp"
    } else if bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml") {
        "image/svg+xml"
    } else {
        "image/png"
    };
    format!("data:{mime};base64,{}", BASE64.encode(bytes))
}

pub(crate) fn build_group_tree(db: &Database) -> VaultGroup {
    let root_ref = db.root();
    VaultGroup {
        uuid: ROOT_GROUP_UUID.to_owned(),
        parent_uuid: None,
        name: ROOT_GROUP_NAME.to_owned(),
        icon: None,
        custom_icon: None,
        is_recycle_bin: false,
        children: root_ref
            .groups()
            .filter_map(|g| build_group(&g, ROOT_GROUP_UUID, db.meta.recyclebin_uuid))
            .collect(),
        entries: root_ref
            .entries()
            .map(|e| build_entry(&e, ROOT_GROUP_UUID))
            .collect(),
    }
}

/// Build a single group's visible tree node. Every group stays visible —
/// empty or not — so the user can navigate to it and populate it. The one
/// exception is the recycle bin, which is hidden while it holds nothing.
fn build_group(
    group: &GroupRef<'_>,
    parent_uuid: &str,
    recyclebin_uuid: Option<Uuid>,
) -> Option<VaultGroup> {
    let uuid = group.id().uuid().to_string();
    let is_bin = Some(group.id().uuid()) == recyclebin_uuid;
    let children = group
        .groups()
        .filter_map(|g| build_group(&g, &uuid, recyclebin_uuid))
        .collect::<Vec<_>>();
    let entries = group
        .entries()
        .map(|e| build_entry(&e, &uuid))
        .collect::<Vec<_>>();
    if entries.is_empty() && children.is_empty() && is_bin {
        return None;
    }
    Some(VaultGroup {
        uuid: uuid.clone(),
        parent_uuid: Some(parent_uuid.to_owned()),
        name: group.name.clone(),
        icon: match group.icon() {
            Some(Icon::BuiltIn(id)) => Some(*id as u32),
            _ => None,
        },
        custom_icon: match group.icon() {
            Some(Icon::Custom(id)) => Some(id.uuid().to_string()),
            _ => None,
        },
        is_recycle_bin: is_bin,
        children,
        entries,
    })
}

pub(crate) fn build_entry(entry: &EntryRef<'_>, group_uuid: &str) -> VaultEntry {
    VaultEntry {
        uuid: entry.id().uuid().to_string(),
        group_uuid: group_uuid.to_owned(),
        title: entry.get_title().unwrap_or_default().to_owned(),
        username: entry.get(FIELD_USERNAME).unwrap_or_default().to_owned(),
        url: entry.get(FIELD_URL).unwrap_or_default().to_owned(),
        notes: entry.get(FIELD_NOTES).unwrap_or_default().to_owned(),
        has_totp: entry_has_otp(entry),
        icon: match entry.icon() {
            Some(Icon::BuiltIn(id)) => Some(*id as u32),
            _ => None,
        },
        custom_icon: match entry.icon() {
            Some(Icon::Custom(id)) => Some(id.uuid().to_string()),
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
        expires: match entry.times.expires {
            Some(true) => entry.times.expiry.map(format_iso),
            _ => None,
        },
        expired: entry.times.expires == Some(true)
            && entry
                .times
                .expiry
                .is_some_and(|expiry| expiry < chrono::Utc::now().naive_utc()),
        custom_fields: {
            let mut fields: Vec<CustomField> = entry
                .fields
                .iter()
                .filter(|(name, _)| !name.is_empty() && !RESERVED_FIELDS.contains(&name.as_str()))
                .map(|(name, value)| {
                    let protected = value.is_protected();
                    CustomField {
                        name: name.clone(),
                        // Protected values never leave the session in the
                        // snapshot; they are resolved on demand.
                        value: if protected {
                            String::new()
                        } else {
                            value.get().clone()
                        },
                        protected,
                    }
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

pub(crate) fn format_iso(time: NaiveDateTime) -> String {
    time.format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// Read an attachment's byte size, tolerating a dangling reference.
///
/// The `keepass` crate stores attachments in a database-level store shared by
/// an entry and its history snapshots. Removing an attachment from the current
/// entry drops it from the store, so older snapshots can still carry the
/// name→ID mapping while the data is gone; deref panics in that case. Callers
/// that read historical snapshots use this to skip such attachments.
pub(crate) fn attachment_size(attachment: &AttachmentRef<'_>) -> Option<usize> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| attachment.data.get().len())).ok()
}

/// Maximum number of historical snapshots kept per entry (KeePass default).
const MAX_HISTORY_VERSIONS: usize = 10;

/// Drop the oldest snapshots until the history fits within the cap. The
/// crate exposes no mutable access to the history, so it is rebuilt with the
/// newest `MAX_HISTORY_VERSIONS` snapshots preserved in their original order.
pub(crate) fn trim_entry_history(entry: &mut Entry) {
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
pub(crate) fn escape_csv(value: &str) -> String {
    if value.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// Mirror of the frontend `estimateEntropy` (`src/lib/utils/password.ts`).
pub(crate) fn estimate_entropy(password: &str) -> u32 {
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

pub(crate) fn now_iso() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

pub(crate) fn write_fields(entry: &mut EntryMut<'_>, input: &EntryInput) {
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
    // Icon: a built-in index; `null` resets to the default icon, and an
    // absent value keeps the current icon so custom favicon icons survive
    // content-only edits (e.g. `update_entry` with unchanged icon).
    match input.icon {
        Some(Some(icon_id)) => entry.set_icon_builtin(icon_id as usize),
        Some(None) => entry.set_icon_none(),
        None => {}
    }
    // Background color tags the entry row; foreground is left unset.
    entry.background_color = parse_color(input.color.as_deref());
    entry.foreground_color = None;
}

/// Apply a partial batch-edit patch to an entry. Absent fields are skipped;
/// the `clear_*` flags and empty-string values clear optional attributes.
pub(crate) fn apply_patch_fields(entry: &mut EntryMut<'_>, patch: &EntryPatch) {
    if let Some(title) = &patch.title {
        entry.set(FIELD_TITLE, Value::unprotected(title.clone()));
    }
    if let Some(username) = &patch.username {
        entry.set(FIELD_USERNAME, Value::unprotected(username.clone()));
    }
    if let Some(password) = &patch.password {
        entry.set(FIELD_PASSWORD, Value::protected(password.clone()));
    }
    if let Some(url) = &patch.url {
        entry.set(FIELD_URL, Value::unprotected(url.clone()));
    }
    if let Some(notes) = &patch.notes {
        entry.set(FIELD_NOTES, Value::unprotected(notes.clone()));
    }
    if let Some(seed) = &patch.totp {
        if seed.trim().is_empty() {
            entry.fields.remove(FIELD_OTP);
        } else {
            entry.set(FIELD_OTP, Value::unprotected(seed.trim().to_owned()));
        }
    }
    if patch.clear_expires {
        entry.times.expiry = None;
        entry.times.expires = Some(false);
    } else if let Some(expires) = &patch.expires {
        match parse_expiry(Some(expires)) {
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
    if patch.clear_icon {
        entry.set_icon_none();
    } else if let Some(icon_id) = patch.icon {
        entry.set_icon_builtin(icon_id as usize);
    }
    if patch.clear_color {
        entry.background_color = None;
    } else if let Some(color) = &patch.color {
        entry.background_color = parse_color(Some(color));
        entry.foreground_color = None;
    }
}

/// Parse a `#RRGGBB` color string; `None` for empty/absent or invalid input.
pub(crate) fn parse_color(value: Option<&str>) -> Option<Color> {
    value?.trim().parse().ok()
}

/// Parse an ISO-8601 expiry string into a UTC `NaiveDateTime`. Accepts the
/// frontend's `toISOString()` output (with milliseconds and `Z` suffix) as
/// well as legacy `%Y-%m-%dT%H:%M:%S` values. Returns `None` for empty input;
/// rejects invalid formats.
pub(crate) fn parse_expiry(value: Option<&str>) -> Option<NaiveDateTime> {
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
pub(crate) fn sync_custom_fields(entry: &mut EntryMut<'_>, fields: &[CustomField]) {
    let mut desired = HashMap::new();
    for field in fields {
        let name = field.name.trim().to_owned();
        if name.is_empty() || RESERVED_FIELDS.contains(&name.as_str()) {
            continue;
        }
        desired.insert(name, (field.value.clone(), field.protected));
    }
    let current: Vec<String> = entry.fields.keys().cloned().collect();
    for name in current {
        if !RESERVED_FIELDS.contains(&name.as_str()) && !desired.contains_key(&name) {
            entry.fields.remove(&name);
        }
    }
    for (name, (value, protected)) in desired {
        entry.set(
            name,
            if protected {
                Value::protected(value)
            } else {
                Value::unprotected(value)
            },
        );
    }
}

/// A pre-decoded attachment payload, ready to write.
pub(crate) struct AttachmentPayload {
    name: String,
    data: Vec<u8>,
}

/// Decode all attachment payloads up-front so a bad base64 payload aborts the
/// whole entry mutation before anything is written (no partial commit, no
/// history snapshot pollution, dirty flag stays untouched).
pub(crate) fn decode_attachments(
    input: &[AttachmentInput],
) -> Result<Vec<AttachmentPayload>, String> {
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
pub(crate) fn sync_attachments(
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
