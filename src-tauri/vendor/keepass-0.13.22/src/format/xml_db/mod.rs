//! XML (de)serialization for KeePass databases.
//!
//! This module provides types that mirror the ones in `crate::db`, but are tailored to closely fit
//! the XML structure of KeePass databases for easy `#[derive(Serialize, Deserialize)]`.
//!
//! See <https://keepass.info/help/download/KDBX_XML.xsd> for an XML schema.

pub mod custom_serde;
pub mod entry;
pub mod group;
pub mod meta;
pub mod tags;
pub mod times;
pub mod timestamp;

use serde::{Deserialize, Serialize, Serializer};

use base64::{engine::general_purpose as base64_engine, Engine as _};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    crypt::ciphers::Cipher,
    db::{GroupId, Value},
    format::xml_db::{
        custom_serde::cs_opt_string, entry::UnprotectError, group::Group, meta::Meta,
        timestamp::Timestamp,
    },
};
#[cfg(feature = "save_kdbx4")]
use crate::{crypt::CryptographyError, db::DatabaseSaveError};

pub fn parse_xml(
    data: &[u8],
    header_attachments: &[Value<Vec<u8>>],
    inner_decryptor: &mut dyn Cipher,
) -> Result<crate::db::Database, ParseXmlError> {
    let kdbx: KeePassFile = quick_xml::de::from_reader(data)?;
    Ok(kdbx.xml_to_db(inner_decryptor, header_attachments)?)
}

/// PlainXml CustomData values are text, even if they happen to be Base64.
/// Keep KDBX's legacy typed decoder isolated from this import boundary.
pub(crate) fn parse_plain_inner_xml(
    data: &[u8],
    header_attachments: &[Value<Vec<u8>>],
    inner_decryptor: &mut dyn Cipher,
) -> Result<crate::db::Database, ParseXmlError> {
    let mut kdbx: KeePassFile = quick_xml::de::from_reader(data)?;
    fn entry_plain(entry: &mut entry::Entry) {
        if let Some(data) = &mut entry.custom_data {
            data.plaintext_strings();
        }
        if let Some(history) = &mut entry.history {
            for old in &mut history.entries {
                entry_plain(old);
            }
        }
    }
    fn group_plain(group: &mut Group) {
        if let Some(data) = &mut group.custom_data {
            data.plaintext_strings();
        }
        for child in &mut group.children {
            match child {
                group::GroupOrEntry::Group(child) => group_plain(child),
                group::GroupOrEntry::Entry(child) => entry_plain(child),
            }
        }
    }
    kdbx.meta.plaintext_custom_data();
    group_plain(&mut kdbx.root.group);
    Ok(kdbx.xml_to_db(inner_decryptor, header_attachments)?)
}

/// Errors that can occur during parsing of the inner XML database of a KDBX file
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ParseXmlError {
    /// Errors related to XML deserialization or serialization.
    #[error("Error parsing XML inside KDBX: {0}")]
    Xml(#[from] quick_xml::DeError),

    /// Errors related to unprotecting entries, such as decryption failures or unsupported
    /// encryption methods.
    #[error("Error unprotecting entry: {0}")]
    Unprotect(#[from] UnprotectError),
}

#[cfg(feature = "save_kdbx4")]
#[allow(clippy::type_complexity)]
pub fn to_xml(
    db: &crate::db::Database,
    inner_encryptor: &mut dyn Cipher,
) -> Result<(Vec<u8>, Vec<crate::db::Value<Vec<u8>>>), DatabaseSaveError> {
    let mut attachments: Vec<(usize, Value<Vec<u8>>)> = db
        .attachments
        .iter()
        .map(|(id, attachment)| (id.id(), attachment.data.clone()))
        .collect();

    attachments.sort_by_key(|(id, _)| *id);

    // In-memory IDs may have gaps after deletion. KDBX4 binary Ref values
    // address positions in this exact header array, not the original IDs.
    let dense_ids: HashMap<usize, usize> = attachments
        .iter()
        .enumerate()
        .map(|(dense, (id, _))| (*id, dense))
        .collect();
    let mut kdbx = KeePassFile::db_to_xml(db, inner_encryptor)?;
    fn remap_entry(
        entry: &mut entry::Entry,
        dense: &HashMap<usize, usize>,
    ) -> Result<(), DatabaseSaveError> {
        for field in &mut entry.binary_fields {
            field.value.value_ref = *dense.get(&field.value.value_ref).ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Entry references a missing attachment",
                )
            })?;
        }
        if let Some(history) = &mut entry.history {
            for historical in &mut history.entries {
                remap_entry(historical, dense)?;
            }
        }
        Ok(())
    }
    fn remap_group(
        group: &mut Group,
        dense: &HashMap<usize, usize>,
    ) -> Result<(), DatabaseSaveError> {
        for child in &mut group.children {
            match child {
                group::GroupOrEntry::Group(group) => remap_group(group, dense)?,
                group::GroupOrEntry::Entry(entry) => remap_entry(entry, dense)?,
            }
        }
        Ok(())
    }
    remap_group(&mut kdbx.root.group, &dense_ids)?;
    let xml = quick_xml::se::to_string_with_root("KeePassFile", &kdbx)?;
    // Validate after protected values became base64; never silently drop text.
    if xml.chars().any(|c| {
        !matches!(c, '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}'
            | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
    }) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Database contains a character not allowed in XML 1.0",
        )
        .into());
    }
    // Model strings are XML text, never CDATA. Character references preserve
    // CR across XML's mandatory line-ending normalization without changing
    // literal entity text or protected fields' base64 payloads.
    let xml = if xml.contains('\r') {
        xml.replace('\r', "&#xD;")
    } else {
        xml
    }
    .into_bytes();

    let attachments = attachments.into_iter().map(|(_, data)| data).collect();

    Ok((xml, attachments))
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct KeePassFile {
    meta: Meta,
    root: Root,
}

/// PlainXml reuses the complete KDBX model and its single dense attachment map.
/// Only the controlled serializer's field protection and date encodings change;
/// arbitrary user field contents are never interpreted as XML or timestamps.
#[cfg(feature = "save_kdbx4")]
pub fn to_plain_xml(db: &crate::db::Database) -> Result<Vec<u8>, DatabaseSaveError> {
    use chrono::Datelike;
    use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
    use std::convert::TryInto;
    use zeroize::Zeroizing;

    let invalid = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Cannot represent database as plaintext XML",
        )
    };
    let (xml, binaries) = to_xml(db, &mut crate::crypt::ciphers::PlainCipher::new(&[])?)?;
    let xml = Zeroizing::new(xml);
    let mut reader = quick_xml::Reader::from_reader(xml.as_slice());
    let mut output = Zeroizing::new(Vec::new());
    let mut writer = quick_xml::Writer::new(&mut *output);
    writer.write_event(Event::Decl(BytesDecl::new(
        "1.0",
        Some("utf-8"),
        Some("yes"),
    )))?;
    writer.write_event(Event::Text(BytesText::new("\n")))?;
    let mut path: Vec<Vec<u8>> = Vec::new();
    loop {
        match reader.read_event().map_err(|_| invalid())? {
            Event::Start(start) => {
                let name = start.name();
                let parent = path.last().map(Vec::as_slice);
                if name.as_ref() == b"Generator" && parent == Some(b"Meta") {
                    reader.read_text(name).map_err(|_| invalid())?;
                } else if name.as_ref() == b"Value"
                    && parent == Some(b"String")
                    && start
                        .try_get_attribute("Protected")
                        .map_err(|_| invalid())?
                        .is_some_and(|a| a.value.as_ref() == b"True")
                {
                    let raw = reader.read_text(name).map_err(|_| invalid())?;
                    let bytes = Zeroizing::new(
                        base64_engine::STANDARD
                            .decode(raw.as_ref())
                            .map_err(|_| invalid())?,
                    );
                    let text = std::str::from_utf8(&bytes).map_err(|_| invalid())?;
                    let mut value = BytesStart::new("Value");
                    value.push_attribute(("ProtectInMemory", "True"));
                    writer.write_event(Event::Start(value))?;
                    writer.write_event(Event::Text(BytesText::new(text)))?;
                    writer.write_event(Event::End(BytesEnd::new("Value")))?;
                } else if plain_timestamp_path(&path, name.as_ref()) {
                    let raw = reader.read_text(name).map_err(|_| invalid())?;
                    let bytes: [u8; 8] = base64_engine::STANDARD
                        .decode(raw.as_ref())
                        .map_err(|_| invalid())?
                        .try_into()
                        .map_err(|_| invalid())?;
                    let time = chrono::Duration::try_seconds(i64::from_le_bytes(bytes))
                        .and_then(|duration| {
                            timestamp::get_epoch_baseline().checked_add_signed(duration)
                        })
                        .filter(|t| (1..=9999).contains(&t.date().year()))
                        .ok_or_else(invalid)?;
                    writer.write_event(Event::Start(start.clone()))?;
                    writer.write_event(Event::Text(BytesText::new(
                        &time.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
                    )))?;
                    writer.write_event(Event::End(start.to_end()))?;
                } else {
                    let meta = name.as_ref() == b"Meta";
                    path.push(name.as_ref().to_vec());
                    writer.write_event(Event::Start(start))?;
                    if meta {
                        writer.write_event(Event::Start(BytesStart::new("Generator")))?;
                        writer.write_event(Event::Text(BytesText::new("SecPivot")))?;
                        writer.write_event(Event::End(BytesEnd::new("Generator")))?;
                    }
                }
            }
            Event::End(end) => {
                if path.len() == 2 && end.name().as_ref() == b"Meta" {
                    writer.write_event(Event::Start(BytesStart::new("Binaries")))?;
                    for (id, binary) in binaries.iter().enumerate() {
                        let mut start = BytesStart::new("Binary");
                        let id = id.to_string();
                        start.push_attribute(("ID", id.as_str()));
                        start.push_attribute(("Compressed", "False"));
                        writer.write_event(Event::Start(start))?;
                        let encoded = Zeroizing::new(base64_engine::STANDARD.encode(binary.get()));
                        writer.write_event(Event::Text(BytesText::new(&encoded)))?;
                        writer.write_event(Event::End(BytesEnd::new("Binary")))?;
                    }
                    writer.write_event(Event::End(BytesEnd::new("Binaries")))?;
                }
                path.pop();
                writer.write_event(Event::End(end))?;
            }
            Event::Empty(start)
                if start.name().as_ref() == b"Generator"
                    && path.last().map(Vec::as_slice) == Some(b"Meta") => {}
            Event::Empty(start)
                if start.name().as_ref() == b"Value"
                    && path.last().map(Vec::as_slice) == Some(b"String")
                    && start
                        .try_get_attribute("Protected")
                        .map_err(|_| invalid())?
                        .is_some_and(|a| a.value.as_ref() == b"True") =>
            {
                let mut value = BytesStart::new("Value");
                value.push_attribute(("ProtectInMemory", "True"));
                writer.write_event(Event::Empty(value))?;
            }
            Event::Eof => break,
            event => writer.write_event(event)?,
        }
    }
    writer.write_event(Event::Text(BytesText::new("\n")))?;
    drop(writer);
    let text =
        Zeroizing::new(String::from_utf8(std::mem::take(&mut *output)).map_err(|_| invalid())?);
    if text.chars().any(|c| {
        !matches!(c, '\u{9}' | '\u{a}' | '\u{d}' | '\u{20}'..='\u{d7ff}'
        | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
    }) {
        return Err(invalid().into());
    }
    Ok(text.replace('\r', "&#13;").into_bytes())
}

#[cfg(feature = "save_kdbx4")]
fn plain_timestamp_path(path: &[Vec<u8>], name: &[u8]) -> bool {
    match path.last().map(Vec::as_slice) {
        Some(b"Times") => matches!(
            name,
            b"CreationTime"
                | b"LastModificationTime"
                | b"LastAccessTime"
                | b"ExpiryTime"
                | b"LocationChanged"
        ),
        Some(b"Meta") => matches!(
            name,
            b"DatabaseNameChanged"
                | b"DatabaseDescriptionChanged"
                | b"DefaultUserNameChanged"
                | b"MasterKeyChanged"
                | b"RecycleBinChanged"
                | b"EntryTemplatesGroupChanged"
                | b"SettingsChanged"
        ),
        Some(b"Icon") => name == b"LastModificationTime",
        Some(b"Item") => name == b"LastModificationTime",
        Some(b"DeletedObject") => name == b"DeletionTime",
        _ => false,
    }
}

impl KeePassFile {
    /// Convert from XML representation to database representation.
    fn xml_to_db(
        mut self,
        inner_decryptor: &mut dyn Cipher,
        header_attachments: &[Value<Vec<u8>>],
    ) -> Result<crate::db::Database, UnprotectError> {
        let mut db =
            crate::db::Database::new_with_root_id(GroupId::from_uuid(self.root.group.uuid.0));

        let mut attachments = HashMap::new();

        // convert header attachments (KDBX4-style) to database attachments
        for (i, header_attachment) in header_attachments.iter().enumerate() {
            let attachment = crate::db::Attachment {
                id: crate::db::AttachmentId::new(i),
                entries: HashSet::new(),
                data: header_attachment.clone(),
            };
            attachments.insert(attachment.id, attachment);
        }

        // convert XML attachments (KDBX3-style) to database attachments
        if let Some(binaries) = self.meta.binaries.take() {
            for binary in binaries.binaries {
                let id = crate::db::AttachmentId::next_free(&db);
                let data = binary.xml_to_db(inner_decryptor)?;

                attachments.insert(
                    id,
                    crate::db::Attachment {
                        id,
                        entries: HashSet::new(),
                        data,
                    },
                );
            }
        }

        let custom_icons = self
            .meta
            .custom_icons
            .take()
            .map(|ci| {
                ci.icons
                    .into_iter()
                    .map(|icon| {
                        let ci: crate::db::CustomIcon = icon.into();
                        (ci.id, ci)
                    })
                    .collect()
            })
            .unwrap_or_default();

        db.meta = self.meta.into();

        db.deleted_objects = self
            .root
            .deleted_objects
            .map(|del_objs| {
                del_objs
                    .objects
                    .into_iter()
                    .map(|obj| (obj.uuid.0, obj.deletion_time.map(|ts| ts.time)))
                    .collect()
            })
            .unwrap_or_default();

        self.root.group.xml_to_db_handle(
            db.root_mut(),
            &attachments,
            &custom_icons,
            inner_decryptor,
        )?;

        db.attachments = attachments;
        db.custom_icons = custom_icons;
        db.rebuild_attachment_references();

        // Re-populate CustomIcon back-reference sets.
        //
        // The XML parser creates CustomIcon values with empty `entries` and `groups` sets
        // because icon data and entry/group data live in separate parts of the XML file.
        // We perform a single pass here to reconstruct all back-references from the icon
        // fields that were already set on each entry and group during xml_to_db_handle.
        let entry_ids: Vec<crate::db::EntryId> = db.entries.keys().copied().collect();
        for entry_id in entry_ids {
            // current version
            if let Some(crate::db::Icon::Custom(icon_id)) =
                db.entries.get(&entry_id).and_then(|e| e.icon.as_ref())
            {
                if let Some(icon) = db.custom_icons.get_mut(icon_id) {
                    icon.entries.insert((entry_id, None));
                }
            }

            // historical versions
            if let Some(entry) = db.entries.get(&entry_id) {
                let history_len = entry.history.as_ref().map_or(0, |h| h.entries.len());

                for i in 0..history_len {
                    #[allow(clippy::indexing_slicing)]
                    // We just checked that the index is in bounds
                    if let Some(crate::db::Icon::Custom(icon_id)) = entry
                        .history
                        .as_ref()
                        .and_then(|h| h.entries[i].icon.as_ref())
                    {
                        if let Some(icon) = db.custom_icons.get_mut(icon_id) {
                            icon.entries.insert((entry_id, Some(i)));
                        }
                    }
                }
            }
        }

        let group_ids: Vec<crate::db::GroupId> = db.groups.keys().copied().collect();
        for group_id in group_ids {
            if let Some(crate::db::Icon::Custom(icon_id)) =
                db.groups.get(&group_id).and_then(|g| g.icon.as_ref())
            {
                if let Some(icon) = db.custom_icons.get_mut(icon_id) {
                    icon.groups.insert(group_id);
                }
            }
        }

        Ok(db)
    }

    /// Convert from database representation to XML representation.
    #[cfg(feature = "save_kdbx4")]
    fn db_to_xml(
        db: &crate::db::Database,
        inner_cipher: &mut dyn Cipher,
    ) -> Result<Self, CryptographyError> {
        use crate::format::xml_db::meta::Icon;

        let group = Group::db_to_xml(db.root(), inner_cipher)?;

        let mut meta: Meta = db.meta.clone().into();
        meta.custom_icons.get_or_insert_default().icons =
            db.custom_icons.values().cloned().map(Icon::from).collect();

        let deleted_objects = if db.deleted_objects.is_empty() {
            None
        } else {
            Some(DeletedObjects {
                objects: db
                    .deleted_objects
                    .iter()
                    .map(|(uuid, deletion_time)| DeletedObject {
                        uuid: UUID(*uuid),
                        deletion_time: deletion_time.map(Timestamp::from),
                    })
                    .collect(),
            })
        };

        Ok(KeePassFile {
            meta,
            root: Root {
                group,
                deleted_objects,
            },
        })
    }
}

/// A UUID deserialized from a Base64 string.
#[allow(clippy::upper_case_acronyms)] // Keep the name consistent with KeePass XML schema
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct UUID(Uuid);

impl<'de> Deserialize<'de> for UUID {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;

        let v = base64_engine::STANDARD
            .decode(input)
            .map_err(serde::de::Error::custom)?;

        let uuid = Uuid::from_slice(&v).map_err(serde::de::Error::custom)?;
        Ok(UUID(uuid))
    }
}

impl Serialize for UUID {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let b64 = base64_engine::STANDARD.encode(self.0.as_bytes());
        serializer.serialize_str(&b64)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Root {
    #[serde(rename = "Group")]
    pub group: Group,

    #[serde(default, rename = "DeletedObjects")]
    pub deleted_objects: Option<DeletedObjects>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeletedObjects {
    #[serde(default, rename = "DeletedObject")]
    pub objects: Vec<DeletedObject>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeletedObject {
    #[serde(rename = "UUID")]
    uuid: UUID,

    #[serde(
        default,
        rename = "DeletionTime",
        alias = "deletion_time",
        with = "cs_opt_string"
    )]
    deletion_time: Option<Timestamp>,
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn secpivot_plain_xml_keeps_empty_protected_and_literal_date_named_fields() {
        let mut db = crate::db::Database::new();
        let mut root = db.root_mut();
        let mut entry = root.add_entry();
        entry.set_protected("Password", "");
        entry.set_protected("CreationTime", " \r\n&<😀\t ");
        entry.set_unprotected("MasterKeyChanged", "not a timestamp");
        drop(entry);
        drop(root);
        let before = db.clone();
        let xml = String::from_utf8(db.export_plain_xml().unwrap()).unwrap();
        assert!(xml.contains("<Value ProtectInMemory=\"True\"/>"));
        assert!(xml.contains(" &#13;\n&amp;&lt;😀\t "));
        assert!(xml.contains("not a timestamp"));
        assert!(!xml.contains(" Protected="));
        assert_eq!(db, before);
    }

    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn secpivot_plain_xml_rejects_invalid_historical_secret_without_mutation() {
        let mut db = crate::db::Database::new();
        let id = db.root_mut().add_entry().id();
        db.entry_mut(id)
            .unwrap()
            .set_protected("Password", "synthetic\u{0000}");
        db.entry_mut(id).unwrap().edit_tracking(|e| {
            e.set_protected("Password", "valid");
        });
        let before = db.clone();
        assert!(db.export_plain_xml().is_err());
        assert_eq!(db, before);
    }

    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn secpivot_plain_xml_rejects_missing_current_or_historical_binary() {
        for historical in [false, true] {
            let mut db = crate::db::Database::new();
            let id = db.root_mut().add_entry().id();
            let binary = db
                .entry_mut(id)
                .unwrap()
                .add_attachment("synthetic.bin", crate::db::Value::protected(vec![0, 255]))
                .id();
            if historical {
                db.entry_mut(id).unwrap().edit_tracking(|e| {
                    e.as_mut().remove_attachment_by_name("synthetic.bin");
                });
            }
            db.attachments.remove(&binary);
            let before = db.clone();
            assert!(db.export_plain_xml().is_err());
            assert_eq!(db, before);
        }
    }

    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn secpivot_plain_xml_rejects_dates_outside_official_range() {
        let mut db = crate::db::Database::new();
        db.meta.master_key_changed = Some(
            chrono::NaiveDate::from_ymd_opt(10000, 1, 1)
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap(),
        );
        let before = db.clone();
        assert!(db.export_plain_xml().is_err());
        assert_eq!(db, before);
    }

    use super::*;

    #[derive(Serialize, Deserialize)]
    struct Test<T>(T);

    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn secpivot_kdbx_scalar_writer_rejects_illegal_text_and_keys() {
        for scalar in [0u32, 1, 8, 11, 12, 14, 31, 0xfffe, 0xffff] {
            let text = format!("synthetic{}tail", char::from_u32(scalar).unwrap());
            for location in ["value", "key", "group", "meta"] {
                let mut db = crate::Database::new();
                match location {
                    "group" => db.root_mut().name = text.clone(),
                    "meta" => db.meta.database_description = Some(text.clone()),
                    _ => {
                        let mut root = db.root_mut();
                        let mut entry = root.add_entry();
                        if location == "key" {
                            entry.set_protected(&text, "synthetic");
                        } else {
                            entry.set_unprotected("Notes", &text);
                        }
                    }
                }
                let mut cipher = crate::config::InnerCipherConfig::Plain
                    .get_cipher(&[])
                    .unwrap();
                let error = to_xml(&db, &mut *cipher).unwrap_err().to_string();
                assert!(error.contains("not allowed in XML 1.0"));
                assert!(!error.contains(&text));
            }
        }
    }

    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn secpivot_kdbx_scalar_writer_keeps_legal_and_protected_text() {
        for scalar in [
            0u32, 1, 8, 11, 12, 14, 31, 0xfffe, 0xffff, 9, 10, 13, 0x85, 0x1fffe, 0x10ffff,
        ] {
            let secret = format!("synthetic{}tail", char::from_u32(scalar).unwrap());
            let plain = if [9, 10, 13, 0x85, 0x1fffe, 0x10ffff].contains(&scalar) {
                secret.as_str()
            } else {
                "synthetic ordinary"
            };
            let mut db = crate::Database::new();
            let id = {
                let mut root = db.root_mut();
                let mut entry = root.add_entry();
                entry.set_unprotected("Notes", plain);
                entry.set_protected("Password", &secret);
                entry.id()
            };
            let mut cipher = crate::config::InnerCipherConfig::Plain
                .get_cipher(&[])
                .unwrap();
            let (xml, attachments) = to_xml(&db, &mut *cipher).unwrap();
            let loaded = parse_xml(&xml, &attachments, &mut *cipher).unwrap();
            let entry = loaded.entry(id).unwrap();
            assert_eq!(entry.get("Notes"), Some(plain));
            assert_eq!(entry.fields["Password"].get(), &secret);
            assert!(entry.fields["Password"].is_protected());
        }
    }

    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn secpivot_kdbx_carriage_serializer_preserves_text_and_literal_entities() {
        let expected = "synthetic\r\nline\rtab\t😀";
        let mut db = crate::Database::new();
        let id = {
            let mut root = db.root_mut();
            let mut entry = root.add_entry();
            entry.set_unprotected("Plain\r\nkey", expected);
            entry.set_unprotected("Literal", "literal &#xD; &amp; <tag>");
            entry.set_protected("Protected", expected);
            entry.id()
        };
        let mut cipher = crate::config::InnerCipherConfig::Plain
            .get_cipher(&[])
            .unwrap();
        let (xml, attachments) = to_xml(&db, &mut *cipher).unwrap();
        assert!(
            !xml.contains(&b'\r'),
            "literal CR would normalize on XML read"
        );
        assert!(std::str::from_utf8(&xml).unwrap().contains("&#xD;"));
        let parsed = parse_xml(&xml, &attachments, &mut *cipher).unwrap();
        let entry = parsed.entry(id).unwrap();
        assert_eq!(entry.get("Plain\r\nkey"), Some(expected));
        assert_eq!(entry.get("Literal"), Some("literal &#xD; &amp; <tag>"));
        assert_eq!(entry.get("Protected"), Some(expected));
        assert!(entry.fields["Protected"].is_protected());
    }

    #[test]
    fn secpivot_kdbx_carriage_reader_keeps_entity_and_standard_normalization_semantics() {
        let xml = r#"<KeePassFile><Meta/><Root><Group><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID><Name>Root</Name>
            <Entry><UUID>AQEBAQEBAQEBAQEBAQEBAQ==</UUID>
                <String><Key>Reference</Key><Value>synthetic&#xD;&#xA;line&#xD;tail</Value></String>
                <String><Key>Literal</Key><Value>literal &amp;#xD;</Value></String>
            </Entry></Group></Root></KeePassFile>"#;
        let mut cipher = crate::config::InnerCipherConfig::Plain
            .get_cipher(&[])
            .unwrap();
        let parsed = parse_xml(xml.as_bytes(), &[], &mut *cipher).unwrap();
        let root = parsed.root();
        let entry = root.entries().next().unwrap();
        assert_eq!(entry.get("Reference"), Some("synthetic\r\nline\rtail"));
        assert_eq!(entry.get("Literal"), Some("literal &#xD;"));
        let raw = xml.replace(
            "synthetic&#xD;&#xA;line&#xD;tail",
            "synthetic\r\nline\rtail",
        );
        let normalized = parse_xml(raw.as_bytes(), &[], &mut *cipher).unwrap();
        let root = normalized.root();
        let entry = root.entries().next().unwrap();
        assert_eq!(entry.get("Reference"), Some("synthetic\nline\ntail"));
    }

    #[test]
    fn test_deserialize_uuid() {
        let uuid_str = "AAECAwQFBgcICQoLDA0ODw==";
        let uuid: UUID = quick_xml::de::from_str(uuid_str).unwrap();
        assert_eq!(
            uuid.0.as_bytes(),
            &[
                0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
                0x0e, 0x0f
            ]
        );
    }

    #[test]
    fn test_serialize_uuid() {
        let uuid = UUID(Uuid::from_bytes([
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ]));
        let serialized = quick_xml::se::to_string(&Test(uuid)).unwrap();
        assert_eq!(serialized, "<Test>AAECAwQFBgcICQoLDA0ODw==</Test>");
    }

    #[test]
    fn test_serialize_deleted_object_deletion_time() {
        let deleted_object = DeletedObject {
            uuid: UUID(Uuid::nil()),
            deletion_time: Some(Timestamp::new_iso8601(
                chrono::NaiveDateTime::parse_from_str("2026-08-15T12:34:56", "%Y-%m-%dT%H:%M:%S")
                    .unwrap(),
            )),
        };

        let serialized =
            quick_xml::se::to_string_with_root("DeletedObject", &deleted_object).unwrap();

        assert!(serialized.contains("<DeletionTime>"));
        assert!(!serialized.contains("<deletion_time>"));
    }

    #[test]
    fn test_deserialize_legacy_deleted_object_deletion_time() {
        let deleted_object: DeletedObject = quick_xml::de::from_str(
            "<DeletedObject><UUID>AAAAAAAAAAAAAAAAAAAAAA==</UUID><deletion_time>2026-08-15T12:34:56Z</deletion_time></DeletedObject>",
        )
        .unwrap();

        assert!(deleted_object.deletion_time.is_some());
    }

    #[cfg(feature = "save_kdbx4")]
    #[test]
    fn test_serialize_deletion_time_mode() {
        let xml = r#"<KeePassFile>
            <Meta></Meta>
            <Root>
               <Group><UUID>tP/vJ/3uSHyomfPZ4dXVlg==</UUID><Name></Name></Group>
               <DeletedObjects>
                   <DeletedObject>
                       <UUID>30lsaI9KSYefuJb0PHSRiw==</UUID>
                       <DeletionTime>io8Y4g4AAAA=</DeletionTime>
                   </DeletedObject>
               </DeletedObjects>
            </Root>
        </KeePassFile>"#;
        let mut cipher = crate::config::InnerCipherConfig::Plain
            .get_cipher(&[])
            .unwrap();
        let db = parse_xml(xml.as_bytes(), &[], &mut *cipher).unwrap();
        let kdbx = KeePassFile::db_to_xml(&db, &mut *cipher).unwrap();
        let xml = quick_xml::se::to_string_with_root("DeletedObjects", &kdbx.root.deleted_objects)
            .unwrap();
        assert!(xml.contains("<DeletionTime>io8Y4g4AAAA=</DeletionTime>"));
    }
}
