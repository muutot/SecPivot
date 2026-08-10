//! WebDAV transport over blocking reqwest + quick_xml multistatus parsing
//! (extracted from remote/mod.rs).

use super::RemoteObject;
use super::RemoteStorage;
use super::{REMOTE_CONNECT_TIMEOUT, REMOTE_IO_TIMEOUT, REMOTE_LIST_TIMEOUT};
use crate::config::RemoteSettings;
use quick_xml::events::Event;
use quick_xml::Reader;
use std::sync::OnceLock;
use std::time::Duration;
use url::Url;
// ---------------------------------------------------------------------------
// WebDAV transport (reqwest blocking + PROPFIND multistatus parsing)
// ---------------------------------------------------------------------------

/// `propfind` request body: ask only for size + last-modified so collections
/// (which carry no `getcontentlength`) are distinguishable from files.
const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:getcontentlength/>
    <d:getlastmodified/>
  </d:prop>
</d:propfind>"#;

/// One process-wide shared reqwest blocking client. The client owns a tokio
/// runtime that must never be dropped from an async context (dropping it on a
/// runtime worker panics — e.g. when a vault session closes); keeping the
/// original alive forever means per-storage clones never tear the runtime down.
///
/// `reqwest::blocking::Client::build` also *blocks* (`wait::enter`), so it must
/// run off any async worker or it panics on first use — hence a dedicated
/// init thread for the one-time construction.
fn shared_blocking_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        std::thread::Builder::new()
            .name("webdav-client-init".into())
            .spawn(|| {
                reqwest::blocking::Client::builder()
                    .connect_timeout(REMOTE_CONNECT_TIMEOUT)
                    .build()
                    .expect("无法初始化 WebDAV 客户端: reqwest 资源不足")
            })
            .expect("spawn webdav client init thread")
            .join()
            .expect("webdav client init thread panicked")
    })
}

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
        let endpoint = cfg.webdav.endpoint.trim();
        if endpoint.is_empty() {
            return Err("请先在设置中配置 WebDAV 服务地址".to_owned());
        }
        let base_url = endpoint.trim_end_matches('/').to_owned();
        let username = cfg.webdav.access_key.trim();
        // Skip Basic auth entirely when no credentials are configured (public
        // servers); empty `Basic ` headers can trip up some implementations.
        let auth = if username.is_empty() {
            None
        } else {
            Some((username.to_owned(), cfg.webdav.secret_key.clone()))
        };
        Ok(Self {
            client: shared_blocking_client().clone(),
            auth,
            base_url,
            list_timeout,
            io_timeout,
        })
    }

    /// Absolute URL for a vault key (or, with an empty key, the base URL).
    /// Rejects `.`/`..` path segments so a key cannot escape the endpoint.
    pub(crate) fn url_for(&self, path: &str) -> Result<String, String> {
        let mut url = self.base_url.clone();
        for segment in path.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                continue;
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
        let url = self.url_for(key)?;
        let mut builder = self.client.get(&url).timeout(self.io_timeout);
        if let Some((user, pass)) = &self.auth {
            builder = builder.basic_auth(user.clone(), Some(pass.clone()));
        }
        let response = builder
            .send()
            .map_err(|e| format!("WebDAV 下载请求失败: {e}"))?;
        if response.status() == reqwest::StatusCode::OK {
            response
                .bytes()
                .map(|b| b.to_vec())
                .map_err(|e| format!("WebDAV 下载响应读取失败: {e}"))
        } else {
            Err(format!("WebDAV 下载失败: HTTP {}", response.status()))
        }
    }

    fn put(&self, key: &str, data: &[u8]) -> Result<(), String> {
        let url = self.url_for(key)?;
        let mut builder = self
            .client
            .put(&url)
            .header("Content-Type", "application/octet-stream")
            .timeout(self.io_timeout)
            .body(data.to_vec());
        if let Some((user, pass)) = &self.auth {
            builder = builder.basic_auth(user.clone(), Some(pass.clone()));
        }
        let response = builder
            .send()
            .map_err(|e| format!("WebDAV 上传请求失败: {e}"))?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "WebDAV 上传失败: HTTP {}（请确认目录已存在）",
                response.status()
            ))
        }
    }
}

/// Record a text payload against the element currently open (`current`).
/// Element names are matched lowercase (see [`local_name`]).
fn record_prop(
    current: &str,
    href: &mut Option<String>,
    size: &mut Option<usize>,
    modified: &mut Option<String>,
    text: &str,
) {
    match current {
        "href" => *href = Some(text.to_string()),
        "getcontentlength" => *size = text.parse().ok(),
        "getlastmodified" => *modified = Some(text.to_string()),
        _ => {}
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
    if !body.to_ascii_lowercase().contains("multistatus") {
        return Err("WebDAV 响应不是有效的 multistatus 列表，请检查服务地址与对象前缀".to_owned());
    }
    let mut reader = Reader::from_str(body);
    reader.trim_text(true);
    let request_key = href_to_key(base_url, request_url);
    let request_key = request_key.trim_matches('/').to_owned();
    let mut objects = Vec::new();
    let mut in_response = false;
    let mut current = String::new();
    let mut href: Option<String> = None;
    let mut size: Option<usize> = None;
    let mut modified: Option<String> = None;
    let mut collection = false;

    loop {
        match reader.read_event() {
            Err(e) => return Err(format!("WebDAV 列表解析失败: {e}")),
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                current = local_name(&e);
                if current == "response" {
                    in_response = true;
                    href = None;
                    size = None;
                    modified = None;
                    collection = false;
                } else if current == "collection" {
                    collection = true;
                }
            }
            Ok(Event::Empty(e)) => {
                current = local_name(&e);
                if current == "collection" {
                    collection = true;
                }
            }
            Ok(Event::Text(t)) => {
                if in_response {
                    let text = t.unescape().unwrap_or_default().trim().to_owned();
                    record_prop(&current, &mut href, &mut size, &mut modified, &text);
                }
            }
            Ok(Event::CData(t)) => {
                if in_response {
                    let text = String::from_utf8_lossy(t.as_ref()).trim().to_owned();
                    record_prop(&current, &mut href, &mut size, &mut modified, &text);
                }
            }
            Ok(Event::End(e)) => {
                let name = end_local_name(&e);
                if name == "response" {
                    if in_response {
                        if let Some(h) = href.as_deref() {
                            let key = href_to_key(base_url, h);
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
                        in_response = false;
                    }
                } else {
                    current.clear();
                }
            }
            Ok(_) => {}
        }
    }
    Ok(objects)
}

fn local_name(e: &quick_xml::events::BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref())
        .into_owned()
        .to_ascii_lowercase()
}

fn end_local_name(e: &quick_xml::events::BytesEnd) -> String {
    String::from_utf8_lossy(e.local_name().as_ref())
        .into_owned()
        .to_ascii_lowercase()
}

/// Convert a PROPFIND `href` (absolute URL or absolute path) to a vault key
/// relative to the configured base URL path.
pub(crate) fn href_to_key(base_url: &str, href: &str) -> String {
    let base_path = Url::parse(base_url)
        .map(|u| u.path().to_string())
        .unwrap_or_default();
    let path = Url::parse(href)
        .map(|u| u.path().to_string())
        .unwrap_or_else(|_| href.to_string());
    let relative = match path.strip_prefix(base_path.as_str()) {
        Some(rest) => rest.to_string(),
        None => path.as_str().to_string(),
    };
    relative.trim_start_matches('/').to_string()
}
