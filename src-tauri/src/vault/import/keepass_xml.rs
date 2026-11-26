//! Bounded native PlainXml preparation. Plaintext never becomes renderer rows.
use super::tree::SourceArchive;
use crate::vault::{ValidatedWebsiteRules, VaultSession, VaultState};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use chrono::{Datelike, NaiveDate, NaiveDateTime};
use keepass::db::{Database, Value};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
};
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};

const INVALID: &str = "KeePass XML 内容无效、身份重复或引用缺失；未导入任何内容";
const LIMIT: &str = "KeePass XML 超过资源上限；未导入任何内容";
const ENCRYPTED: &str = "仅支持明文 KeePass XML；请从 KeePass 导出未加密 XML";
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_BINARY: usize = 256 * 1024 * 1024;

#[derive(Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeePassXmlCounts {
    pub groups: usize,
    pub items: usize,
    pub history: usize,
    pub binaries: usize,
}

/// No Debug/Serialize: prepared native secrets are not an IPC document.
pub struct KeePassXmlImport {
    database: Database,
    archive: Value<String>,
    counts: KeePassXmlCounts,
}

impl KeePassXmlImport {
    pub fn counts(&self) -> KeePassXmlCounts {
        self.counts
    }
}

struct Node {
    name: String,
    attrs: HashMap<String, Zeroizing<String>>,
    text: Zeroizing<String>,
    children: Vec<Node>,
}

impl Node {
    fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            attrs: HashMap::new(),
            text: Zeroizing::new(String::new()),
            children: Vec::new(),
        }
    }
    fn one(&self, name: &str) -> Option<&Node> {
        self.children.iter().find(|n| n.name == name)
    }
    fn one_mut(&mut self, name: &str) -> Option<&mut Node> {
        self.children.iter_mut().find(|n| n.name == name)
    }
    fn required(&self, name: &str) -> Result<&Node, String> {
        self.one(name).ok_or_else(|| INVALID.into())
    }
    fn leaf(&self) -> Result<&str, String> {
        if self.children.is_empty() {
            Ok(&self.text)
        } else {
            Err(INVALID.into())
        }
    }
    fn put(&mut self, name: &str, text: String) {
        if let Some(node) = self.one_mut(name) {
            *node.text = text;
        } else {
            let mut node = Node::new(name);
            *node.text = text;
            self.children.push(node);
        }
    }
}

fn scalar(ch: char) -> bool {
    matches!(ch, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}

fn xml_name(name: &str) -> bool {
    fn start(ch: char) -> bool {
        matches!(ch, ':' | '_' | 'A'..='Z' | 'a'..='z' | '\u{c0}'..='\u{d6}' | '\u{d8}'..='\u{f6}' | '\u{f8}'..='\u{2ff}' | '\u{370}'..='\u{37d}' | '\u{37f}'..='\u{1fff}' | '\u{200c}'..='\u{200d}' | '\u{2070}'..='\u{218f}' | '\u{2c00}'..='\u{2fef}' | '\u{3001}'..='\u{d7ff}' | '\u{f900}'..='\u{fdcf}' | '\u{fdf0}'..='\u{fffd}' | '\u{10000}'..='\u{effff}')
    }
    let mut chars = name.chars();
    chars.next().is_some_and(start) && chars.all(|ch| start(ch) || matches!(ch, '-' | '.' | '0'..='9' | '\u{b7}' | '\u{300}'..='\u{36f}' | '\u{203f}'..='\u{2040}'))
}

fn source_text(bytes: &[u8]) -> Result<Zeroizing<String>, String> {
    if bytes.len() > MAX_BYTES {
        return Err(LIMIT.into());
    }
    let text = if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        let little = bytes[0] == 0xff;
        if !bytes.len().is_multiple_of(2) {
            return Err(INVALID.into());
        }
        let units = Zeroizing::new(
            bytes[2..]
                .chunks_exact(2)
                .map(|b| {
                    if little {
                        u16::from_le_bytes([b[0], b[1]])
                    } else {
                        u16::from_be_bytes([b[0], b[1]])
                    }
                })
                .collect::<Vec<_>>(),
        );
        String::from_utf16(&units).map_err(|_| INVALID)?
    } else {
        std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
            .map_err(|_| INVALID)?
            .to_owned()
    };
    let text = Zeroizing::new(text);
    if text.len() > MAX_BYTES {
        return Err(LIMIT.into());
    }
    if text.chars().any(|ch| !scalar(ch)) {
        return Err(INVALID.into());
    }
    Ok(text)
}

fn repeated(parent: &str, child: &str) -> bool {
    matches!(
        (parent, child),
        ("Group", "Group" | "Entry")
            | ("Entry", "String" | "Binary")
            | ("History", "Entry")
            | ("CustomData", "Item")
            | ("CustomIcons", "Icon")
            | ("Binaries", "Binary")
            | ("DeletedObjects", "DeletedObject")
            | ("AutoType", "Association")
    )
}

fn attach(stack: &mut [Node], node: Node) -> Result<(), String> {
    let parent = stack.last_mut().ok_or(INVALID)?;
    if !repeated(&parent.name, &node.name) && parent.one(&node.name).is_some() {
        return Err(INVALID.into());
    }
    if !node.children.is_empty() && !node.text.trim().is_empty() {
        return Err(INVALID.into());
    }
    parent.children.push(node);
    Ok(())
}

fn read_tree(text: &str, utf16: bool) -> Result<Node, String> {
    // XML normalizes literal CR, but character references remain exact.
    let normalized = Zeroizing::new(text.replace("\r\n", "\n").replace('\r', "\n"));
    let mut reader = quick_xml::Reader::from_str(&normalized);
    let mut stack = vec![Node::new("Document")];
    let mut nodes = 0;
    let mut declared = false;
    loop {
        match reader.read_event().map_err(|_| INVALID)? {
            event @ (Event::Start(_) | Event::Empty(_)) => {
                let empty = matches!(event, Event::Empty(_));
                let start = match event {
                    Event::Start(s) | Event::Empty(s) => s,
                    _ => unreachable!(),
                };
                nodes += 1;
                if nodes > 262_144 || stack.len() >= 64 {
                    return Err(LIMIT.into());
                }
                let name = std::str::from_utf8(start.name().as_ref())
                    .map_err(|_| INVALID)?
                    .to_owned();
                if !xml_name(&name) || name.contains(':') {
                    return Err(INVALID.into());
                }
                let mut node = Node::new(&name);
                for attr in start.attributes() {
                    let attr = attr.map_err(|_| INVALID)?;
                    let key = std::str::from_utf8(attr.key.as_ref())
                        .map_err(|_| INVALID)?
                        .to_owned();
                    if !xml_name(&key) {
                        return Err(INVALID.into());
                    }
                    let value = Zeroizing::new(
                        attr.decoded_and_normalized_value(
                            quick_xml::XmlVersion::Implicit1_0,
                            reader.decoder(),
                        )
                        .map_err(|_| INVALID)?
                        .into_owned(),
                    );
                    if value.chars().any(|ch| !scalar(ch))
                        || node.attrs.insert(key, value).is_some()
                    {
                        return Err(INVALID.into());
                    }
                }
                if empty {
                    attach(&mut stack, node)?;
                } else {
                    stack.push(node);
                }
            }
            Event::End(_) => {
                if stack.len() < 2 {
                    return Err(INVALID.into());
                }
                let node = stack.pop().ok_or(INVALID)?;
                attach(&mut stack, node)?;
            }
            Event::Text(value) => {
                let decoded = Zeroizing::new(
                    value
                        .xml_content(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(|_| INVALID)?
                        .into_owned(),
                );
                let text = Zeroizing::new(
                    quick_xml::escape::unescape(&decoded)
                        .map_err(|_| INVALID)?
                        .into_owned(),
                );
                if text.chars().any(|ch| !scalar(ch)) {
                    return Err(INVALID.into());
                }
                stack.last_mut().ok_or(INVALID)?.text.push_str(&text);
            }
            Event::CData(value) => {
                let text = Zeroizing::new(
                    value
                        .xml_content(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(|_| INVALID)?
                        .into_owned(),
                );
                if stack.len() < 2 || text.chars().any(|ch| !scalar(ch)) {
                    return Err(INVALID.into());
                }
                stack.last_mut().ok_or(INVALID)?.text.push_str(&text);
            }
            Event::GeneralRef(value) => {
                let name = value.decode().map_err(|_| INVALID)?;
                let entity = Zeroizing::new(format!("&{name};"));
                let text = Zeroizing::new(
                    quick_xml::escape::unescape(&entity)
                        .map_err(|_| INVALID)?
                        .into_owned(),
                );
                if text.chars().any(|ch| !scalar(ch)) {
                    return Err(INVALID.into());
                }
                stack.last_mut().ok_or(INVALID)?.text.push_str(&text);
            }
            Event::Decl(value) => {
                if declared
                    || stack.len() != 1
                    || !stack[0].children.is_empty()
                    || !stack[0].text.is_empty()
                {
                    return Err(INVALID.into());
                }
                declared = true;
                if value.version().map_err(|_| INVALID)?.as_ref() != b"1.0" {
                    return Err(INVALID.into());
                }
                if let Some(encoding) = value.encoding() {
                    let encoding = encoding.map_err(|_| INVALID)?;
                    if (utf16 && !encoding.eq_ignore_ascii_case(b"utf-16"))
                        || (!utf16 && !encoding.eq_ignore_ascii_case(b"utf-8"))
                    {
                        return Err(INVALID.into());
                    }
                }
            }
            Event::DocType(_) | Event::PI(_) => return Err(INVALID.into()),
            Event::Comment(_) => {}
            Event::Eof => break,
        }
    }
    if stack.len() != 1 {
        return Err(INVALID.into());
    }
    let mut document = stack.pop().ok_or(INVALID)?;
    if !document.text.trim().is_empty() || document.children.len() != 1 {
        return Err(INVALID.into());
    }
    let root = document.children.pop().ok_or(INVALID)?;
    if root.name != "KeePassFile"
        || root
            .children
            .iter()
            .any(|n| !matches!(n.name.as_str(), "Meta" | "Root"))
    {
        return Err(INVALID.into());
    }
    Ok(root)
}

fn decode(text: &str) -> Result<Zeroizing<Vec<u8>>, String> {
    let compact = Zeroizing::new(
        text.chars()
            .filter(|ch| !ch.is_ascii_whitespace())
            .collect::<String>(),
    );
    STANDARD
        .decode(compact.as_bytes())
        .map(Zeroizing::new)
        .map_err(|_| INVALID.into())
}

fn id(text: &str) -> Result<Uuid, String> {
    let bytes = decode(text)?;
    Uuid::from_slice(&bytes).map_err(|_| INVALID.into())
}

fn flag(value: Option<&Zeroizing<String>>) -> Result<bool, String> {
    match value.map(|s| s.as_str()) {
        None => Ok(false),
        Some(v) if v.eq_ignore_ascii_case("True") => Ok(true),
        Some(v) if v.eq_ignore_ascii_case("False") => Ok(false),
        _ => Err(INVALID.into()),
    }
}

fn number(text: &str) -> Result<usize, String> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(INVALID.into());
    }
    text.parse().map_err(|_| INVALID.into())
}

fn normalize_dates(node: &mut Node, parent: &str) -> Result<(), String> {
    let date = matches!(
        (parent, node.name.as_str()),
        (
            "Times",
            "CreationTime"
                | "LastModificationTime"
                | "LastAccessTime"
                | "ExpiryTime"
                | "LocationChanged"
        ) | (
            "Meta",
            "DatabaseNameChanged"
                | "DatabaseDescriptionChanged"
                | "DefaultUserNameChanged"
                | "MasterKeyChanged"
                | "RecycleBinChanged"
                | "EntryTemplatesGroupChanged"
                | "SettingsChanged"
        ) | ("DeletedObject", "DeletionTime")
            | ("Item" | "Icon", "LastModificationTime")
    );
    if date && !node.leaf()?.trim().is_empty() {
        let text = node.text.trim();
        let time = if let Ok(time) = chrono::DateTime::parse_from_rfc3339(text) {
            time.naive_utc()
        } else if let Ok(time) = NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f") {
            time
        } else {
            let bytes: [u8; 8] = decode(text)?.as_slice().try_into().map_err(|_| INVALID)?;
            NaiveDate::from_ymd_opt(1, 1, 1)
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .and_then(|epoch| {
                    chrono::Duration::try_seconds(i64::from_le_bytes(bytes))
                        .and_then(|d| epoch.checked_add_signed(d))
                })
                .ok_or(INVALID)?
        };
        if !(1..=9999).contains(&time.year()) {
            return Err(INVALID.into());
        }
        *node.text = time.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    }
    if node.name == "CustomData" {
        let mut keys = HashSet::new();
        for item in &node.children {
            if item.name != "Item" || !keys.insert(item.required("Key")?.leaf()?) {
                return Err(INVALID.into());
            }
            item.required("Value")?.leaf()?;
        }
    }
    let name = node.name.clone();
    for child in &mut node.children {
        normalize_dates(child, &name)?;
    }
    Ok(())
}

#[derive(Default)]
struct Preparation {
    counts: KeePassXmlCounts,
    identities: HashSet<Uuid>,
    icons: HashSet<Uuid>,
    binary_ids: HashMap<usize, usize>,
    binaries: Vec<Value<Vec<u8>>>,
    expanded: usize,
    fields: usize,
    legacy: bool,
}

impl Preparation {
    fn binary(&mut self, node: &Node) -> Result<usize, String> {
        if flag(node.attrs.get("Protected"))? {
            return Err(ENCRYPTED.into());
        }
        let mut bytes = decode(node.leaf()?)?;
        if flag(node.attrs.get("Compressed"))? {
            let mut inflated = Zeroizing::new(Vec::new());
            let available = MAX_BINARY.saturating_sub(self.expanded).min(MAX_BYTES);
            let mut gzip =
                flate2::read::MultiGzDecoder::new(bytes.as_slice()).take(available as u64 + 1);
            gzip.read_to_end(&mut inflated).map_err(|_| INVALID)?;
            bytes = inflated;
        }
        self.expanded = self
            .expanded
            .checked_add(bytes.len())
            .filter(|n| *n <= MAX_BINARY)
            .ok_or(LIMIT)?;
        if bytes.len() > MAX_BYTES || self.binaries.len() >= 8192 {
            return Err(LIMIT.into());
        }
        let index = self.binaries.len();
        self.binaries
            .push(Value::protected(std::mem::take(&mut *bytes)));
        self.counts.binaries += 1;
        Ok(index)
    }
    fn identity(&mut self, node: &mut Node, historical: Option<Uuid>) -> Result<Uuid, String> {
        let current = match node.one("UUID") {
            Some(node) if !node.leaf()?.trim().is_empty() => id(node.leaf()?)?,
            _ => historical.unwrap_or_else(Uuid::new_v4),
        };
        if current.is_nil()
            || historical.is_some_and(|owner| owner != current)
            || (historical.is_none() && !self.identities.insert(current))
        {
            return Err(INVALID.into());
        }
        node.put("UUID", STANDARD.encode(current.as_bytes()));
        Ok(current)
    }
    fn icon(&self, node: &mut Node) -> Result<(), String> {
        if let Some(icon) = node.one("CustomIconUUID") {
            let icon = id(icon.leaf()?)?;
            if icon.is_nil() {
                node.children.retain(|n| n.name != "CustomIconUUID");
            } else if !self.icons.contains(&icon) {
                return Err(INVALID.into());
            }
        }
        Ok(())
    }
    fn entry(&mut self, node: &mut Node, historical: Option<Uuid>) -> Result<(), String> {
        let owner = self.identity(node, historical)?;
        self.icon(node)?;
        if let Some(auto_type) = node.one_mut("AutoType") {
            if auto_type
                .one("Enabled")
                .map(|n| n.leaf())
                .transpose()?
                .is_none_or(|v| v.trim().is_empty())
            {
                auto_type.put("Enabled", "True".into());
            }
        }
        let count = if historical.is_some() {
            &mut self.counts.history
        } else {
            &mut self.counts.items
        };
        *count += 1;
        if self.counts.items > 8192 || self.counts.history > 16384 {
            return Err(LIMIT.into());
        }
        let mut names = HashSet::new();
        let mut files = HashSet::new();
        let override_url = node
            .one("OverrideURL")
            .map(|n| n.leaf())
            .transpose()?
            .unwrap_or("")
            .trim()
            .to_owned();
        for child in &mut node.children {
            match child.name.as_str() {
                "String" => {
                    self.fields += 1;
                    if self.fields > 65536 {
                        return Err(LIMIT.into());
                    }
                    let name = child.required("Key")?.leaf()?.to_owned();
                    if !names.insert(name.clone()) {
                        return Err(INVALID.into());
                    }
                    let value = child.one_mut("Value").ok_or(INVALID)?;
                    value.leaf()?;
                    let protected = flag(value.attrs.get("Protected"))?;
                    let literal = flag(value.attrs.get("ProtectInMemory"))?;
                    if protected && (!self.legacy || value.attrs.contains_key("ProtectInMemory")) {
                        return Err(ENCRYPTED.into());
                    }
                    if protected {
                        let decoded = decode(&value.text)?;
                        let plaintext = std::str::from_utf8(&decoded).map_err(|_| INVALID)?;
                        value.text.zeroize();
                        value.text.push_str(plaintext);
                    }
                    if name == "SecPivot.WebsiteRules" {
                        ValidatedWebsiteRules::from_json(&value.text).map_err(|_| INVALID)?;
                        if !override_url.is_empty() {
                            return Err(INVALID.into());
                        }
                    }
                    let protect = protected || literal || name == "SecPivot.WebsiteRules";
                    value.attrs.remove("ProtectInMemory");
                    value.attrs.remove("Protected");
                    if protect {
                        let encoded = STANDARD.encode(value.text.as_bytes());
                        value.text.zeroize();
                        *value.text = encoded;
                        value
                            .attrs
                            .insert("Protected".into(), Zeroizing::new("True".into()));
                    }
                }
                "Binary" => {
                    if !files.insert(child.required("Key")?.leaf()?.to_owned()) {
                        return Err(INVALID.into());
                    }
                    let value = child.one_mut("Value").ok_or(INVALID)?;
                    let binary = if let Some(reference) = value.attrs.get("Ref") {
                        if !value.leaf()?.trim().is_empty()
                            || flag(value.attrs.get("Protected"))?
                            || flag(value.attrs.get("Compressed"))?
                        {
                            return Err(INVALID.into());
                        }
                        *self.binary_ids.get(&number(reference)?).ok_or(INVALID)?
                    } else {
                        self.binary(value)?
                    };
                    value.text.zeroize();
                    value.attrs.clear();
                    value
                        .attrs
                        .insert("Ref".into(), Zeroizing::new(binary.to_string()));
                }
                "History" => {
                    if historical.is_some() {
                        return Err(INVALID.into());
                    }
                    for old in &mut child.children {
                        if old.name != "Entry" {
                            return Err(INVALID.into());
                        }
                        self.entry(old, Some(owner))?;
                    }
                }
                "Entry" | "Group" => return Err(INVALID.into()),
                _ => {}
            }
        }
        Ok(())
    }
    fn group(&mut self, node: &mut Node) -> Result<(), String> {
        self.identity(node, None)?;
        self.icon(node)?;
        node.required("Name")?.leaf()?;
        // KeePass groups default expanded when old PlainXml omits the flag.
        if node
            .one("IsExpanded")
            .map(|n| n.leaf())
            .transpose()?
            .is_none_or(|v| v.trim().is_empty())
        {
            node.put("IsExpanded", "True".into());
        }
        self.counts.groups += 1;
        if self.counts.groups > 8192 {
            return Err(LIMIT.into());
        }
        for child in &mut node.children {
            match child.name.as_str() {
                "Group" => self.group(child)?,
                "Entry" => self.entry(child, None)?,
                "History" => return Err(INVALID.into()),
                _ => {}
            }
        }
        Ok(())
    }
}

fn write_node(node: &Node, writer: &mut quick_xml::Writer<&mut Vec<u8>>) -> Result<(), String> {
    let mut start = BytesStart::new(&node.name);
    for (key, value) in &node.attrs {
        start.push_attribute((key.as_str(), value.as_str()));
    }
    writer
        .write_event(Event::Start(start))
        .map_err(|_| INVALID)?;
    if !node.text.is_empty() {
        let escaped =
            Zeroizing::new(quick_xml::escape::escape(node.text.as_str()).replace('\r', "&#xD;"));
        writer
            .write_event(Event::Text(BytesText::from_escaped(escaped.as_str())))
            .map_err(|_| INVALID)?;
    }
    for child in &node.children {
        write_node(child, writer)?;
    }
    writer
        .write_event(Event::End(BytesEnd::new(&node.name)))
        .map_err(|_| INVALID)?;
    Ok(())
}

pub fn prepare_keepass_xml_import(bytes: &[u8]) -> Result<KeePassXmlImport, String> {
    let mut text = source_text(bytes)?;
    let mut xml = read_tree(
        &text,
        bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]),
    )?;
    let mut prep = Preparation::default();
    let meta = xml.one_mut("Meta").ok_or(INVALID)?;
    prep.legacy = meta.children.len() == 1
        && meta.one("Generator").map(|n| n.leaf()).transpose()? == Some("SecPivot");
    if let Some(pool) = meta.one("Binaries") {
        for binary in &pool.children {
            if binary.name != "Binary" {
                return Err(INVALID.into());
            }
            let source = number(binary.attrs.get("ID").ok_or(INVALID)?)?;
            if prep.binary_ids.contains_key(&source) {
                return Err(INVALID.into());
            }
            let dense = prep.binary(binary)?;
            prep.binary_ids.insert(source, dense);
        }
    }
    meta.children.retain(|n| n.name != "Binaries");
    if let Some(icons) = meta.one("CustomIcons") {
        for icon in &icons.children {
            if icon.name != "Icon" {
                return Err(INVALID.into());
            }
            let identity = id(icon.required("UUID")?.leaf()?)?;
            if identity.is_nil() || !prep.icons.insert(identity) || prep.icons.len() > 8192 {
                return Err(INVALID.into());
            }
            let data = decode(icon.required("Data")?.leaf()?)?;
            prep.expanded = prep
                .expanded
                .checked_add(data.len())
                .filter(|n| *n <= MAX_BINARY)
                .ok_or(LIMIT)?;
            if data.len() > MAX_BYTES {
                return Err(LIMIT.into());
            }
        }
    }
    normalize_dates(&mut xml, "Document")?;
    let root = xml.one_mut("Root").ok_or(INVALID)?;
    if root
        .children
        .iter()
        .any(|n| !matches!(n.name.as_str(), "Group" | "DeletedObjects"))
    {
        return Err(INVALID.into());
    }
    if let Some(deleted) = root.one("DeletedObjects") {
        let mut identities = HashSet::new();
        for object in &deleted.children {
            if object.name != "DeletedObject"
                || !identities.insert(id(object.required("UUID")?.leaf()?)?)
            {
                return Err(INVALID.into());
            }
        }
    }
    prep.group(root.one_mut("Group").ok_or(INVALID)?)?;
    let mut normalized = Zeroizing::new(Vec::new());
    write_node(&xml, &mut quick_xml::Writer::new(&mut normalized))?;
    let database =
        Database::from_unencrypted_inner_xml(&normalized, &prep.binaries).map_err(|_| INVALID)?;
    let archive = Value::protected(std::mem::take(&mut *text));
    Ok(KeePassXmlImport {
        database,
        archive,
        counts: prep.counts,
    })
}

impl VaultSession {
    pub(crate) fn xml_import_lifetime(&self, target: &str) -> Result<Uuid, String> {
        self.native_import_lifetime(target)
    }
    pub(crate) fn import_keepass_xml(
        &mut self,
        plan: KeePassXmlImport,
        target: &str,
        lifetime: Uuid,
    ) -> Result<VaultState, String> {
        self.import_prepared_tree(
            plan.database,
            SourceArchive {
                title: "KeePass XML 导入来源",
                key: "KeePass Source XML",
                value: plan.archive,
                literal_fields: false,
            },
            target,
            lifetime,
        )
    }
}

#[cfg(test)]
#[path = "keepass_xml_tests.rs"]
mod tests;
