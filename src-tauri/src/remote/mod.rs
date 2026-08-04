//! Remote storage transports shared by the S3 and WebDAV backends: a transport
//! trait plus the real S3 and WebDAV implementations and an in-memory fake used
//! by offline tests. The payloads are ordinary KDBX files; only the transport
//! differs. S3 keys / WebDAV credentials are sent from the frontend config on
//! every command (never cached in the session) and live in `conf/config.json`
//! per the approved security model — they are secondary credentials, distinct
//! from vault master passwords.

pub(crate) mod backup;

use crate::config::RemoteSettings;
use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;
use tauri::Manager;
use url::Url;

/// Prefix used for the display path of remote vaults (`s3://<key>`).
pub const REMOTE_URI_PREFIX: &str = "s3://";

/// TCP connect timeout for remote storage requests (shared). rust-s3's
/// default is 60 s; 15 s keeps an unreachable endpoint failure snappy.
const REMOTE_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Overall bound for the object listing call (small payloads).
const REMOTE_LIST_TIMEOUT: Duration = Duration::from_secs(30);
/// Overall bound for download/upload calls (vault files; generous for slow links).
const REMOTE_IO_TIMEOUT: Duration = Duration::from_secs(120);

/// A single process-wide tokio runtime shared by every S3 transport. Creating
/// a runtime per command would spin up (and tear down) a full thread pool on
/// each list/open/save, which is measurable overhead on hot paths.
fn shared_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("无法初始化 S3 运行时: tokio 资源不足")
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteObject {
    pub key: String,
    pub size: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<String>,
}

/// Transport abstraction so the vault session can be tested without network.
pub trait RemoteStorage: Send + Sync {
    fn list(&self, prefix: &str) -> Result<Vec<RemoteObject>, String>;
    fn get(&self, key: &str) -> Result<Vec<u8>, String>;
    fn put(&self, key: &str, data: &[u8]) -> Result<(), String>;
}

// ---------------------------------------------------------------------------
// S3 transport (rust-s3, path-style so custom endpoints such as MinIO work)
// ---------------------------------------------------------------------------

pub struct S3Storage {
    bucket: s3::Bucket,
    runtime: &'static tokio::runtime::Runtime,
    list_timeout: Duration,
    io_timeout: Duration,
}

impl S3Storage {
    pub fn new(cfg: &RemoteSettings) -> Result<Self, String> {
        Self::with_timeouts(cfg, REMOTE_LIST_TIMEOUT, REMOTE_IO_TIMEOUT)
    }

    fn with_timeouts(
        cfg: &RemoteSettings,
        list_timeout: Duration,
        io_timeout: Duration,
    ) -> Result<Self, String> {
        let endpoint = cfg.endpoint.trim();
        let region = cfg.region.trim();
        let bucket_name = cfg.bucket.trim();
        let access_key = cfg.access_key.trim();
        let secret_key = cfg.secret_key.trim();
        if endpoint.is_empty() {
            return Err("请先在设置中配置 S3 服务地址".to_owned());
        }
        if region.is_empty() {
            return Err("请先在设置中配置 S3 区域".to_owned());
        }
        if bucket_name.is_empty() {
            return Err("请先在设置中配置 S3 存储桶".to_owned());
        }
        // An empty or `/`-containing access key would emit a malformed
        // `X-Amz-Credential` (SigV4 splits it on `/`) and fail with a cryptic
        // AuthorizationQueryParametersError HTTP 400. Fail early with a clear
        // message instead; AWS access keys never contain `/`.
        if access_key.is_empty() {
            return Err("请先在设置中配置 S3 Access Key".to_owned());
        }
        if access_key.contains('/') {
            return Err("S3 Access Key 无效：包含非法字符 `/`".to_owned());
        }
        if secret_key.is_empty() {
            return Err("请先在设置中配置 S3 Secret Key".to_owned());
        }
        let region = s3::Region::Custom {
            region: region.to_owned(),
            endpoint: endpoint.to_owned(),
        };
        let credentials =
            s3::creds::Credentials::new(Some(access_key), Some(secret_key), None, None, None)
                .map_err(|e| format!("S3 凭据无效: {e}"))?;
        let mut bucket = s3::Bucket::new(bucket_name, region, credentials)
            .map_err(|e| format!("S3 配置无效: {e}"))?
            .with_request_timeout(REMOTE_CONNECT_TIMEOUT)
            .map_err(|e| format!("S3 配置无效: {e}"))?;
        bucket.set_path_style();
        Ok(Self {
            bucket,
            runtime: shared_runtime(),
            list_timeout,
            io_timeout,
        })
    }

    fn object_key(key: &str) -> String {
        format!("/{}", key.trim_start_matches('/'))
    }
}

/// Build the transport matching a profile's `kind` (`"s3"` → S3, `"webdav"` →
/// WebDAV). Vault code only ever holds `Arc<dyn RemoteStorage>`, so swapping
/// the backend is isolated to this factory.
pub fn make_storage(cfg: &RemoteSettings) -> Result<Arc<dyn RemoteStorage>, String> {
    match cfg.kind.as_str() {
        "webdav" => Ok(Arc::new(WebDavStorage::new(cfg)?)),
        _ => Ok(Arc::new(S3Storage::new(cfg)?)),
    }
}

/// Command body for `s3_list_objects` (name kept for frontend compatibility):
/// lists all objects under the configured prefix, `.kdbx` files first, then
/// key descending. The transport's sync methods block on their own runtime,
/// which panics when called from an async runtime worker thread (the command
/// future then aborts and the invoke never resolves). Hop to the blocking pool
/// first, exactly like the open/create commands do.
pub async fn list_objects_async(cfg: RemoteSettings) -> Result<Vec<RemoteObject>, String> {
    let storage = make_storage(&cfg)?;
    let prefix = cfg.prefix.clone();
    let mut objects = tokio::task::spawn_blocking(move || storage.list(&prefix))
        .await
        .map_err(|e| format!("远程列表任务异常: {e}"))??;
    objects.sort_by(|a, b| {
        let a_db = a.key.ends_with(".kdbx");
        let b_db = b.key.ends_with(".kdbx");
        b_db.cmp(&a_db).then_with(|| b.key.cmp(&a.key))
    });
    Ok(objects)
}

impl RemoteStorage for S3Storage {
    fn list(&self, prefix: &str) -> Result<Vec<RemoteObject>, String> {
        self.runtime.block_on(async {
            let result = tokio::time::timeout(self.list_timeout, async {
                let results = self
                    .bucket
                    .list(prefix.trim_start_matches('/').to_owned(), None)
                    .await
                    .map_err(|e| format!("S3 列表请求失败: {e}"))?;
                let mut objects = Vec::new();
                for page in results {
                    for item in page.contents {
                        objects.push(RemoteObject {
                            key: item.key.trim_start_matches('/').to_owned(),
                            size: item.size as usize,
                            modified: Some(item.last_modified),
                        });
                    }
                }
                Ok(objects)
            })
            .await;
            result.map_err(|_| "S3 列表请求超时，请检查网络与服务地址".to_owned())?
        })
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, String> {
        self.runtime.block_on(async {
            let result = tokio::time::timeout(self.io_timeout, async {
                let response = self
                    .bucket
                    .get_object(&Self::object_key(key))
                    .await
                    .map_err(|e| format!("S3 下载失败: {e}"))?;
                if response.status_code() != 200 {
                    return Err(format!("S3 下载失败: HTTP {}", response.status_code()));
                }
                Ok(response.to_vec())
            })
            .await;
            result.map_err(|_| "S3 下载超时，请检查网络与服务地址".to_owned())?
        })
    }

    fn put(&self, key: &str, data: &[u8]) -> Result<(), String> {
        self.runtime.block_on(async {
            let result = tokio::time::timeout(self.io_timeout, async {
                let response = self
                    .bucket
                    .put_object_with_content_type(
                        &Self::object_key(key),
                        data,
                        "application/octet-stream",
                    )
                    .await
                    .map_err(|e| format!("S3 上传失败: {e}"))?;
                if response.status_code() != 200 {
                    return Err(format!("S3 上传失败: HTTP {}", response.status_code()));
                }
                Ok(())
            })
            .await;
            result.map_err(|_| "S3 上传超时，请检查网络与服务地址".to_owned())?
        })
    }
}

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
    client: reqwest::blocking::Client,
    auth: Option<(String, String)>,
    base_url: String,
    list_timeout: Duration,
    io_timeout: Duration,
}

impl WebDavStorage {
    pub fn new(cfg: &RemoteSettings) -> Result<Self, String> {
        Self::with_timeouts(cfg, REMOTE_LIST_TIMEOUT, REMOTE_IO_TIMEOUT)
    }

    fn with_timeouts(
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
            client: shared_blocking_client().clone(),
            auth,
            base_url,
            list_timeout,
            io_timeout,
        })
    }

    /// Absolute URL for a vault key (or, with an empty key, the base URL).
    /// Rejects `.`/`..` path segments so a key cannot escape the endpoint.
    fn url_for(&self, path: &str) -> Result<String, String> {
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
fn encode_path_segment(segment: &str) -> String {
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
fn parse_multistatus(
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
fn href_to_key(base_url: &str, href: &str) -> String {
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

// ---------------------------------------------------------------------------
// In-memory fake for offline tests
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct MemoryStorage {
    objects: Arc<RwLock<HashMap<String, Vec<u8>>>>,
}

impl MemoryStorage {
    pub fn seed(&self, key: &str, data: Vec<u8>) {
        self.objects
            .write()
            .expect("storage lock poisoned")
            .insert(key.to_owned(), data);
    }
}

impl RemoteStorage for MemoryStorage {
    fn list(&self, prefix: &str) -> Result<Vec<RemoteObject>, String> {
        let guard = self.objects.read().map_err(|_| "存储锁已损坏".to_owned())?;
        let mut objects: Vec<RemoteObject> = guard
            .iter()
            .filter(|(key, _)| key.starts_with(prefix))
            .map(|(key, data)| RemoteObject {
                key: key.clone(),
                size: data.len(),
                modified: None,
            })
            .collect();
        objects.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(objects)
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, String> {
        let guard = self.objects.read().map_err(|_| "存储锁已损坏".to_owned())?;
        guard
            .get(key)
            .cloned()
            .ok_or_else(|| format!("远程文件不存在: {key}"))
    }

    fn put(&self, key: &str, data: &[u8]) -> Result<(), String> {
        self.objects
            .write()
            .map_err(|_| "存储锁已损坏".to_owned())?
            .insert(key.to_owned(), data.to_vec());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Local mirror helpers ("保存到本地" mode)
// ---------------------------------------------------------------------------

/// Base directory for local copies: `<app_data>/Storage/remote/<profile_name>`.
/// `local_dir` is the remote profile name (the frontend's 远程名/配置名); the
/// name is sanitized so it cannot escape the remote storage tree.
pub fn local_storage_dir(app: &tauri::AppHandle, local_dir: &str) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位应用数据目录: {e}"))?;
    let name = sanitize_dir_name(local_dir);
    Ok(base.join("Storage").join("remote").join(name))
}

/// Sanitize a profile name into a safe folder name: keeps letters/digits
/// (Unicode-aware, so Chinese names survive) plus `-`/`_`; anything else
/// (spaces, `.`, `/`, `\`, `:` …) becomes `_`. Empty/whitespace → `remote`.
fn sanitize_dir_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "remote".to_owned();
    }
    let mut out = String::new();
    for ch in trimmed.chars() {
        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "remote".to_owned()
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_storage_lists_gets_puts() {
        let storage = MemoryStorage::default();
        storage.seed("vaults/a.kdbx", vec![1, 2, 3]);
        storage.seed("notes/b.txt", vec![4]);

        let objects = storage.list("vaults/").unwrap();
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].key, "vaults/a.kdbx");
        assert_eq!(objects[0].size, 3);

        let objects = storage.list("").unwrap();
        assert_eq!(objects.len(), 2);

        assert_eq!(storage.get("vaults/a.kdbx").unwrap(), vec![1, 2, 3]);
        assert!(storage.get("missing.kdbx").is_err());

        storage.put("vaults/b.kdbx", &[9, 9]).unwrap();
        assert_eq!(storage.get("vaults/b.kdbx").unwrap(), vec![9, 9]);
    }

    #[test]
    fn local_dir_name_is_sanitized() {
        assert_eq!(sanitize_dir_name("  "), "remote");
        assert_eq!(sanitize_dir_name("my vaults"), "my_vaults");
        assert_eq!(sanitize_dir_name("..\\..\\evil"), "______evil");
        assert_eq!(sanitize_dir_name("safe-dir_1"), "safe-dir_1");
        // Unicode (Chinese) profile names survive the sanitizer.
        assert_eq!(sanitize_dir_name("阿里云"), "阿里云");
        assert_eq!(sanitize_dir_name("默认 备份"), "默认_备份");
        assert_eq!(sanitize_dir_name("vault/备份"), "vault_备份");
    }

    /// S3 transports must share one process-wide runtime: a fresh thread pool
    /// per command would defeat its purpose. Both calls must resolve to the
    /// exact same instance.
    #[test]
    fn s3_uses_a_single_shared_runtime() {
        let a = shared_runtime() as *const _;
        let b = shared_runtime() as *const _;
        assert!(std::ptr::eq(a, b));
    }

    // -----------------------------------------------------------------------
    // Local mock S3 server: turns the "no live S3 evidence" gap into an
    // offline HTTP-level round trip through the real S3Storage transport
    // (path-style signing, ListObjectsV2 XML parsing, get/put).
    // -----------------------------------------------------------------------

    const LIST_V2_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<ListBucketResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">\
<Name>test-bucket</Name><Prefix>vaults/</Prefix><KeyCount>1</KeyCount><MaxKeys>1000</MaxKeys>\
<IsTruncated>false</IsTruncated>\
<Contents><Key>vaults/a.kdbx</Key>\
<LastModified>2024-01-01T00:00:00.000Z</LastModified>\
<ETag>&quot;abc&quot;</ETag><Size>3</Size><StorageClass>STANDARD</StorageClass></Contents>\
</ListBucketResult>";

    /// Mixed bucket for the ordering test: two databases plus one other file.
    const MIXED_V2_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<ListBucketResult xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">\
<Name>test-bucket</Name><Prefix>vaults/</Prefix><KeyCount>3</KeyCount><MaxKeys>1000</MaxKeys>\
<IsTruncated>false</IsTruncated>\
<Contents><Key>vaults/notes.txt</Key><LastModified>2024-01-01T00:00:00.000Z</LastModified>\
<ETag>&quot;1&quot;</ETag><Size>4</Size><StorageClass>STANDARD</StorageClass></Contents>\
<Contents><Key>vaults/z.kdbx</Key><LastModified>2024-01-01T00:00:00.000Z</LastModified>\
<ETag>&quot;2&quot;</ETag><Size>5</Size><StorageClass>STANDARD</StorageClass></Contents>\
<Contents><Key>vaults/a.kdbx</Key><LastModified>2024-01-01T00:00:00.000Z</LastModified>\
<ETag>&quot;3&quot;</ETag><Size>3</Size><StorageClass>STANDARD</StorageClass></Contents>\
</ListBucketResult>";

    /// Serve one HTTP/1.1 request per connection (no keep-alive): list-type=2
    /// returns `list_xml`, PUTs are acknowledged, other GETs return `[1,2,3]`.
    fn s3_mock_handle(mut stream: std::net::TcpStream, list_xml: &'static str) {
        use std::io::{Read, Write};
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let header_end;
        loop {
            let n = stream.read(&mut chunk).unwrap_or(0);
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = pos + 4;
                break;
            }
        }
        let text = String::from_utf8_lossy(&buf[..header_end]);
        let head = text.lines().next().unwrap_or("").to_string();
        let is_list = head.starts_with("GET") && text.contains("list-type=2");
        let is_put = head.starts_with("PUT");
        let content_length: usize = text
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
            .and_then(|l| l.split(':').nth(1).and_then(|v| v.trim().parse().ok()))
            .unwrap_or(0);
        while buf.len() < header_end + content_length {
            let n = stream.read(&mut chunk).unwrap_or(0);
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        let body: Vec<u8> = if is_list {
            list_xml.as_bytes().to_vec()
        } else if is_put {
            Vec::new()
        } else {
            vec![1u8, 2, 3]
        };
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(&body);
    }

    fn spawn_s3_mock_with_xml(list_xml: &'static str) -> std::net::SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock listener");
        let addr = listener.local_addr().expect("mock addr");
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || s3_mock_handle(stream, list_xml));
            }
        });
        addr
    }

    fn spawn_s3_mock() -> std::net::SocketAddr {
        spawn_s3_mock_with_xml(LIST_V2_XML)
    }
    fn mock_config(addr: std::net::SocketAddr) -> RemoteSettings {
        RemoteSettings {
            endpoint: format!("http://{addr}"),
            region: "us-east-1".to_owned(),
            bucket: "test-bucket".to_owned(),
            access_key: "AK".to_owned(),
            secret_key: "SK".to_owned(),
            ..Default::default()
        }
    }

    #[test]
    fn s3_transport_round_trips_against_local_mock() {
        let storage = S3Storage::with_timeouts(
            &mock_config(spawn_s3_mock()),
            Duration::from_secs(10),
            Duration::from_secs(10),
        )
        .expect("storage");

        let objects = storage.list("vaults/").expect("list");
        assert_eq!(objects.len(), 1);
        assert_eq!(objects[0].key, "vaults/a.kdbx");
        assert_eq!(objects[0].size, 3);

        assert_eq!(storage.get("vaults/a.kdbx").expect("get"), vec![1, 2, 3]);
        storage.put("vaults/b.kdbx", &[9, 9]).expect("put");
    }

    /// The Tauri commands run on async runtime worker threads, where
    /// `S3Storage`'s sync methods (`Runtime::block_on`) panic — the command
    /// future aborts and the invoke never resolves, leaving the UI on an
    /// endless "正在加载…". Commands must hop to the blocking pool first, as
    /// `list_objects_async` does; this test pins that path end-to-end.
    #[test]
    fn s3_list_objects_async_works_from_runtime_worker_thread() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("rt");
        let cfg = mock_config(spawn_s3_mock());
        rt.block_on(async move {
            let objects = list_objects_async(cfg).await.expect("list");
            assert_eq!(objects.len(), 1);
            assert_eq!(objects[0].key, "vaults/a.kdbx");
            assert_eq!(objects[0].size, 3);
        });
    }

    /// The list must not filter to `.kdbx` only: every object shows, with
    /// databases first and then keys descending.
    #[test]
    fn s3_list_objects_async_lists_all_objects_kdbx_first() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("rt");
        let cfg = mock_config(spawn_s3_mock_with_xml(MIXED_V2_XML));
        rt.block_on(async move {
            let objects = list_objects_async(cfg).await.expect("list");
            let keys: Vec<&str> = objects.iter().map(|o| o.key.as_str()).collect();
            assert_eq!(keys, ["vaults/z.kdbx", "vaults/a.kdbx", "vaults/notes.txt"]);
        });
    }

    /// A server that accepts the connection but never answers must surface a
    /// bounded error instead of hanging the list forever.
    #[test]
    fn s3_list_surfaces_timeout_error_instead_of_hanging() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock listener");
        let addr = listener.local_addr().expect("mock addr");
        std::thread::spawn(move || {
            for _stream in listener.incoming().flatten() {
                std::thread::sleep(Duration::from_secs(30));
            }
        });
        let storage = S3Storage::with_timeouts(
            &mock_config(addr),
            Duration::from_millis(500),
            Duration::from_secs(10),
        )
        .expect("storage");
        let err = storage.list("vaults/").expect_err("list must time out");
        assert!(err.contains("超时"), "unexpected error: {err}");
    }

    // -----------------------------------------------------------------------
    // WebDAV transport tests (offline mock HTTP server + pure parsers)
    // -----------------------------------------------------------------------

    const WEBDAV_MULTISTATUS: &str = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<d:multistatus xmlns:d=\"DAV:\">\
<d:response><d:href>/dav/vaults/</d:href><d:propstat><d:prop>\
<d:resourcetype><d:collection/></d:resourcetype></d:prop>\
<d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>\
<d:response><d:href>/dav/vaults/a.kdbx</d:href><d:propstat><d:prop>\
<d:getcontentlength>3</d:getcontentlength>\
<d:getlastmodified>Mon, 01 Jan 2024 00:00:00 GMT</d:getlastmodified></d:prop>\
<d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>\
<d:response><d:href>/dav/vaults/sub/</d:href><d:propstat><d:prop>\
<d:resourcetype><d:collection/></d:resourcetype></d:prop>\
<d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>\
<d:response><d:href>/dav/vaults/z.kdbx</d:href><d:propstat><d:prop>\
<d:getcontentlength>5</d:getcontentlength></d:prop>\
<d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>\
</d:multistatus>";

    /// Serve one WebDAV request per connection: PROPFIND → multistatus,
    /// GET `a.kdbx` → `[1,2,3]`, PUT → 201, anything else → 404.
    fn webdav_mock_handle(mut stream: std::net::TcpStream) {
        use std::io::{Read, Write};
        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        let header_end;
        loop {
            let n = stream.read(&mut chunk).unwrap_or(0);
            if n == 0 {
                return;
            }
            buf.extend_from_slice(&chunk[..n]);
            if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                header_end = pos + 4;
                break;
            }
        }
        let text = String::from_utf8_lossy(&buf[..header_end]).into_owned();
        let head = text.lines().next().unwrap_or("").to_string();
        let content_length: usize = text
            .lines()
            .find(|l| l.to_ascii_lowercase().starts_with("content-length:"))
            .and_then(|l| l.split(':').nth(1).and_then(|v| v.trim().parse().ok()))
            .unwrap_or(0);
        while buf.len() < header_end + content_length {
            let n = stream.read(&mut chunk).unwrap_or(0);
            if n == 0 {
                break;
            }
            buf.extend_from_slice(&chunk[..n]);
        }
        let is_propfind = head.starts_with("PROPFIND");
        let is_put = head.starts_with("PUT");
        let is_get = head.starts_with("GET");
        let (status, body): (&str, Vec<u8>) = if is_propfind {
            ("207 Multi-Status", WEBDAV_MULTISTATUS.as_bytes().to_vec())
        } else if is_get && text.contains("/a.kdbx") {
            ("200 OK", vec![1u8, 2, 3])
        } else if is_put {
            ("201 Created", Vec::new())
        } else {
            ("404 Not Found", Vec::new())
        };
        let content_type = if is_propfind {
            "application/xml; charset=utf-8"
        } else {
            "application/octet-stream"
        };
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        );
        let _ = stream.write_all(response.as_bytes());
        let _ = stream.write_all(&body);
    }

    fn spawn_webdav_mock() -> std::net::SocketAddr {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("mock listener");
        let addr = listener.local_addr().expect("mock addr");
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                std::thread::spawn(move || webdav_mock_handle(stream));
            }
        });
        addr
    }

    fn mock_webdav_config(addr: std::net::SocketAddr) -> RemoteSettings {
        RemoteSettings {
            kind: "webdav".into(),
            endpoint: format!("http://{addr}/dav"),
            access_key: "user".into(),
            secret_key: "pass".into(),
            prefix: "vaults/".into(),
            ..Default::default()
        }
    }

    #[test]
    fn webdav_transport_round_trips_against_local_mock() {
        let storage = WebDavStorage::with_timeouts(
            &mock_webdav_config(spawn_webdav_mock()),
            Duration::from_secs(10),
            Duration::from_secs(10),
        )
        .expect("storage");

        let mut objects = storage.list("vaults/").expect("list");
        objects.sort_by(|a, b| a.key.cmp(&b.key));
        assert_eq!(objects.len(), 2, "collections and self must be dropped");
        assert_eq!(objects[0].key, "vaults/a.kdbx");
        assert_eq!(objects[0].size, 3);
        assert_eq!(
            objects[0].modified.as_deref(),
            Some("Mon, 01 Jan 2024 00:00:00 GMT")
        );
        assert_eq!(objects[1].key, "vaults/z.kdbx");
        assert_eq!(objects[1].size, 5);

        assert_eq!(storage.get("vaults/a.kdbx").expect("get"), vec![1, 2, 3]);
        storage.put("vaults/b.kdbx", &[9, 9]).expect("put");
    }

    /// The command path (factory + blocking-pool hop) must work end-to-end for
    /// WebDAV exactly as it does for S3.
    #[test]
    fn webdav_list_objects_async_works_from_runtime_worker_thread() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("rt");
        let cfg = mock_webdav_config(spawn_webdav_mock());
        rt.block_on(async move {
            let objects = list_objects_async(cfg).await.expect("list");
            let keys: Vec<&str> = objects.iter().map(|o| o.key.as_str()).collect();
            // .kdbx first (both are), then key descending.
            assert_eq!(keys, ["vaults/z.kdbx", "vaults/a.kdbx"]);
        });
    }

    #[test]
    fn webdav_parser_keeps_files_and_drops_collections() {
        let storage = WebDavStorage {
            client: reqwest::blocking::Client::new(),
            auth: None,
            base_url: "http://dav.example.com/dav".into(),
            list_timeout: Duration::from_secs(10),
            io_timeout: Duration::from_secs(10),
        };
        let objects = parse_multistatus(
            WEBDAV_MULTISTATUS,
            &storage.base_url,
            "http://dav.example.com/dav/vaults/",
        )
        .expect("parse");
        assert_eq!(objects.len(), 2);
        assert_eq!(objects[0].key, "vaults/a.kdbx");
        assert_eq!(objects[1].key, "vaults/z.kdbx");
    }

    /// Cloud-drive gateways often omit `getcontentlength` (or return it in a
    /// 404 propstat) and may use uppercase element names. Both must not cause
    /// every file to be dropped — the empty-list symptom reported against
    /// 123pan's WebDAV.
    #[test]
    fn webdav_parser_keeps_files_without_content_length_and_uppercase_tags() {
        let body = "<?xml version=\"1.0\"?><D:MULTISTATUS xmlns:D=\"DAV:\">\
<D:RESPONSE><D:HREF>/dav/</D:HREF><D:PROPSTAT><D:PROP>\
<D:RESOURCETYPE><D:COLLECTION/></D:RESOURCETYPE></D:PROP>\
<D:STATUS>HTTP/1.1 200 OK</D:STATUS></D:PROPSTAT></D:RESPONSE>\
<D:RESPONSE><D:HREF>/dav/a.kdbx</D:HREF><D:PROPSTAT><D:PROP>\
<D:GETCONTENTLENGTH>3</D:GETCONTENTLENGTH></D:PROP>\
<D:STATUS>HTTP/1.1 200 OK</D:STATUS></D:PROPSTAT></D:RESPONSE>\
<D:RESPONSE><D:HREF>/dav/z.kdbx</D:HREF><D:PROPSTAT>\
<D:STATUS>HTTP/1.1 404 Not Found</D:STATUS></D:PROPSTAT></D:RESPONSE>\
</D:MULTISTATUS>";
        let objects = parse_multistatus(
            body,
            "http://dav.example.com/dav",
            "http://dav.example.com/dav/",
        )
        .expect("parse");
        let keys: Vec<&str> = objects.iter().map(|o| o.key.as_str()).collect();
        // a.kdbx keeps its size; z.kdbx survives without any getcontentlength.
        assert_eq!(keys, ["a.kdbx", "z.kdbx"]);
        assert_eq!(objects[0].size, 3);
        assert_eq!(objects[1].size, 0);
    }

    /// A 200 response that is not a multistatus (an HTML/JSON error page from a
    /// misconfigured endpoint) must surface an actionable error instead of
    /// silently reporting "no files".
    #[test]
    fn webdav_parser_rejects_non_multistatus_body() {
        let err = parse_multistatus(
            "<html><body>Invalid request</body></html>",
            "http://dav.example.com/dav",
            "http://dav.example.com/dav/",
        )
        .expect_err("must fail");
        assert!(err.contains("multistatus"), "unexpected error: {err}");
    }

    #[test]
    fn webdav_href_to_key_handles_absolute_and_relative_hrefs() {
        assert_eq!(
            href_to_key("http://h/dav", "http://h/dav/vaults/a.kdbx"),
            "vaults/a.kdbx"
        );
        assert_eq!(
            href_to_key("http://h/dav", "/dav/vaults/a.kdbx"),
            "vaults/a.kdbx"
        );
        assert_eq!(
            href_to_key("http://h/dav", "vaults/a.kdbx"),
            "vaults/a.kdbx"
        );
        assert_eq!(href_to_key("http://h", "/vaults/a.kdbx"), "vaults/a.kdbx");
        assert_eq!(href_to_key("http://h/dav", "http://h/dav/"), "");
    }

    #[test]
    fn webdav_url_for_encodes_segments_and_rejects_dotdot() {
        let storage = WebDavStorage {
            client: reqwest::blocking::Client::new(),
            auth: None,
            base_url: "http://h/dav".into(),
            list_timeout: Duration::from_secs(10),
            io_timeout: Duration::from_secs(10),
        };
        assert_eq!(
            storage.url_for("a b/c.kdbx").unwrap(),
            "http://h/dav/a%20b/c.kdbx"
        );
        assert_eq!(storage.url_for("a.kdbx").unwrap(), "http://h/dav/a.kdbx");
        // ".." segments must not escape the endpoint.
        assert_eq!(storage.url_for("../x.kdbx").unwrap(), "http://h/dav/x.kdbx");
    }
}
