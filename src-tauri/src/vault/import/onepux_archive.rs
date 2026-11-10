//! Native-only, bounded 1PUX archive decoding. Never extracts files to disk or
//! serializes plaintext through IPC. Item migration is a separate transaction.

use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::io::{Cursor, Read};
use zeroize::{Zeroize, Zeroizing};
use zip::{CompressionMethod, ZipArchive};

const INVALID: &str = "1PUX 归档无效或使用了不支持的格式；未导入任何内容";
const LIMIT: &str = "1PUX 归档超过导入资源上限；未导入任何内容";

#[derive(Clone, Copy)]
struct Limits {
    compressed: usize,
    members: usize,
    member: usize,
    json: usize,
    expanded: usize,
}

const LIMITS: Limits = Limits {
    compressed: 64 * 1024 * 1024,
    members: 8192,
    member: 64 * 1024 * 1024,
    json: 8 * 1024 * 1024,
    expanded: 256 * 1024 * 1024,
};

struct SecretJson(Value);

// Reject repeated JSON keys instead of silently discarding earlier source data.
// Partial owned trees are also wrapped so errors release their strings via Drop.
impl<'de> Deserialize<'de> for SecretJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct JsonVisitor;
        impl<'de> Visitor<'de> for JsonVisitor {
            type Value = SecretJson;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("JSON without duplicate keys")
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<SecretJson, E> {
                Ok(SecretJson(Value::Null))
            }

            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<SecretJson, E> {
                Ok(SecretJson(Value::Bool(value)))
            }

            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<SecretJson, E> {
                Ok(SecretJson(Value::from(value)))
            }

            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<SecretJson, E> {
                Ok(SecretJson(Value::from(value)))
            }

            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<SecretJson, E> {
                Ok(SecretJson(Value::from(value)))
            }

            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<SecretJson, E> {
                self.visit_string(value.to_owned())
            }

            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<SecretJson, E> {
                Ok(SecretJson(Value::String(value)))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<SecretJson, A::Error> {
                let mut result = SecretJson(Value::Array(Vec::new()));
                while let Some(mut value) = seq.next_element::<SecretJson>()? {
                    result
                        .0
                        .as_array_mut()
                        .unwrap()
                        .push(std::mem::take(&mut value.0));
                }
                Ok(result)
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<SecretJson, A::Error> {
                let mut result = SecretJson(Value::Object(serde_json::Map::new()));
                while let Some(key) = map.next_key::<String>()? {
                    let mut key = Zeroizing::new(key);
                    let values = result.0.as_object_mut().unwrap();
                    if values.contains_key(&*key) {
                        return Err(serde::de::Error::custom("duplicate JSON key"));
                    }
                    let mut value = map.next_value::<SecretJson>()?;
                    values.insert(std::mem::take(&mut *key), std::mem::take(&mut value.0));
                }
                Ok(result)
            }
        }
        deserializer.deserialize_any(JsonVisitor)
    }
}

impl Drop for SecretJson {
    fn drop(&mut self) {
        fn wipe(value: Value) {
            match value {
                Value::String(mut value) => value.zeroize(),
                Value::Array(values) => values.into_iter().for_each(wipe),
                Value::Object(values) => {
                    for (mut key, value) in values {
                        key.zeroize();
                        wipe(value);
                    }
                }
                _ => {}
            }
        }
        wipe(std::mem::take(&mut self.0));
    }
}

/// An opaque native-memory archive, deliberately without Debug or Serialize.
/// Accessors borrow data; dropping it wipes owned JSON strings and file buffers.
pub struct OnePuxArchive {
    attributes: SecretJson,
    data: SecretJson,
    files: BTreeMap<String, Zeroizing<Vec<u8>>>,
}

impl OnePuxArchive {
    pub fn attributes(&self) -> &Value {
        &self.attributes.0
    }

    pub fn data(&self) -> &Value {
        &self.data.0
    }

    /// Exact archive paths, not basenames or guessed document identities.
    pub fn files(&self) -> &BTreeMap<String, Zeroizing<Vec<u8>>> {
        &self.files
    }
}

/// Decode version-3 1PUX metadata and binary members into temporary native memory.
/// This does not import items or claim account/vault/field migration completion.
pub fn parse_1pux_archive(bytes: &[u8]) -> Result<OnePuxArchive, String> {
    decode(bytes, LIMITS).map_err(str::to_owned)
}

fn number<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], &'static str> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(INVALID)?)
        .ok_or(INVALID)?
        .try_into()
        .map_err(|_| INVALID)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, &'static str> {
    Ok(u16::from_le_bytes(number(bytes, offset)?))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, &'static str> {
    Ok(u32::from_le_bytes(number(bytes, offset)?))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, &'static str> {
    Ok(u64::from_le_bytes(number(bytes, offset)?))
}

/// Bound the raw central directory BEFORE ZipArchive allocates its entry map.
/// The library deduplicates names, so also validate all raw names independently.
/// ZIP64 is supported for a normal single-disk, non-prefixed export.
fn preflight(bytes: &[u8], limits: Limits) -> Result<(), &'static str> {
    if bytes.len() > limits.compressed {
        return Err(LIMIT);
    }
    let start = bytes.len().saturating_sub(65_557);
    let end = (start..bytes.len().saturating_sub(21))
        .rev()
        .find(|&offset| {
            bytes.get(offset..offset + 4) == Some(b"PK\x05\x06")
                && u16_at(bytes, offset + 20)
                    .ok()
                    .is_some_and(|len| offset + 22 + usize::from(len) == bytes.len())
        })
        .ok_or(INVALID)?;
    if u16_at(bytes, end + 4)? != 0 || u16_at(bytes, end + 6)? != 0 {
        return Err(INVALID);
    }
    let count = u16_at(bytes, end + 10)?;
    let size = u32_at(bytes, end + 12)?;
    let offset = u32_at(bytes, end + 16)?;
    let (count, size, offset, boundary) =
        if count == u16::MAX || size == u32::MAX || offset == u32::MAX {
            let locator = end.checked_sub(20).ok_or(INVALID)?;
            if bytes.get(locator..locator + 4) != Some(b"PK\x06\x07")
                || u32_at(bytes, locator + 4)? != 0
                || u32_at(bytes, locator + 16)? != 1
            {
                return Err(INVALID);
            }
            let zip64 = usize::try_from(u64_at(bytes, locator + 8)?).map_err(|_| INVALID)?;
            if bytes.get(zip64..zip64.checked_add(4).ok_or(INVALID)?) != Some(b"PK\x06\x06")
                || u64_at(bytes, zip64 + 4)? < 44
                || u32_at(bytes, zip64 + 16)? != 0
                || u32_at(bytes, zip64 + 20)? != 0
                || u64_at(bytes, zip64 + 24)? != u64_at(bytes, zip64 + 32)?
            {
                return Err(INVALID);
            }
            let record_end = (zip64 as u64)
                .checked_add(12)
                .and_then(|n| n.checked_add(u64_at(bytes, zip64 + 4).ok()?))
                .ok_or(INVALID)?;
            if record_end != locator as u64 {
                return Err(INVALID);
            }
            (
                u64_at(bytes, zip64 + 32)?,
                u64_at(bytes, zip64 + 40)?,
                u64_at(bytes, zip64 + 48)?,
                zip64,
            )
        } else {
            if u16_at(bytes, end + 8)? != count {
                return Err(INVALID);
            }
            (u64::from(count), u64::from(size), u64::from(offset), end)
        };
    if count > limits.members as u64 {
        return Err(LIMIT);
    }
    let mut cursor = usize::try_from(offset).map_err(|_| INVALID)?;
    let finish = usize::try_from(offset.checked_add(size).ok_or(INVALID)?).map_err(|_| INVALID)?;
    if finish != boundary {
        return Err(INVALID);
    }
    let mut names = HashSet::new();
    for _ in 0..count {
        if bytes.get(cursor..cursor.checked_add(4).ok_or(INVALID)?) != Some(b"PK\x01\x02") {
            return Err(INVALID);
        }
        let name_length = usize::from(u16_at(bytes, cursor + 28)?);
        let extra = usize::from(u16_at(bytes, cursor + 30)?);
        let comment = usize::from(u16_at(bytes, cursor + 32)?);
        if u16_at(bytes, cursor + 34)? != 0 {
            return Err(INVALID);
        }
        let name_start = cursor.checked_add(46).ok_or(INVALID)?;
        let name_end = name_start.checked_add(name_length).ok_or(INVALID)?;
        let name = bytes.get(name_start..name_end).ok_or(INVALID)?;
        validate_name(name)?;
        if !names.insert(name) {
            return Err(INVALID);
        }
        cursor = name_end
            .checked_add(extra)
            .and_then(|n| n.checked_add(comment))
            .ok_or(INVALID)?;
        if cursor > finish {
            return Err(INVALID);
        }
    }
    if cursor != finish {
        return Err(INVALID);
    }
    Ok(())
}

fn validate_name(raw: &[u8]) -> Result<&str, &'static str> {
    let name = std::str::from_utf8(raw).map_err(|_| INVALID)?;
    if name.is_empty()
        || name.len() > 1024
        || name.chars().any(|ch| ch.is_control() || ch == '\\')
        || name.starts_with('/')
        || name
            .strip_suffix('/')
            .unwrap_or(name)
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(INVALID);
    }
    if name == "export.attributes" || name == "export.data" || name == "files/" {
        return Ok(name);
    }
    if name.starts_with("files/") {
        return Ok(name);
    }
    Err(INVALID)
}

fn decode(bytes: &[u8], limits: Limits) -> Result<OnePuxArchive, &'static str> {
    preflight(bytes, limits)?;
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|_| INVALID)?;
    if zip.has_duplicate_names() || zip.has_overlapping_entries().map_err(|_| INVALID)? {
        return Err(INVALID);
    }
    let mut members = BTreeMap::new();
    let mut total = 0usize;
    for index in 0..zip.len() {
        let mut member = zip.by_index(index).map_err(|_| INVALID)?;
        let name = validate_name(member.name_raw())?.to_owned();
        if member.encrypted()
            || member.is_symlink()
            || member.unix_mode().is_some_and(|mode| {
                let kind = mode & 0o170000;
                kind != 0 && kind != 0o100000 && kind != 0o040000
            })
            || !matches!(
                member.compression(),
                CompressionMethod::Stored | CompressionMethod::Deflated
            )
        {
            return Err(INVALID);
        }
        let directory = member.is_dir();
        if directory && (!name.starts_with("files/") || member.size() != 0) {
            return Err(INVALID);
        }
        let cap = if directory {
            0
        } else if name.starts_with("export.") {
            limits.json
        } else {
            limits.member
        };
        let declared = usize::try_from(member.size()).map_err(|_| LIMIT)?;
        let remaining = limits.expanded.checked_sub(total).ok_or(LIMIT)?;
        if declared > cap || declared > remaining {
            return Err(LIMIT);
        }
        let mut content = Zeroizing::new(Vec::new());
        member
            .by_ref()
            .take(cap.min(remaining) as u64 + 1)
            .read_to_end(&mut content)
            .map_err(|_| INVALID)?;
        if content.len() > cap || content.len() > remaining {
            return Err(LIMIT);
        }
        if content.len() != declared {
            return Err(INVALID);
        }
        total += content.len();
        if !directory {
            members.insert(name, content);
        }
    }
    let attributes = members.remove("export.attributes").ok_or(INVALID)?;
    let data = members.remove("export.data").ok_or(INVALID)?;
    let attributes: SecretJson = serde_json::from_slice(&attributes).map_err(|_| INVALID)?;
    let data: SecretJson = serde_json::from_slice(&data).map_err(|_| INVALID)?;
    if attributes.0.get("version").and_then(Value::as_u64) != Some(3)
        || !data.0.is_object()
        || !data.0.get("accounts").is_some_and(Value::is_array)
    {
        return Err(INVALID);
    }
    Ok(OnePuxArchive {
        attributes,
        data,
        files: members,
    })
}

#[cfg(test)]
#[path = "onepux_archive_tests.rs"]
mod tests;
