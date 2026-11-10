//! WebDAV transport over blocking reqwest + quick_xml multistatus parsing
//! (extracted from remote/mod.rs).

use super::shared_blocking_client;
use super::RemoteObject;
use super::RemoteStorage;
use super::{REMOTE_CONFLICT_MARKER, REMOTE_IO_TIMEOUT, REMOTE_LIST_TIMEOUT};
use crate::config::RemoteSettings;
use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::NsReader;
use quick_xml::Reader;
use std::time::Duration;
use url::Url;
// ---------------------------------------------------------------------------
// WebDAV transport (reqwest blocking + PROPFIND multistatus parsing)
// ---------------------------------------------------------------------------

/// Request the type needed to distinguish collections from files. Size and
/// last-modified remain optional metadata, not resource-type indicators.
const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:getcontentlength/>
    <d:getlastmodified/>
    <d:resourcetype/>
  </d:prop>
</d:propfind>"#;

/// WebDAV transport. `endpoint` is the WebDAV base URL, `access_key`/`secret_key`
/// are the Basic-auth username/password, and `prefix` is the folder to list
/// from. Vault keys are URL paths relative to `endpoint` (e.g. `vaults/a.kdbx`).
pub struct WebDavStorage {
    pub(crate) client: reqwest::blocking::Client,
    pub(crate) auth: Option<(String, String)>,
    pub(crate) base_url: String,
    pub(crate) list_timeout: Duration,
    pub(crate) io_timeout: Duration,
}

impl WebDavStorage {
    pub fn new(cfg: &RemoteSettings) -> Result<Self, String> {
        Self::with_timeouts(cfg, REMOTE_LIST_TIMEOUT, REMOTE_IO_TIMEOUT)
    }

    pub(crate) fn with_timeouts(
        cfg: &RemoteSettings,
        list_timeout: Duration,
        io_timeout: Duration,
    ) -> Result<Self, String> {
        let endpoint = cfg.endpoint.trim();
        if endpoint.is_empty() {
            return Err("请先在设置中配置 WebDAV 服务地址".to_owned());
        }
        let base_url = endpoint.trim_end_matches('/').to_owned();
        let username = cfg.access_key.trim();
        // Skip Basic auth entirely when no credentials are configured (public
        // servers); empty `Basic ` headers can trip up some implementations.
        let auth = if username.is_empty() {
            None
        } else {
            Some((username.to_owned(), cfg.secret_key.clone()))
        };
        Ok(Self {
            client: shared_blocking_client()?.clone(),
            auth,
            base_url,
            list_timeout,
            io_timeout,
        })
    }

    /// Absolute URL for a vault key (or, with an empty key, the base URL).
    /// Rejects `.`/`..` path segments so a key can neither escape the endpoint
    /// nor alias another key (`a/../a` must fail, not silently become `a`).
    pub(crate) fn url_for(&self, path: &str) -> Result<String, String> {
        let mut url = self.base_url.clone();
        for segment in path.split('/') {
            if segment.is_empty() {
                continue;
            }
            if segment == "." || segment == ".." {
                return Err("远程键名不得包含 `.` 或 `..` 路径段".to_owned());
            }
            url.push('/');
            url.push_str(&encode_path_segment(segment));
        }
        Ok(url)
    }
}

/// Percent-encode a URL path segment, keeping unreserved + sub-delims intact so
/// the server receives exactly the vault key (spaces etc. stay valid).
pub(crate) fn encode_path_segment(segment: &str) -> String {
    let mut out = String::with_capacity(segment.len());
    for byte in segment.bytes() {
        let keep = byte.is_ascii_alphanumeric()
            || matches!(
                byte,
                b'-' | b'_'
                    | b'.'
                    | b'~'
                    | b'!'
                    | b'$'
                    | b'&'
                    | b'\''
                    | b'('
                    | b')'
                    | b'*'
                    | b'+'
                    | b','
                    | b';'
                    | b'='
                    | b':'
                    | b'@'
            );
        if keep {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

impl RemoteStorage for WebDavStorage {
    fn list(&self, prefix: &str) -> Result<Vec<RemoteObject>, String> {
        // List always targets a collection; a trailing slash is required by
        // several servers (Nextcloud, Apache, cloud gateways) — without it they
        // return just the collection itself instead of Depth:1 children.
        let mut target = self.url_for(prefix.trim_matches('/'))?;
        if !target.ends_with('/') {
            target.push('/');
        }
        let mut builder = self
            .client
            .request(
                reqwest::Method::from_bytes(b"PROPFIND").expect("valid method"),
                &target,
            )
            .header("Depth", "1")
            .timeout(self.list_timeout)
            .body(PROPFIND_BODY);
        if let Some((user, pass)) = &self.auth {
            builder = builder.basic_auth(user.clone(), Some(pass.clone()));
        }
        let response = builder
            .send()
            .map_err(|e| format!("WebDAV 列表请求失败: {e}"))?;
        let status = response.status();
        if !status.is_success() {
            return Err(format!("WebDAV 列表失败: HTTP {status}"));
        }
        let text = response
            .text()
            .map_err(|e| format!("WebDAV 列表响应读取失败: {e}"))?;
        parse_multistatus(&text, &self.base_url, &target)
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, String> {
        self.get_with_etag(key).map(|(data, _)| data)
    }

    fn get_with_etag(&self, key: &str) -> Result<(Vec<u8>, Option<String>), String> {
        let url = self.url_for(key)?;
        let mut builder = self.client.get(&url).timeout(self.io_timeout);
        if let Some((user, pass)) = &self.auth {
            builder = builder.basic_auth(user.clone(), Some(pass.clone()));
        }
        let response = builder
            .send()
            .map_err(|e| format!("WebDAV 下载请求失败: {e}"))?;
        if response.status() == reqwest::StatusCode::OK {
            // ETag support is optional in WebDAV servers; absent → plain put.
            let etag = response
                .headers()
                .get("ETag")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let data = super::read_response_capped(
                response,
                super::MAX_REMOTE_OBJECT_BYTES,
                "WebDAV 下载响应",
            )?;
            Ok((data, etag))
        } else {
            Err(format!("WebDAV 下载失败: HTTP {}", response.status()))
        }
    }

    fn put(&self, key: &str, data: &[u8]) -> Result<(), String> {
        self.put_if_match(key, data, None)
    }

    fn put_if_match(&self, key: &str, data: &[u8], etag: Option<&str>) -> Result<(), String> {
        let url = self.url_for(key)?;
        let mut builder = self
            .client
            .put(&url)
            .header("Content-Type", "application/octet-stream")
            .timeout(self.io_timeout)
            .body(data.to_vec());
        // RFC 7232 conditional upload; servers without ETag support ignore it,
        // degrading gracefully to the previous best-effort behavior.
        if let Some(etag) = etag {
            builder = builder.header("If-Match", etag);
        }
        if let Some((user, pass)) = &self.auth {
            builder = builder.basic_auth(user.clone(), Some(pass.clone()));
        }
        let response = builder
            .send()
            .map_err(|e| format!("WebDAV 上传请求失败: {e}"))?;
        let status = response.status();
        if status.as_u16() == 412 {
            return Err(format!(
                "{REMOTE_CONFLICT_MARKER}远程库已被其他设备修改（条件写入被拒绝 HTTP 412），请选择合并、覆盖远程、下载远程或保留本地"
            ));
        }
        if status.is_success() {
            Ok(())
        } else {
            Err(format!(
                "WebDAV 上传失败: HTTP {status}（请确认目录已存在）"
            ))
        }
    }
}

/// Read a complete simple property before interpreting its value. XML can
/// divide text around references, CDATA, comments or processing instructions.
/// Decode one reference event at a time, never recursively unescape the result.
fn read_prop_text(reader: &mut NsReader<&[u8]>) -> Result<String, String> {
    let invalid = || "WebDAV 列表字段包含无效 XML 内容".to_owned();
    let mut value = String::new();
    loop {
        match reader.read_event().map_err(|_| invalid())? {
            Event::Text(text) => {
                value.push_str(&text.xml10_content().map_err(|_| invalid())?);
            }
            Event::CData(text) => {
                value.push_str(&text.xml10_content().map_err(|_| invalid())?);
            }
            Event::GeneralRef(reference) => {
                let entity = format!("&{};", reference.decode().map_err(|_| invalid())?);
                value.push_str(&quick_xml::escape::unescape(&entity).map_err(|_| invalid())?);
            }
            Event::End(_) => return Ok(value),
            Event::Comment(_) | Event::PI(_) => {}
            _ => return Err(invalid()),
        }
    }
}

fn invalid_multistatus_document() -> String {
    "WebDAV 响应不是完整的 multistatus 文档，请检查服务地址与对象前缀".to_owned()
}

fn root_uses_dav_namespace(namespace: &ResolveResult<'_>) -> Result<bool, String> {
    match namespace {
        ResolveResult::Bound(namespace) if namespace.as_ref() == b"DAV:" => Ok(true),
        // Keep the explicitly separate legacy, entirely unqualified format.
        ResolveResult::Unbound => Ok(false),
        _ => Err(invalid_multistatus_document()),
    }
}

fn namespace_matches(namespace: &ResolveResult<'_>, qualified: bool) -> Result<bool, String> {
    match namespace {
        ResolveResult::Bound(namespace) => Ok(qualified && namespace.as_ref() == b"DAV:"),
        ResolveResult::Unbound => Ok(!qualified),
        ResolveResult::Unknown(_) => Err("WebDAV XML 命名空间前缀无效".to_owned()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DavElement {
    Multistatus,
    Response,
    Propstat,
    Prop,
    ResourceType,
    Collection,
    Href,
    Size,
    Modified,
}

/// A familiar name in another container is still an unknown extension here.
fn expected_element(name: &str, parent: &[DavElement]) -> Option<DavElement> {
    use DavElement::*;
    match (name, parent) {
        ("response", [Multistatus]) => Some(Response),
        ("href", [Multistatus, Response]) => Some(Href),
        ("propstat", [Multistatus, Response]) => Some(Propstat),
        ("prop", [Multistatus, Response, Propstat]) => Some(Prop),
        ("resourcetype", [Multistatus, Response, Propstat, Prop]) => Some(ResourceType),
        ("collection", [Multistatus, Response, Propstat, Prop, ResourceType]) => Some(Collection),
        ("getcontentlength", [Multistatus, Response, Propstat, Prop]) => Some(Size),
        ("getlastmodified", [Multistatus, Response, Propstat, Prop]) => Some(Modified),
        _ => None,
    }
}

/// RFC 4918 section 17 ignores the whole unexpected subtree, including any
/// recognized-looking descendants. Namespace prefixes must still be bound.
fn skip_unknown_element(reader: &mut NsReader<&[u8]>) -> Result<(), String> {
    let mut depth = 1usize;
    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| invalid_multistatus_document())?;
        if matches!(namespace, ResolveResult::Unknown(_)) {
            return Err("WebDAV XML 命名空间前缀无效".to_owned());
        }
        match event {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    return Ok(());
                }
            }
            Event::Eof => return Err(invalid_multistatus_document()),
            _ => {}
        }
    }
}

/// Check the document envelope independently of the property reader, which
/// consumes complete elements. Never publish a partial list or an error page
/// just because its text happens to contain the word `multistatus`.
fn validate_multistatus_document(body: &str) -> Result<(), String> {
    let invalid = invalid_multistatus_document;
    let mut reader = Reader::from_str(body);
    let mut depth = 0usize;
    let mut seen_root = false;
    loop {
        match reader.read_event().map_err(|_| invalid())? {
            Event::Start(element) => {
                if depth == 0 {
                    if seen_root || local_name(&element) != "multistatus" {
                        return Err(invalid());
                    }
                    seen_root = true;
                }
                depth += 1;
            }
            Event::Empty(element) if depth == 0 => {
                if seen_root || local_name(&element) != "multistatus" {
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

/// Parse a PROPFIND `multistatus` body into file objects. A `<response>` is a
/// file when it is not the requested collection and is not a collection
/// (`<resourcetype><collection/></resourcetype>`). `getcontentlength` is used
/// only as a hint — many real servers omit it, and requiring it made every file
/// silently dropped (list would report "未找到"). A body that is not a
/// multistatus at all (e.g. a 200 error page) surfaces an actionable error.
pub(crate) fn parse_multistatus(
    body: &str,
    base_url: &str,
    request_url: &str,
) -> Result<Vec<RemoteObject>, String> {
    let mut reader = NsReader::from_str(body);
    let request_key = href_to_key(base_url, request_url)?;
    let request_key = request_key.trim_matches('/').to_owned();
    let mut objects = Vec::new();
    let mut qualified_root = None;
    let mut path = Vec::new();
    let mut href: Option<String> = None;
    let mut size: Option<usize> = None;
    let mut modified: Option<String> = None;
    let mut collection = false;

    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .map_err(|_| "WebDAV 列表解析失败".to_owned())?;
        match event {
            Event::Eof => break,
            Event::Start(e) => {
                let name = local_name(&e);
                if path.is_empty() {
                    if qualified_root.is_some() || name != "multistatus" {
                        return Err(invalid_multistatus_document());
                    }
                    qualified_root = Some(root_uses_dav_namespace(&namespace)?);
                    path.push(DavElement::Multistatus);
                    continue;
                }
                let matching = namespace_matches(
                    &namespace,
                    qualified_root.ok_or_else(invalid_multistatus_document)?,
                )?;
                let Some(element) = matching.then(|| expected_element(&name, &path)).flatten()
                else {
                    skip_unknown_element(&mut reader)?;
                    continue;
                };
                match element {
                    DavElement::Response => {
                        href = None;
                        size = None;
                        modified = None;
                        collection = false;
                    }
                    DavElement::Href => {
                        href = Some(read_prop_text(&mut reader)?.trim().to_owned());
                        continue;
                    }
                    DavElement::Size => {
                        size = read_prop_text(&mut reader)?.trim().parse().ok();
                        continue;
                    }
                    DavElement::Modified => {
                        let text = read_prop_text(&mut reader)?.trim().to_owned();
                        modified = (!text.is_empty()).then_some(text);
                        continue;
                    }
                    DavElement::Collection => collection = true,
                    _ => {}
                }
                path.push(element);
            }
            Event::Empty(e) => {
                let name = local_name(&e);
                if path.is_empty() {
                    if qualified_root.is_some() || name != "multistatus" {
                        return Err(invalid_multistatus_document());
                    }
                    qualified_root = Some(root_uses_dav_namespace(&namespace)?);
                } else if namespace_matches(
                    &namespace,
                    qualified_root.ok_or_else(invalid_multistatus_document)?,
                )? && expected_element(&name, &path) == Some(DavElement::Collection)
                {
                    collection = true;
                }
            }
            Event::End(_) => {
                namespace_matches(
                    &namespace,
                    qualified_root.ok_or_else(invalid_multistatus_document)?,
                )?;
                let element = path.pop().ok_or_else(invalid_multistatus_document)?;
                if element == DavElement::Response {
                    if let Some(h) = href.as_deref() {
                        let key = href_to_key(base_url, h)?;
                        // Skip the collection itself, sub-collections, and
                        // any href that is not a plain file key.
                        if !collection
                            && !key.is_empty()
                            && !key.trim_matches('/').is_empty()
                            && !h.ends_with('/')
                            && key.trim_matches('/') != request_key
                        {
                            objects.push(RemoteObject {
                                key,
                                size: size.unwrap_or(0),
                                modified: modified.clone(),
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Preserve the more specific simple-property failures while ensuring no
    // objects leave this function before the entire envelope is complete.
    validate_multistatus_document(body)?;
    Ok(objects)
}

fn local_name(e: &quick_xml::events::BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref())
        .into_owned()
        .to_ascii_lowercase()
}

/// Convert a URI path to a logical key, decoding each segment exactly once.
/// Do not use a form decoder (`+` is a literal filename character), nor allow
/// an encoded slash/dot segment to become new path structure.
fn decode_webdav_path(path: &str) -> Result<String, String> {
    if path.contains("//") {
        return Err("WebDAV 文件路径包含不支持的空路径段".to_owned());
    }
    path.split('/')
        .map(|segment| {
            let mut decoded = Vec::with_capacity(segment.len());
            let bytes = segment.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if bytes[i] == b'%' {
                    let byte = bytes
                        .get(i + 1..i + 3)
                        .and_then(|pair| {
                            let high = (pair[0] as char).to_digit(16)?;
                            let low = (pair[1] as char).to_digit(16)?;
                            Some((high * 16 + low) as u8)
                        })
                        .ok_or_else(|| "WebDAV 文件路径的百分号编码无效".to_owned())?;
                    decoded.push(byte);
                    i += 3;
                } else {
                    decoded.push(bytes[i]);
                    i += 1;
                }
            }
            let decoded = String::from_utf8(decoded)
                .map_err(|_| "WebDAV 文件路径不是有效的 UTF-8".to_owned())?;
            if decoded == "."
                || decoded == ".."
                || decoded.contains('/')
                || decoded.chars().any(char::is_control)
            {
                return Err("WebDAV 文件路径包含不支持的路径段".to_owned());
            }
            Ok(decoded)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|segments| segments.join("/"))
}

/// Convert a PROPFIND `href` to a decoded key relative to the configured base.
/// Read the original URI path before URL parsing can normalize dot segments.
/// Absolute href authorities are intentionally not used as request destinations.
pub(crate) fn href_to_key(base_url: &str, href: &str) -> Result<String, String> {
    let base = Url::parse(base_url).map_err(|_| "WebDAV 服务地址无效".to_owned())?;
    let base_path = decode_webdav_path(base.path())?;
    let path = href.split(['?', '#']).next().unwrap_or_default();
    let path = if let Some((_, authority_path)) = path.split_once("://") {
        Url::parse(href).map_err(|_| "WebDAV 文件地址无效".to_owned())?;
        authority_path
            .find('/')
            .map_or("/", |i| &authority_path[i..])
    } else if let Some(authority_path) = path.strip_prefix("//") {
        base.join(href)
            .map_err(|_| "WebDAV 文件地址无效".to_owned())?;
        authority_path
            .find('/')
            .map_or("/", |i| &authority_path[i..])
    } else {
        path
    };
    let path = decode_webdav_path(path)?;
    let base_path = base_path.trim_end_matches('/');
    let relative = if path.starts_with('/') && !base_path.is_empty() {
        path.strip_prefix(base_path)
            .filter(|rest| rest.is_empty() || rest.starts_with('/'))
            .ok_or_else(|| "WebDAV 文件路径不在配置的根路径内".to_owned())?
    } else {
        &path
    };
    Ok(relative.trim_start_matches('/').to_owned())
}
