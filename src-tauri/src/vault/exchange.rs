//! KeePass "Data Exchange": whole-entry clipboard transfer.
//!
//! Mirrors the official KeePass 2.x contract (the `Entry → Data Exchange`
//! submenu in KeePass' own main menu) so the entry context menu gains the same
//! three commands:
//!
//! - **Copy Entry (Encrypted)**   — the payload is additionally wrapped with
//!   Windows DPAPI (`CurrentUser` scope, KeePass' `ClipDomainSep` entropy), so
//!   only this user's session can unwrap it.
//! - **Copy Entry (Unencrypted)** — the payload goes on the clipboard as-is.
//!   The KDBX document is *not* encrypted at this layer (KeePass writes a
//!   `PlainXml` payload for exactly this reason), so this variant exposes
//!   plaintext credentials to anything that can read the clipboard and must
//!   travel through the scheduled clipboard wipe like a password copy.
//! - **Paste Entry** — reads the payload back and inserts the entries into the
//!   selected group under **fresh UUIDs** (KeePass `AddEntry(…, bAllowNewUuid)`),
//!   so a paste is a distinct record rather than a sync-identical duplicate.
//!
//! Wire format (framing byte-identical to KeePass' `EntryUtil`):
//!
//! ```text
//! clipboard format : "Entries-F"            (KeePass' own custom format name)
//! payload          : [flags: u32 LE][gzip(kdbx xml document)]
//! flags bit 0      : payload is DPAPI-protected
//! DPAPI entropy    : F8 03 FA 51 87 18 49 5D (KeePass `ClipDomainSep`)
//! ```
//!
//! The gzip body is the KDBX XML document (`<KeePassFile><Root><Group>…`) that
//! KeePass stores inside its `PlainXml` container. KeePass additionally wraps
//! that document in a KDBX 4.1 container (outer header + inner header + SHA-256
//! footer); that container is **not** emitted here, so SecPivot ↔ SecPivot
//! paste works while cross-client paste is not yet interoperable. The XML
//! document, the clipboard format name, the framing and the DPAPI scheme all
//! match KeePass, so the container is the only remaining difference.
//!
//! Known gap versus KeePass: entry *history* is not restored on paste, because
//! the `keepass` crate exposes no API to append a historical entry to an
//! existing entry (`History::add_entry` needs a fully built `Entry`). History
//! items are therefore not written to the payload either. Binary
//! `CustomData` items are skipped for the same reason (the KDBX XML `<Value>`
//! element cannot tell a text value from a base64 one), while text items
//! round-trip.

use std::collections::HashMap;
use std::io::{Read, Write};

use base64::{engine::general_purpose as base64_engine, Engine as _};
use keepass::db::{
    AutoType, AutoTypeAssociation, CustomDataItem, CustomDataValue, Database, EntryId, GroupMut,
    Icon, Value,
};
use quick_xml::se::to_string_with_root;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// KeePass' clipboard format name for a whole-entry package.
pub const CLIP_FORMAT_ENTRIES: &str = "Entries-F";

/// `flags & 1` — the payload is DPAPI-protected.
const FLAG_ENCRYPTED: u32 = 1;

/// KeePass `EntryUtil.ClipDomainSep`, passed as DPAPI optional entropy so a
/// blob written by an unrelated program is never unwrapped by accident.
const CLIP_DOMAIN_SEP: [u8; 8] = [0xF8, 0x03, 0xFA, 0x51, 0x87, 0x18, 0x49, 0x5D];

/// Bound both the clipboard read and the inflate output: any local app can
/// write this format, so an attacker-chosen blob must not be able to make the
/// backend allocate without limit.
const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

/// KeePass stores attachment bytes out of line: the entry keeps a
/// `BinDes-<id>` string field holding the file name and the payload lives in
/// the document's `<Binaries>` list.
const BINARY_FIELD_PREFIX: &str = "BinDes-";

// ---------------------------------------------------------------------------
// KDBX XML document model
//
// Element and attribute names follow the KDBX XML schema — the same one the
// `keepass` crate's own (crate-private) `format::xml_db` serializer emits.
// Optional elements are all defaulted so foreign documents with extra or
// missing elements still parse, matching how KeePass' reader tolerates
// third-party payloads.
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct KeePassFile {
    #[serde(default)]
    pub meta: Meta,
    pub root: Root,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub binaries: Option<Binaries>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Meta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generator: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_icons: Option<CustomIcons>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Root {
    pub group: Group,
}

/// Entries only. KeePass' `WriteEntries` wraps the selected entries in one
/// throwaway group; its `Copy Group` command (2.47+) exchanges a group tree and
/// is not covered here.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Group {
    #[serde(rename = "UUID")]
    pub uuid: String,
    #[serde(default)]
    pub name: String,
    #[serde(rename = "Entry", default)]
    pub entries: Vec<XmlEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XmlEntry {
    #[serde(rename = "UUID")]
    pub uuid: String,
    #[serde(default, rename = "IconID", skip_serializing_if = "Option::is_none")]
    pub icon_id: Option<i64>,
    #[serde(
        default,
        rename = "CustomIconUUID",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_icon_uuid: Option<String>,
    #[serde(
        default,
        rename = "ForegroundColor",
        skip_serializing_if = "Option::is_none"
    )]
    pub foreground_color: Option<String>,
    #[serde(
        default,
        rename = "BackgroundColor",
        skip_serializing_if = "Option::is_none"
    )]
    pub background_color: Option<String>,
    #[serde(
        default,
        rename = "OverrideURL",
        skip_serializing_if = "Option::is_none"
    )]
    pub override_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub times: Option<XmlTimes>,
    #[serde(default, rename = "String")]
    pub string_fields: Vec<XmlStringField>,
    #[serde(default, rename = "AutoType", skip_serializing_if = "Option::is_none")]
    pub auto_type: Option<XmlAutoType>,
    #[serde(
        default,
        rename = "CustomData",
        skip_serializing_if = "Option::is_none"
    )]
    pub custom_data: Option<XmlCustomData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_check: Option<bool>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XmlStringField {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "Value")]
    pub value: XmlStringValue,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct XmlStringValue {
    #[serde(rename = "$value", default)]
    pub value: String,
    /// KeePass spells booleans `True`/`False`, not `true`/`false`.
    #[serde(
        rename = "@Protected",
        default,
        skip_serializing_if = "String::is_empty"
    )]
    pub protected: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XmlAutoType {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<String>,
    #[serde(
        default,
        rename = "DefaultSequence",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_sequence: Option<String>,
    #[serde(rename = "Association", default)]
    pub associations: Vec<XmlAutoTypeAssociation>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XmlAutoTypeAssociation {
    #[serde(default)]
    pub window: String,
    #[serde(default, rename = "KeystrokeSequence")]
    pub keystroke_sequence: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XmlCustomData {
    #[serde(rename = "Item", default)]
    pub items: Vec<XmlCustomDataItem>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XmlCustomDataItem {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XmlTimes {
    #[serde(
        default,
        rename = "CreationTime",
        skip_serializing_if = "Option::is_none"
    )]
    pub creation_time: Option<i64>,
    #[serde(
        default,
        rename = "LastModificationTime",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_modification_time: Option<i64>,
    #[serde(
        default,
        rename = "LastAccessTime",
        skip_serializing_if = "Option::is_none"
    )]
    pub last_access_time: Option<i64>,
    #[serde(
        default,
        rename = "ExpiryTime",
        skip_serializing_if = "Option::is_none"
    )]
    pub expiry_time: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage_count: Option<i64>,
    #[serde(
        default,
        rename = "LocationChanged",
        skip_serializing_if = "Option::is_none"
    )]
    pub location_changed: Option<i64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Binaries {
    #[serde(rename = "Binary", default)]
    pub binaries: Vec<XmlBinary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct XmlBinary {
    #[serde(rename = "$value")]
    pub data: String,
    #[serde(rename = "@ID")]
    pub id: usize,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct CustomIcons {
    #[serde(rename = "Icon", default)]
    pub icons: Vec<XmlCustomIcon>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct XmlCustomIcon {
    #[serde(rename = "UUID")]
    pub uuid: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

// ---------------------------------------------------------------------------
// Database -> KDBX XML document
// ---------------------------------------------------------------------------

/// Serialize the addressed entries into a KDBX XML document for the clipboard.
///
/// Protected fields are written as `Protected="True"` with their **plaintext**
/// value, exactly like KeePass' `PlainXml` payload: the document itself carries
/// no encryption, which is why the caller must route the framed payload through
/// the scheduled clipboard wipe.
pub fn build_entries_document(db: &Database, ids: &[EntryId]) -> Result<Vec<u8>, String> {
    let mut binaries: Vec<XmlBinary> = Vec::new();
    let mut custom_icons: HashMap<String, XmlCustomIcon> = HashMap::new();
    let mut entries = Vec::with_capacity(ids.len());

    for id in ids {
        let entry = db.entry(*id).ok_or_else(|| "条目不存在".to_owned())?;
        entries.push(serialize_entry(&entry, &mut binaries, &mut custom_icons)?);
    }

    let mut icons: Vec<XmlCustomIcon> = custom_icons.into_values().collect();
    icons.sort_by(|a, b| a.uuid.cmp(&b.uuid));

    let file = KeePassFile {
        meta: Meta {
            generator: Some("SecPivot".to_owned()),
            custom_icons: (!icons.is_empty()).then_some(CustomIcons { icons }),
        },
        root: Root {
            group: Group {
                uuid: encode_uuid(Uuid::new_v4()),
                name: String::new(),
                entries,
            },
        },
        binaries: (!binaries.is_empty()).then_some(Binaries { binaries }),
    };

    to_string_with_root("KeePassFile", &file)
        .map(String::into_bytes)
        .map_err(|e| format!("序列化失败: {e}"))
}

fn serialize_entry(
    entry: &keepass::db::EntryRef<'_>,
    binaries: &mut Vec<XmlBinary>,
    custom_icons: &mut HashMap<String, XmlCustomIcon>,
) -> Result<XmlEntry, String> {
    let mut keys: Vec<&String> = entry.fields.keys().collect();
    keys.sort();
    let mut string_fields: Vec<XmlStringField> = keys
        .into_iter()
        .map(|key| {
            let value = &entry.fields[key];
            XmlStringField {
                key: key.clone(),
                value: XmlStringValue {
                    value: value.get().clone(),
                    protected: bool_xml(value.is_protected()),
                },
            }
        })
        .collect();

    // Attachments live in the document's <Binaries> list; each entry keeps an
    // ordered `BinDes-<id>` field holding the file name.
    for (name, attachment) in entry.attachments_named() {
        let id = binaries.len();
        binaries.push(XmlBinary {
            data: base64_engine::STANDARD.encode(attachment.data.get()),
            id,
        });
        string_fields.push(XmlStringField {
            key: format!("{BINARY_FIELD_PREFIX}{id}"),
            value: XmlStringValue {
                value: name.to_owned(),
                protected: bool_xml(attachment.data.is_protected()),
            },
        });
    }

    let (icon_id, custom_icon_uuid) = match entry.icon() {
        Some(Icon::BuiltIn(id)) => (i64::try_from(*id).ok(), None),
        Some(Icon::Custom(id)) => (None, Some(encode_uuid(id.uuid()))),
        None => (None, None),
    };
    if let Some(uuid) = &custom_icon_uuid {
        if let Some(icon) = entry.custom_icon() {
            custom_icons
                .entry(uuid.clone())
                .or_insert_with(|| XmlCustomIcon {
                    uuid: uuid.clone(),
                    data: Some(base64_engine::STANDARD.encode(&icon.data)),
                });
        }
    }

    Ok(XmlEntry {
        uuid: encode_uuid(entry.id().uuid()),
        icon_id,
        custom_icon_uuid,
        foreground_color: entry.foreground_color.as_ref().map(ToString::to_string),
        background_color: entry.background_color.as_ref().map(ToString::to_string),
        override_url: entry.override_url.clone(),
        tags: (!entry.tags.is_empty()).then(|| entry.tags.join("; ")),
        times: Some(XmlTimes {
            creation_time: entry.times.creation.map(to_unix),
            last_modification_time: entry.times.last_modification.map(to_unix),
            last_access_time: entry.times.last_access.map(to_unix),
            expiry_time: entry.times.expiry.map(to_unix),
            expires: entry.times.expires.map(bool_xml),
            usage_count: entry.times.usage_count.map(|count| count as i64),
            location_changed: entry.times.location_changed.map(to_unix),
        }),
        string_fields,
        auto_type: entry.autotype.as_ref().map(xml_auto_type),
        custom_data: xml_custom_data(entry),
        quality_check: Some(entry.quality_check),
    })
}

fn xml_custom_data(entry: &keepass::db::EntryRef<'_>) -> Option<XmlCustomData> {
    let mut keys: Vec<&String> = entry
        .custom_data
        .iter()
        // Binary items are skipped: the KDBX XML <Value> element cannot tell a
        // text value from a base64 one, so carrying them would either corrupt
        // them or silently turn them into text on paste.
        .filter(|(_, item)| matches!(item.value, Some(CustomDataValue::String(_))))
        .map(|(key, _)| key)
        .collect();
    if keys.is_empty() {
        return None;
    }
    keys.sort();
    Some(XmlCustomData {
        items: keys
            .into_iter()
            .map(|key| XmlCustomDataItem {
                key: key.clone(),
                value: match &entry.custom_data[key].value {
                    Some(CustomDataValue::String(value)) => Some(value.clone()),
                    _ => None,
                },
            })
            .collect(),
    })
}

fn xml_auto_type(auto_type: &AutoType) -> XmlAutoType {
    XmlAutoType {
        enabled: Some(bool_xml(auto_type.enabled)),
        default_sequence: auto_type.default_sequence.clone(),
        associations: auto_type
            .associations
            .iter()
            .map(|association| XmlAutoTypeAssociation {
                window: association.window.clone(),
                keystroke_sequence: association.sequence.clone(),
            })
            .collect(),
    }
}

fn bool_xml(value: bool) -> String {
    if value { "True" } else { "False" }.to_owned()
}

fn to_unix(time: chrono::NaiveDateTime) -> i64 {
    time.and_utc().timestamp()
}

fn encode_uuid(uuid: Uuid) -> String {
    base64_engine::STANDARD.encode(uuid.as_bytes())
}

// ---------------------------------------------------------------------------
// Framing: flags + gzip (+ DPAPI)
// ---------------------------------------------------------------------------

/// Frame a KDBX XML document for the clipboard: gzip it, DPAPI-wrap it when
/// `encrypt` is set, and prepend KeePass' `flags` word.
pub fn encode_payload(document: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    encoder
        .write_all(document)
        .map_err(|e| format!("压缩失败: {e}"))?;
    let compressed = encoder.finish().map_err(|e| format!("压缩失败: {e}"))?;

    let (flags, body) = if encrypt {
        (
            FLAG_ENCRYPTED,
            protect(&compressed).map_err(|e| format!("加密失败: {e}"))?,
        )
    } else {
        (0u32, compressed)
    };

    let mut payload = Vec::with_capacity(body.len() + 4);
    payload.extend_from_slice(&flags.to_le_bytes());
    payload.extend_from_slice(&body);
    Ok(payload)
}

/// Reverse of [`encode_payload`]: flags -> optional DPAPI unwrap -> gunzip.
pub fn decode_payload(payload: &[u8]) -> Result<Vec<u8>, String> {
    if payload.len() < 4 {
        return Err("剪贴板数据无效".to_owned());
    }
    if payload.len() > MAX_PAYLOAD_BYTES {
        return Err("剪贴板数据过大".to_owned());
    }
    let flags = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);
    let body = &payload[4..];
    let body = if flags & FLAG_ENCRYPTED != 0 {
        unprotect(body).map_err(|_| "剪贴板数据无法解密".to_owned())?
    } else {
        body.to_vec()
    };
    gunzip(&body)
}

fn gunzip(body: &[u8]) -> Result<Vec<u8>, String> {
    let decoder = flate2::read::GzDecoder::new(body);
    let mut out = Vec::new();
    // Bound the inflate output too: a tiny payload can expand enormously.
    decoder
        .take(MAX_PAYLOAD_BYTES as u64)
        .read_to_end(&mut out)
        .map_err(|_| "剪贴板数据已损坏".to_owned())?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// DPAPI (Windows) — KeePass `CryptoUtil.ProtectData` with ClipDomainSep
// ---------------------------------------------------------------------------

#[cfg(target_os = "windows")]
fn protect(data: &[u8]) -> Result<Vec<u8>, String> {
    crate::platform::dpapi::protect_bytes_with_entropy(data, &CLIP_DOMAIN_SEP)
}

#[cfg(target_os = "windows")]
fn unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    crate::platform::dpapi::unprotect_bytes_with_entropy(data, &CLIP_DOMAIN_SEP)
}

#[cfg(not(target_os = "windows"))]
fn protect(_data: &[u8]) -> Result<Vec<u8>, String> {
    Err("当前平台不支持 DPAPI，请改用「复制条目（未加密）」".to_owned())
}

#[cfg(not(target_os = "windows"))]
fn unprotect(_data: &[u8]) -> Result<Vec<u8>, String> {
    Err("当前平台不支持 DPAPI".to_owned())
}

// ---------------------------------------------------------------------------
// KDBX XML document -> entries
// ---------------------------------------------------------------------------

/// One entry recovered from a clipboard package, with attachment payloads
/// resolved back to `(name, bytes, protected)`.
pub struct PastedEntry {
    pub fields: Vec<(String, String, bool)>,
    pub attachments: Vec<(String, Vec<u8>, bool)>,
    pub auto_type: Option<XmlAutoType>,
    pub custom_data: Vec<XmlCustomDataItem>,
    pub tags: Vec<String>,
    pub quality_check: bool,
    pub icon: Option<i64>,
    pub custom_icon: Option<Vec<u8>>,
    pub foreground_color: Option<String>,
    pub background_color: Option<String>,
    pub override_url: Option<String>,
    pub expiry_time: Option<i64>,
    pub expires: bool,
}

/// Parse a KDBX XML document into entries ready for insertion under fresh
/// UUIDs. Source UUIDs are intentionally dropped.
pub fn parse_entries_document(document: &[u8]) -> Result<Vec<PastedEntry>, String> {
    let file: KeePassFile =
        quick_xml::de::from_reader(document).map_err(|e| format!("剪贴板数据解析失败: {e}"))?;
    let binaries: HashMap<usize, Vec<u8>> = match file.binaries {
        Some(binaries) => {
            let mut map = HashMap::with_capacity(binaries.binaries.len());
            for binary in binaries.binaries {
                let data = base64_engine::STANDARD
                    .decode(binary.data.trim())
                    .map_err(|_| "附件数据无效".to_owned())?;
                map.insert(binary.id, data);
            }
            map
        }
        None => HashMap::new(),
    };
    let custom_icons: HashMap<String, Vec<u8>> = file
        .meta
        .custom_icons
        .map(|icons| {
            icons
                .icons
                .into_iter()
                .filter_map(|icon| {
                    let data = icon.data?;
                    let bytes = base64_engine::STANDARD.decode(data.trim()).ok()?;
                    Some((icon.uuid, bytes))
                })
                .collect()
        })
        .unwrap_or_default();

    file.root
        .group
        .entries
        .iter()
        .map(|entry| parse_entry(entry, &binaries, &custom_icons))
        .collect()
}

fn parse_entry(
    entry: &XmlEntry,
    binaries: &HashMap<usize, Vec<u8>>,
    custom_icons: &HashMap<String, Vec<u8>>,
) -> Result<PastedEntry, String> {
    let mut fields: Vec<(String, String, bool)> = Vec::new();
    let mut attachments: Vec<(String, Vec<u8>, bool)> = Vec::new();

    for field in &entry.string_fields {
        let protected = field.value.protected == "True";
        if let Some(binary_id) = field.key.strip_prefix(BINARY_FIELD_PREFIX) {
            let id: usize = binary_id.parse().map_err(|_| "附件索引无效".to_owned())?;
            let data = binaries
                .get(&id)
                .cloned()
                .ok_or_else(|| "缺少附件数据".to_owned())?;
            attachments.push((field.value.value.clone(), data, protected));
            continue;
        }
        fields.push((field.key.clone(), field.value.value.clone(), protected));
    }

    let times = entry.times.as_ref();
    Ok(PastedEntry {
        fields,
        attachments,
        auto_type: entry.auto_type.clone(),
        custom_data: entry
            .custom_data
            .as_ref()
            .map(|data| data.items.clone())
            .unwrap_or_default(),
        tags: entry
            .tags
            .as_deref()
            .map(|tags| {
                tags.split(';')
                    .map(str::trim)
                    .filter(|tag| !tag.is_empty())
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
        quality_check: entry.quality_check.unwrap_or(true),
        icon: entry.icon_id,
        custom_icon: entry
            .custom_icon_uuid
            .as_ref()
            .and_then(|uuid| custom_icons.get(uuid).cloned()),
        foreground_color: entry.foreground_color.clone(),
        background_color: entry.background_color.clone(),
        override_url: entry.override_url.clone(),
        expiry_time: times.and_then(|times| times.expiry_time),
        expires: times
            .and_then(|times| times.expires.as_deref())
            .map(|expires| expires == "True")
            .unwrap_or(false),
    })
}

/// Insert one parsed entry into `group` under a fresh UUID, restoring fields,
/// attachments, tags, custom data, auto-type, colours, expiry and the icon.
pub fn insert_entry(group: &mut GroupMut<'_>, entry: &PastedEntry) -> Result<Uuid, String> {
    let mut node = group.add_entry();
    for (key, value, protected) in &entry.fields {
        node.set(
            key.clone(),
            if *protected {
                Value::protected(value.clone())
            } else {
                Value::unprotected(value.clone())
            },
        );
    }
    for (name, data, protected) in &entry.attachments {
        node.add_attachment(
            name.clone(),
            if *protected {
                Value::protected(data.clone())
            } else {
                Value::unprotected(data.clone())
            },
        );
    }
    node.tags = entry.tags.clone();
    if !entry.quality_check {
        node.quality_check = false;
    }
    for item in &entry.custom_data {
        node.custom_data.insert(
            item.key.clone(),
            CustomDataItem {
                value: item.value.clone().map(CustomDataValue::String),
                last_modification_time: None,
            },
        );
    }
    if let Some(auto_type) = &entry.auto_type {
        node.autotype = Some(AutoType {
            enabled: auto_type
                .enabled
                .as_deref()
                .map(|enabled| enabled == "True")
                .unwrap_or(false),
            default_sequence: auto_type.default_sequence.clone(),
            data_transfer_obfuscation: Default::default(),
            associations: auto_type
                .associations
                .iter()
                .map(|association| AutoTypeAssociation {
                    window: association.window.clone(),
                    sequence: association.keystroke_sequence.clone(),
                })
                .collect(),
        });
    }
    if entry.expires {
        node.times.expires = Some(true);
        node.times.expiry = entry
            .expiry_time
            .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
            .map(|value| value.naive_utc());
    }
    if let Some(color) = entry
        .background_color
        .as_deref()
        .and_then(|value| value.parse().ok())
    {
        node.background_color = Some(color);
    }
    if let Some(color) = entry
        .foreground_color
        .as_deref()
        .and_then(|value| value.parse().ok())
    {
        node.foreground_color = Some(color);
    }
    if let Some(url) = &entry.override_url {
        node.override_url = Some(url.clone());
    }
    if let Some(data) = &entry.custom_icon {
        // A pasted custom icon becomes a fresh icon in the target database, so
        // no cross-database icon id has to be remapped.
        let _ = node.set_icon_custom_new(data.clone());
    } else if let Some(icon) = entry.icon {
        node.set_icon_builtin(icon as usize);
    }
    Ok(node.as_ref().id().uuid())
}

#[cfg(test)]
mod tests {
    use super::*;
    use keepass::db::fields;

    /// Source database with one entry that exercises every carried facet:
    /// protected + unprotected fields, an attachment, tags, custom data,
    /// auto-type, a colour, an expiry and a built-in icon.
    fn sample_db() -> (Database, EntryId) {
        let mut db = Database::new();
        let id = {
            let mut group = db.root_mut();
            let mut entry = group.add_entry();
            entry.set_unprotected(fields::TITLE, "GitHub");
            entry.set_unprotected(fields::USERNAME, "octocat");
            entry.set_protected(fields::PASSWORD, "s3cret");
            entry.set_unprotected(fields::URL, "https://github.com");
            entry.set_unprotected("KDBX Custom Field", "custom-value");
            entry.tags = vec!["work".to_owned(), "mail".to_owned()];
            entry.override_url = Some("https://github.com/*".to_owned());
            entry.background_color = Some("#336699".parse().expect("color"));
            entry.custom_data.insert(
                "plugin".to_owned(),
                CustomDataItem {
                    value: Some(CustomDataValue::String("metadata".to_owned())),
                    last_modification_time: None,
                },
            );
            entry.add_attachment("notes.txt", Value::unprotected(b"hello".to_vec()));
            entry.set_icon_builtin(42);
            entry.times.expires = Some(true);
            entry.times.expiry =
                chrono::DateTime::from_timestamp(1_800_000_000, 0).map(|value| value.naive_utc());
            entry.as_ref().id()
        };
        (db, id)
    }

    #[test]
    fn document_round_trips_every_carried_facet() {
        let (db, id) = sample_db();
        let document = build_entries_document(&db, &[id]).expect("document");
        let text = String::from_utf8(document.clone()).expect("utf8");

        // KDBX XML spellings, not serde defaults.
        assert!(text.starts_with("<KeePassFile>"), "{text}");
        assert!(text.contains("<Meta>"), "{text}");
        assert!(text.contains("<Root>"), "{text}");
        assert!(text.contains("<Key>Title</Key>"), "{text}");
        assert!(text.contains("Protected=\"True\""), "{text}");
        assert!(text.contains("<Tags>work; mail</Tags>"), "{text}");
        assert!(text.contains("<IconID>42</IconID>"), "{text}");
        assert!(
            text.contains("<BackgroundColor>#336699</BackgroundColor>"),
            "{text}"
        );
        assert!(text.contains("<Key>BinDes-0</Key>"), "{text}");
        assert!(
            text.contains("<OverrideURL>https://github.com/*</OverrideURL>"),
            "{text}"
        );

        let parsed = parse_entries_document(&document).expect("parse");
        assert_eq!(parsed.len(), 1);
        let entry = &parsed[0];
        let field = |key: &str| {
            entry
                .fields
                .iter()
                .find(|(name, _, _)| name == key)
                .map(|(_, value, protected)| (value.clone(), *protected))
        };
        assert_eq!(field("Title"), Some(("GitHub".to_owned(), false)));
        assert_eq!(field("UserName"), Some(("octocat".to_owned(), false)));
        // The password survives as a protected field, plaintext in the document
        // exactly like KeePass' PlainXml payload.
        assert_eq!(field("Password"), Some(("s3cret".to_owned(), true)));
        assert_eq!(
            field("KDBX Custom Field"),
            Some(("custom-value".to_owned(), false))
        );
        assert_eq!(entry.tags, vec!["work".to_owned(), "mail".to_owned()]);
        assert_eq!(entry.icon, Some(42));
        assert_eq!(entry.override_url.as_deref(), Some("https://github.com/*"));
        assert_eq!(entry.background_color.as_deref(), Some("#336699"));
        assert!(entry.expires);
        assert_eq!(entry.custom_data.len(), 1);
        assert_eq!(entry.custom_data[0].key, "plugin");
        assert_eq!(entry.attachments.len(), 1);
        assert_eq!(entry.attachments[0].0, "notes.txt");
        assert_eq!(entry.attachments[0].1, b"hello".to_vec());
    }

    #[test]
    fn paste_inserts_under_a_fresh_uuid_and_keeps_protection() {
        let (db, id) = sample_db();
        let document = build_entries_document(&db, &[id]).expect("document");
        let parsed = parse_entries_document(&document).expect("parse");

        let mut target = Database::new();
        let uuid = {
            let mut group = target.root_mut();
            insert_entry(&mut group, &parsed[0]).expect("insert")
        };

        // A paste must not reuse the source identity (KeePass bAllowNewUuid).
        assert_ne!(uuid, id.uuid());
        let entry = target.entry(EntryId::from_uuid(uuid)).expect("entry");
        assert_eq!(entry.get_title(), Some("GitHub"));
        assert_eq!(entry.get(fields::USERNAME), Some("octocat"));
        assert_eq!(entry.get(fields::PASSWORD), Some("s3cret"));
        assert!(
            entry.fields[fields::PASSWORD].is_protected(),
            "password must stay protected"
        );
        assert_eq!(entry.tags, vec!["work".to_owned(), "mail".to_owned()]);
        assert_eq!(entry.attachments_named().count(), 1);
        let (name, data) = entry.attachments_named().next().expect("attachment");
        assert_eq!(name, "notes.txt");
        assert_eq!(data.data.get(), b"hello".as_slice());
        assert_eq!(
            entry.background_color.as_ref().map(ToString::to_string),
            Some("#336699".to_owned())
        );
    }

    #[test]
    fn payload_framing_matches_the_keepass_layout() {
        let (db, id) = sample_db();
        let document = build_entries_document(&db, &[id]).expect("document");

        // Unencrypted: flags word 0, then a gzip stream.
        let payload = encode_payload(&document, false).expect("encode");
        assert_eq!(&payload[..4], &[0, 0, 0, 0], "flags must precede the body");
        assert_eq!(&payload[4..6], &[0x1f, 0x8b], "body must be a gzip stream");
        assert_eq!(decode_payload(&payload).expect("decode"), document);

        // Encrypted (Windows only, and only for this user): flags bit 0 set and
        // the body is no longer a gzip stream.
        #[cfg(target_os = "windows")]
        {
            let payload = encode_payload(&document, true).expect("encode");
            assert_eq!(&payload[..4], &[1, 0, 0, 0], "encrypted flag must be set");
            assert_ne!(&payload[4..6], &[0x1f, 0x8b], "body must be DPAPI-wrapped");
            assert_eq!(decode_payload(&payload).expect("decode"), document);
        }
    }

    #[test]
    fn payload_rejects_truncated_and_flagless_input() {
        assert!(decode_payload(&[0, 0, 0]).is_err(), "short payload");
        assert!(
            decode_payload(&[1, 0, 0, 0, 0x1f, 0x8b]).is_err(),
            "corrupt body"
        );
    }

    #[test]
    fn encrypted_flag_without_dpapi_support_never_falls_back_to_plaintext() {
        // Guards the security property: a payload that claims DPAPI protection
        // must never be silently accepted as unencrypted.
        #[cfg(not(target_os = "windows"))]
        {
            let payload = encode_payload(b"<KeePassFile/>", true);
            assert!(payload.is_err(), "must fail closed without DPAPI");
        }
    }

    #[test]
    fn foreign_documents_with_missing_elements_still_parse() {
        // A payload from another client may omit <Meta>, <Times> and <QualityCheck>.
        let document = concat!(
            r#"<KeePassFile><Root><Group><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID><Name>x</Name>"#,
            r#"<Entry><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID>"#,
            r#"<String><Key>Title</Key><Value>Only</Value></String></Entry>"#,
            r#"</Group></Root></KeePassFile>"#
        );
        let parsed = parse_entries_document(document.as_bytes()).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].fields.len(), 1);
        assert!(parsed[0].tags.is_empty());
        assert!(parsed[0].quality_check, "absent means enabled");
        assert!(!parsed[0].expires);
    }
}
