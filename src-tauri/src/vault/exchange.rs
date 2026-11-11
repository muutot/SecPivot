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
//! document uses SecPivot's legacy plaintext `Protected` marker; official
//! PlainXml uses `ProtectInMemory`. Container, timestamps and other payload
//! details still need independent cross-client validation.
//!
//! Known gap versus KeePass: entry *history* is not restored on paste, because
//! the `keepass` crate exposes no API to append a historical entry to an
//! existing entry (`History::add_entry` needs a fully built `Entry`). History
//! items are therefore not written to the payload either. CustomData values
//! retain their native kinds through SecPivot's internal `SecPivotType`
//! extension; this does not establish official PlainXml interoperability.

use std::collections::HashMap;
use std::io::{Read, Write};

use base64::{engine::general_purpose as base64_engine, Engine as _};
use keepass::db::{
    AutoType, AutoTypeAssociation, CustomDataItem, CustomDataValue, DataTransferObfuscation,
    Database, EntryId, GroupMut, Icon, Value,
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
/// DPAPI itself only exists on Windows, so is the entropy.
#[cfg(target_os = "windows")]
const CLIP_DOMAIN_SEP: [u8; 8] = [0xF8, 0x03, 0xFA, 0x51, 0x87, 0x18, 0x49, 0x5D];

/// Bound exchange documents, complete frames and owned binary copies: any local app can
/// write this format, so an attacker-chosen blob must not be able to make the
/// backend allocate without limit.
const MAX_PAYLOAD_BYTES: usize = 64 * 1024 * 1024;

/// SecPivot's legacy exchange stores attachment bytes out of line: the entry
/// keeps a `BinDes-<id>` string field holding the file name and the payload lives
/// in the document's `<Binaries>` list. Ordinary fields must not use this prefix.
const BINARY_FIELD_PREFIX: &str = "BinDes-";

// ---------------------------------------------------------------------------
// KDBX XML document model
//
// Element and attribute names follow the KDBX XML schema — the same one the
// `keepass` crate's own (crate-private) `format::xml_db` serializer emits.
// Optional elements are all defaulted so foreign documents with extra or
// missing elements still parse, matching how KeePass' reader tolerates
// third-party payloads.
// Binary envelopes require an ID and simple Base64 text; unknown attributes or
// nested content reject instead of becoming a guessed empty attachment.
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
    #[serde(rename = "$text", default)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_transfer_obfuscation: Option<String>,
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
    /// Local exchange extension: unmarked values remain legacy text.
    #[serde(
        rename = "@SecPivotType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub value_type: Option<String>,
    /// Internal exchange uses Unix seconds, like XmlTimes; this is not the
    /// Base64/ISO timestamp representation of official KDBX PlainXml.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modification_time: Option<i64>,
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
#[serde(deny_unknown_fields)]
pub struct XmlBinary {
    // An empty Base64 element is a valid zero-byte attachment. Dedicated text
    // avoids dropping whitespace; unknown structure must not become empty data.
    #[serde(rename = "$text", default)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<XmlIconName>,
    /// Unix seconds in SecPivot's internal exchange dialect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_modification_time: Option<i64>,
}

/// Dedicated text preserves an explicitly empty or whitespace-only icon name.
#[derive(Debug, Serialize, Deserialize)]
pub struct XmlIconName {
    #[serde(rename = "$text", default)]
    pub value: String,
}

// ---------------------------------------------------------------------------
// Database -> KDBX XML document
// ---------------------------------------------------------------------------

/// Serialize the addressed entries into a KDBX XML document for the clipboard.
///
/// Protected fields are written as `Protected="True"` with their **plaintext**
/// value using SecPivot's internal legacy marker. Official KeePass PlainXml uses
/// `ProtectInMemory`; this internal payload carries no encryption, so the caller
/// must route the framed payload through the scheduled clipboard wipe.
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
        // XML readers normalize literal CR/CRLF; character references preserve
        // the exact source text, including keys and attachment file names.
        .map(|xml| xml.replace('\r', "&#13;").into_bytes())
        .map_err(|e| format!("序列化失败: {e}"))
}

fn serialize_entry(
    entry: &keepass::db::EntryRef<'_>,
    binaries: &mut Vec<XmlBinary>,
    custom_icons: &mut HashMap<String, XmlCustomIcon>,
) -> Result<XmlEntry, String> {
    // The legacy reader cannot distinguish these ordinary fields from its
    // attachment references. Refuse before publishing a lossy clipboard blob.
    if entry
        .fields
        .keys()
        .any(|key| key.starts_with(BINARY_FIELD_PREFIX))
    {
        return Err("条目包含与附件交换格式冲突的字段名".to_owned());
    }
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
                    name: icon.name.clone().map(|value| XmlIconName { value }),
                    last_modification_time: icon.last_modification_time.map(to_unix),
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
    let mut keys: Vec<&String> = entry.custom_data.keys().collect();
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
                    Some(CustomDataValue::Binary(bytes)) => {
                        Some(base64_engine::STANDARD.encode(bytes))
                    }
                    None => None,
                },
                value_type: match &entry.custom_data[key].value {
                    Some(CustomDataValue::String(_)) => None,
                    Some(CustomDataValue::Binary(_)) => Some("Binary".to_owned()),
                    None => Some("None".to_owned()),
                },
                last_modification_time: entry.custom_data[key].last_modification_time.map(to_unix),
            })
            .collect(),
    })
}

fn xml_auto_type(auto_type: &AutoType) -> XmlAutoType {
    XmlAutoType {
        enabled: Some(bool_xml(auto_type.enabled)),
        data_transfer_obfuscation: Some(
            match auto_type.data_transfer_obfuscation {
                DataTransferObfuscation::None => "0",
                DataTransferObfuscation::UseClipboard => "1",
            }
            .to_owned(),
        ),
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
    encode_payload_limited(document, encrypt, MAX_PAYLOAD_BYTES)
}

fn encode_payload_limited(
    document: &[u8],
    encrypt: bool,
    max_payload_bytes: usize,
) -> Result<Vec<u8>, String> {
    // The receiver also bounds the inflated document, even when gzip is tiny.
    if document.len() > max_payload_bytes {
        return Err("剪贴板数据过大".to_owned());
    }
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

    // Count gzip/DPAPI overhead and the flags word before publishing a frame.
    let payload_len = body
        .len()
        .checked_add(4)
        .filter(|length| *length <= max_payload_bytes)
        .ok_or_else(|| "剪贴板数据过大".to_owned())?;
    let mut payload = Vec::with_capacity(payload_len);
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
    gunzip_limited(body, MAX_PAYLOAD_BYTES)
}

fn gunzip_limited(body: &[u8], max_output_bytes: usize) -> Result<Vec<u8>, String> {
    let decoder = flate2::read::MultiGzDecoder::new(body);
    let mut out = Vec::new();
    // One extra byte detects overflow and reaches real EOF at the exact limit,
    // validating all member trailers and refusing trailing garbage before
    // publishing any bytes, rather than treating the first member as EOF.
    decoder
        .take(max_output_bytes.saturating_add(1) as u64)
        .read_to_end(&mut out)
        .map_err(|_| "剪贴板数据已损坏".to_owned())?;
    if out.len() > max_output_bytes {
        return Err("剪贴板数据过大".to_owned());
    }
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
    pub auto_type: Option<AutoType>,
    pub custom_data: Vec<PastedCustomDataItem>,
    pub tags: Vec<String>,
    pub quality_check: bool,
    pub icon: Option<i64>,
    pub custom_icon: Option<PastedCustomIcon>,
    pub foreground_color: Option<String>,
    pub background_color: Option<String>,
    pub override_url: Option<String>,
    pub expiry_time: Option<i64>,
    pub expires: bool,
}

/// A resolved native metadata value, validated before target insertion.
pub struct PastedCustomDataItem {
    pub key: String,
    pub value: Option<CustomDataValue>,
    pub last_modification_time: Option<chrono::NaiveDateTime>,
}

/// Resolved icon metadata and bytes; source identity is deliberately omitted.
pub struct PastedCustomIcon {
    pub data: Vec<u8>,
    pub name: Option<String>,
    pub last_modification_time: Option<chrono::NaiveDateTime>,
}

/// Parse a KDBX XML document into entries ready for insertion under fresh
/// UUIDs. Source UUIDs are intentionally dropped.
pub fn parse_entries_document(document: &[u8]) -> Result<Vec<PastedEntry>, String> {
    parse_entries_document_limited(document, MAX_PAYLOAD_BYTES)
}

/// Serde reads one model, which does not prove the remaining bytes form a
/// complete document. Check the envelope before publishing any parsed entries.
fn validate_entries_document(document: &[u8]) -> Result<(), String> {
    use quick_xml::events::Event;

    let invalid = || "剪贴板文档格式无效".to_owned();
    let mut reader = quick_xml::Reader::from_reader(document);
    reader.config_mut().check_comments = true;
    let mut depth = 0usize;
    let mut seen_root = false;
    loop {
        match reader.read_event().map_err(|_| invalid())? {
            Event::Start(element) => {
                if depth == 0 {
                    if seen_root || element.name().as_ref() != b"KeePassFile" {
                        return Err(invalid());
                    }
                    seen_root = true;
                }
                depth = depth.checked_add(1).ok_or_else(invalid)?;
            }
            Event::Empty(element) if depth == 0 => {
                if seen_root || element.name().as_ref() != b"KeePassFile" {
                    return Err(invalid());
                }
                seen_root = true;
            }
            Event::End(_) => depth = depth.checked_sub(1).ok_or_else(invalid)?,
            Event::Text(text) if depth == 0 => {
                if !text
                    .iter()
                    .all(|byte| matches!(byte, b' ' | b'\t' | b'\r' | b'\n'))
                {
                    return Err(invalid());
                }
            }
            Event::CData(_) | Event::GeneralRef(_) if depth == 0 => return Err(invalid()),
            Event::Decl(_) | Event::DocType(_) if seen_root => return Err(invalid()),
            Event::Eof => {
                return if seen_root && depth == 0 {
                    Ok(())
                } else {
                    Err(invalid())
                };
            }
            _ => {}
        }
    }
}

fn parse_entries_document_limited(
    document: &[u8],
    max_expanded_binary_bytes: usize,
) -> Result<Vec<PastedEntry>, String> {
    validate_entries_document(document)?;
    let file: KeePassFile =
        quick_xml::de::from_reader(document).map_err(|e| format!("剪贴板数据解析失败: {e}"))?;
    let binaries: HashMap<usize, Vec<u8>> = match file.binaries {
        Some(binaries) => {
            let mut map = HashMap::with_capacity(binaries.binaries.len());
            for binary in binaries.binaries {
                let data = base64_engine::STANDARD
                    .decode(binary.data.trim())
                    .map_err(|_| "附件数据无效".to_owned())?;
                // Definitions must have one numeric identity; shared entry
                // references are valid, but duplicate definitions are not.
                if map.insert(binary.id, data).is_some() {
                    return Err("附件编号重复".to_owned());
                }
            }
            map
        }
        None => HashMap::new(),
    };
    let custom_icons: HashMap<String, PastedCustomIcon> = match file.meta.custom_icons {
        Some(icons) => {
            // Validate every definition before filtering data. Missing or
            // damaged data must not hide a repeated exact XML identity.
            let mut ids = std::collections::HashSet::new();
            for icon in &icons.icons {
                if !ids.insert(icon.uuid.as_str()) {
                    return Err("自定义图标编号重复".to_owned());
                }
            }
            let mut map = HashMap::with_capacity(icons.icons.len());
            for icon in icons.icons {
                let last_modification_time = icon
                    .last_modification_time
                    .map(|seconds| {
                        chrono::DateTime::from_timestamp(seconds, 0)
                            .map(|time| time.naive_utc())
                            .ok_or_else(|| "自定义图标时间无效".to_owned())
                    })
                    .transpose()?;
                if let Some(data) = icon
                    .data
                    .and_then(|data| base64_engine::STANDARD.decode(data.trim()).ok())
                {
                    map.insert(
                        icon.uuid,
                        PastedCustomIcon {
                            data,
                            name: icon.name.map(|name| name.value),
                            last_modification_time,
                        },
                    );
                }
            }
            map
        }
        None => HashMap::new(),
    };

    // Attachment/icon references, icon names and resolved metadata share one copy
    // budget across all entries; preparation refuses the whole list on overflow.
    let mut remaining_binary_bytes = max_expanded_binary_bytes;
    file.root
        .group
        .entries
        .iter()
        .map(|entry| parse_entry(entry, &binaries, &custom_icons, &mut remaining_binary_bytes))
        .collect()
}

fn clone_binary_with_budget(data: &[u8], remaining: &mut usize) -> Result<Vec<u8>, String> {
    *remaining = remaining
        .checked_sub(data.len())
        .ok_or_else(|| "剪贴板数据过大".to_owned())?;
    Ok(data.to_vec())
}

fn clone_custom_icon_with_budget(
    icon: &PastedCustomIcon,
    remaining: &mut usize,
) -> Result<PastedCustomIcon, String> {
    // One shared name can otherwise expand independently of its image bytes.
    // Charge the complete independent copy before allocating either value.
    let bytes = icon
        .data
        .len()
        .checked_add(icon.name.as_ref().map_or(0, String::len))
        .ok_or_else(|| "剪贴板数据过大".to_owned())?;
    *remaining = remaining
        .checked_sub(bytes)
        .ok_or_else(|| "剪贴板数据过大".to_owned())?;
    Ok(PastedCustomIcon {
        data: icon.data.clone(),
        name: icon.name.clone(),
        last_modification_time: icon.last_modification_time,
    })
}

fn parse_entry(
    entry: &XmlEntry,
    binaries: &HashMap<usize, Vec<u8>>,
    custom_icons: &HashMap<String, PastedCustomIcon>,
    remaining_binary_bytes: &mut usize,
) -> Result<PastedEntry, String> {
    let mut fields: Vec<(String, String, bool)> = Vec::new();
    let mut attachments: Vec<(String, Vec<u8>, bool)> = Vec::new();
    let mut field_keys = std::collections::HashSet::new();

    for field in &entry.string_fields {
        let protected = field.value.protected == "True";
        if let Some(binary_id) = field.key.strip_prefix(BINARY_FIELD_PREFIX) {
            let id: usize = binary_id.parse().map_err(|_| "附件索引无效".to_owned())?;
            let data = binaries.get(&id).ok_or_else(|| "缺少附件数据".to_owned())?;
            let data = clone_binary_with_budget(data, remaining_binary_bytes)?;
            attachments.push((field.value.value.clone(), data, protected));
            continue;
        }
        // Ordinary fields map to one value per exact key. Binary references
        // above have their own identity and may legitimately repeat.
        if !field_keys.insert(field.key.as_str()) {
            return Err("条目字段重复".to_owned());
        }
        fields.push((field.key.clone(), field.value.value.clone(), protected));
    }

    let custom_data = parse_custom_data(entry.custom_data.as_ref(), remaining_binary_bytes)?;
    let auto_type = parse_auto_type(entry.auto_type.as_ref())?;

    let times = entry.times.as_ref();
    Ok(PastedEntry {
        fields,
        attachments,
        auto_type,
        custom_data,
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
            .and_then(|uuid| custom_icons.get(uuid))
            .map(|icon| clone_custom_icon_with_budget(icon, remaining_binary_bytes))
            .transpose()?,
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

/// Resolve the known metadata option before any target entry is inserted.
fn parse_auto_type(auto_type: Option<&XmlAutoType>) -> Result<Option<AutoType>, String> {
    let Some(auto_type) = auto_type else {
        return Ok(None);
    };
    let data_transfer_obfuscation = match auto_type.data_transfer_obfuscation.as_deref() {
        None => DataTransferObfuscation::None,
        Some(value) => match value.trim().parse::<u32>() {
            Ok(0) => DataTransferObfuscation::None,
            Ok(1) => DataTransferObfuscation::UseClipboard,
            _ => return Err("条目自动输入配置无效".to_owned()),
        },
    };
    Ok(Some(AutoType {
        enabled: auto_type.enabled.as_deref() == Some("True"),
        default_sequence: auto_type.default_sequence.clone(),
        data_transfer_obfuscation,
        associations: auto_type
            .associations
            .iter()
            .map(|association| AutoTypeAssociation {
                window: association.window.clone(),
                sequence: association.keystroke_sequence.clone(),
            })
            .collect(),
    }))
}

fn parse_custom_data(
    data: Option<&XmlCustomData>,
    remaining_binary_bytes: &mut usize,
) -> Result<Vec<PastedCustomDataItem>, String> {
    let Some(data) = data else {
        return Ok(Vec::new());
    };
    let mut keys = std::collections::HashSet::new();
    let mut values = Vec::with_capacity(data.items.len());
    for item in &data.items {
        if !keys.insert(item.key.as_str()) {
            return Err("条目自定义数据重复".to_owned());
        }
        let value = match item.value_type.as_deref() {
            None | Some("String") => item.value.clone().map(CustomDataValue::String),
            Some("None") if item.value.is_none() => None,
            Some("Binary") => {
                let encoded = item
                    .value
                    .as_deref()
                    .ok_or_else(|| "自定义数据无效".to_owned())?;
                Some(CustomDataValue::Binary(decode_custom_binary(
                    encoded,
                    remaining_binary_bytes,
                )?))
            }
            _ => return Err("自定义数据类型无效".to_owned()),
        };
        values.push(PastedCustomDataItem {
            key: item.key.clone(),
            value,
            last_modification_time: item
                .last_modification_time
                .map(|seconds| {
                    chrono::DateTime::from_timestamp(seconds, 0)
                        .map(|time| time.naive_utc())
                        .ok_or_else(|| "自定义数据时间无效".to_owned())
                })
                .transpose()?,
        });
    }
    Ok(values)
}

fn decode_custom_binary(encoded: &str, remaining: &mut usize) -> Result<Vec<u8>, String> {
    let encoded = encoded.trim();
    // Base64's conservative decoded-size estimate can exceed the actual size
    // by two padding bytes. Allow a bounded three-byte lookahead, then charge
    // only the complete decoded value before publishing it.
    let capacity = base64::decoded_len_estimate(encoded.len()).min(remaining.saturating_add(3));
    let mut bytes = zeroize::Zeroizing::new(vec![0; capacity]);
    let count = base64_engine::STANDARD
        .decode_slice(encoded, &mut bytes[..])
        .map_err(|error| match error {
            base64::DecodeSliceError::OutputSliceTooSmall => "剪贴板数据过大".to_owned(),
            base64::DecodeSliceError::DecodeError(_) => "自定义数据无效".to_owned(),
        })?;
    *remaining = remaining
        .checked_sub(count)
        .ok_or_else(|| "剪贴板数据过大".to_owned())?;
    bytes.truncate(count);
    Ok(std::mem::take(&mut *bytes))
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
                value: item.value.clone(),
                last_modification_time: item.last_modification_time,
            },
        );
    }
    if let Some(auto_type) = &entry.auto_type {
        node.autotype = Some(auto_type.clone());
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
    if let Some(icon) = &entry.custom_icon {
        // A pasted custom icon becomes a fresh icon in the target database, so
        // no cross-database icon id has to be remapped.
        let mut restored = node.set_icon_custom_new(icon.data.clone());
        restored.name = icon.name.clone();
        restored.last_modification_time = icon.last_modification_time;
    } else if let Some(icon) = entry.icon.filter(|icon| *icon >= 0) {
        // The clipboard payload is untrusted: a negative icon id would wrap
        // into a huge usize and corrupt the built-in icon lookup.
        node.set_icon_builtin(icon as usize);
    }
    Ok(node.as_ref().id().uuid())
}

#[cfg(test)]
#[path = "exchange_text_tests.rs"]
mod text_tests;

#[cfg(test)]
#[path = "exchange_binary_tests.rs"]
mod binary_tests;

#[cfg(test)]
#[path = "exchange_field_identity_tests.rs"]
mod field_identity_tests;

#[cfg(test)]
#[path = "exchange_encode_tests.rs"]
mod encode_tests;

#[cfg(test)]
#[path = "exchange_expansion_tests.rs"]
mod expansion_tests;

#[cfg(test)]
#[path = "exchange_inflate_tests.rs"]
mod inflate_tests;

#[cfg(test)]
#[path = "exchange_document_tests.rs"]
mod document_tests;

#[cfg(test)]
#[path = "exchange_gzip_member_tests.rs"]
mod gzip_member_tests;

#[cfg(test)]
#[path = "exchange_string_identity_tests.rs"]
mod string_identity_tests;

#[cfg(test)]
#[path = "exchange_custom_identity_tests.rs"]
mod custom_identity_tests;

#[cfg(test)]
#[path = "exchange_custom_value_tests.rs"]
mod custom_value_tests;

#[cfg(test)]
#[path = "exchange_custom_time_tests.rs"]
mod custom_time_tests;

#[cfg(test)]
#[path = "exchange_icon_identity_tests.rs"]
mod icon_identity_tests;

#[cfg(test)]
#[path = "exchange_icon_metadata_tests.rs"]
mod icon_metadata_tests;

#[cfg(test)]
#[path = "exchange_autotype_tests.rs"]
mod autotype_tests;

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
            entry.autotype = Some(AutoType {
                enabled: true,
                default_sequence: Some("{USERNAME}{TAB}{PASSWORD}{ENTER}".to_owned()),
                data_transfer_obfuscation: DataTransferObfuscation::UseClipboard,
                associations: vec![AutoTypeAssociation {
                    window: "Synthetic browser *".to_owned(),
                    sequence: "{USERNAME}".to_owned(),
                }],
            });
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
        assert_eq!(
            entry.auto_type.as_ref(),
            db.entry(id).unwrap().autotype.as_ref()
        );
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
