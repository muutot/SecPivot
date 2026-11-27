//! Strict response decoding and bounded whole-document XML validation.
//! The semantic DAV reader retains its existing field-specific diagnostics.
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE, UTF_8};
use quick_xml::events::Event;
use quick_xml::Reader;

pub(super) const MAX_LIST_BYTES: usize = 8 * 1024 * 1024;
pub(super) const MAX_LIST_OBJECTS: usize = 10_000;
const MAX_XML_EVENTS: usize = 262_144;
const MAX_XML_DEPTH: usize = 64;
const MAX_ELEMENT_ATTRIBUTES: usize = 128;
const MAX_XML_ATTRIBUTES: usize = 262_144;

pub(super) fn limit_error() -> String {
    "WebDAV 列表响应超过资源上限".to_owned()
}

fn invalid_xml() -> String {
    "WebDAV 列表 XML 文档无效".to_owned()
}

fn invalid_encoding() -> String {
    "WebDAV 列表响应编码无效或不受支持".to_owned()
}

fn declared_encoding(bytes: &[u8]) -> Result<Option<&'static Encoding>, String> {
    let mut reader = Reader::from_reader(bytes);
    match reader.read_event() {
        Ok(Event::Decl(declaration)) => declaration
            .encoding()
            .map(|label| {
                let label = label.map_err(|_| invalid_encoding())?;
                Encoding::for_label_no_replacement(&label).ok_or_else(invalid_encoding)
            })
            .transpose(),
        _ => Ok(None),
    }
}

/// BOM/UTF-16 byte signature wins, then HTTP charset, then XML declaration/UTF-8.
/// UTF-16's initial less-than bytes also identify endian order without a BOM.
pub(super) fn decode(bytes: &[u8], content_type: Option<&str>) -> Result<String, String> {
    if bytes.len() > MAX_LIST_BYTES {
        return Err(limit_error());
    }
    let charset = content_type.and_then(|value| {
        value.split(';').skip(1).find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            name.trim()
                .eq_ignore_ascii_case("charset")
                .then(|| value.trim().trim_matches('"'))
        })
    });
    let (encoding, start) = if let Some(bom) = Encoding::for_bom(bytes) {
        bom
    } else if bytes.starts_with(b"\x00<\x00") {
        (UTF_16BE, 0)
    } else if bytes.starts_with(b"<\x00") {
        (UTF_16LE, 0)
    } else if let Some(charset) = charset {
        (
            Encoding::for_label_no_replacement(charset.as_bytes()).ok_or_else(invalid_encoding)?,
            0,
        )
    } else {
        (declared_encoding(bytes)?.unwrap_or(UTF_8), 0)
    };
    let text = encoding
        .decode_without_bom_handling_and_without_replacement(&bytes[start..])
        .ok_or_else(invalid_encoding)?;
    if text.len() > MAX_LIST_BYTES {
        return Err(limit_error());
    }
    Ok(text.into_owned())
}

/// Preflight quotas before allocating semantic fields or the validation tree.
/// Lexical failures defer to the established semantic/envelope diagnostics.
pub(super) fn check_limits(body: &str) -> Result<(), String> {
    if body.len() > MAX_LIST_BYTES {
        return Err(limit_error());
    }
    let mut reader = Reader::from_str(body);
    let mut events = 0usize;
    let mut depth = 0usize;
    let mut attributes = 0usize;
    loop {
        let Ok(event) = reader.read_event() else {
            return Ok(());
        };
        if matches!(event, Event::Eof) {
            return Ok(());
        }
        events += 1;
        if events > MAX_XML_EVENTS {
            return Err(limit_error());
        }
        let starts = matches!(event, Event::Start(_));
        match event {
            Event::Start(element) | Event::Empty(element) => {
                if depth + 1 > MAX_XML_DEPTH {
                    return Err(limit_error());
                }
                let count = element
                    .attributes()
                    .with_checks(false)
                    .take(MAX_ELEMENT_ATTRIBUTES + 1)
                    .count();
                attributes += count;
                if count > MAX_ELEMENT_ATTRIBUTES || attributes > MAX_XML_ATTRIBUTES {
                    return Err(limit_error());
                }
                if starts {
                    depth += 1;
                }
            }
            Event::End(_) => depth = depth.saturating_sub(1),
            Event::DocType(_) => return Err(invalid_xml()),
            Event::Decl(declaration) if declaration.version().ok().as_deref() != Some(b"1.0") => {
                return Err(invalid_xml())
            }
            _ => {}
        }
    }
}

/// No DTD or resolver; parser errors never leave the backend as raw diagnostics.
pub(super) fn validate(body: &str) -> Result<(), String> {
    roxmltree::Document::parse_with_options(
        body,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: MAX_XML_EVENTS as u32,
            entity_resolver: None,
        },
    )
    .map_err(|_| invalid_xml())?;
    // The tree parser substitutes surrogate references and permits reserved PI
    // targets. Check references/PI independently before any objects are returned.
    let legal = |text: &str| {
        text.chars().all(|ch| {
        matches!(ch as u32, 0x9 | 0xa | 0xd | 0x20..=0xd7ff | 0xe000..=0xfffd | 0x10000..=0x10ffff)
    })
    };
    if !legal(body) {
        return Err(invalid_xml());
    }
    let mut reader = Reader::from_str(body);
    loop {
        match reader.read_event().map_err(|_| invalid_xml())? {
            Event::GeneralRef(reference) => {
                let entity = format!("&{};", reference.decode().map_err(|_| invalid_xml())?);
                let decoded = quick_xml::escape::unescape(&entity).map_err(|_| invalid_xml())?;
                if !legal(&decoded) {
                    return Err(invalid_xml());
                }
            }
            Event::Start(element) | Event::Empty(element) => {
                for attribute in element.attributes() {
                    let attribute = attribute.map_err(|_| invalid_xml())?;
                    let decoded = attribute
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(|_| invalid_xml())?;
                    if !legal(&decoded) {
                        return Err(invalid_xml());
                    }
                }
            }
            Event::PI(instruction) if instruction.target().eq_ignore_ascii_case(b"xml") => {
                return Err(invalid_xml());
            }
            Event::DocType(_) => return Err(invalid_xml()),
            Event::Eof => return Ok(()),
            _ => {}
        }
    }
}
