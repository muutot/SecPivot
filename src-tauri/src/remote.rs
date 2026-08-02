//! S3 remote storage: a transport trait plus the real S3 implementation and
//! an in-memory fake used by offline tests. The payloads are ordinary KDBX
//! files; only the transport differs. S3 access keys are sent from the
//! frontend config on every command (never cached in the session) and live in
//! `conf/config.json` per the approved security model — they are secondary
//! credentials, distinct from vault master passwords.

use crate::config::RemoteSettings;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::Duration;
use tauri::Manager;

/// Prefix used for the display path of remote vaults (`s3://<key>`).
pub const REMOTE_URI_PREFIX: &str = "s3://";

/// TCP connect timeout for S3 requests. rust-s3's default is 60 s; 15 s keeps
/// an unreachable endpoint failure snappy.
const S3_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Overall bound for the object listing call (small payloads).
const S3_LIST_TIMEOUT: Duration = Duration::from_secs(30);
/// Overall bound for download/upload calls (vault files; generous for slow links).
const S3_IO_TIMEOUT: Duration = Duration::from_secs(120);

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
        Self::with_timeouts(cfg, S3_LIST_TIMEOUT, S3_IO_TIMEOUT)
    }

    fn with_timeouts(
        cfg: &RemoteSettings,
        list_timeout: Duration,
        io_timeout: Duration,
    ) -> Result<Self, String> {
        let endpoint = cfg.endpoint.trim();
        let region = cfg.region.trim();
        let bucket_name = cfg.bucket.trim();
        if endpoint.is_empty() {
            return Err("请先在设置中配置 S3 服务地址".to_owned());
        }
        if region.is_empty() {
            return Err("请先在设置中配置 S3 区域".to_owned());
        }
        if bucket_name.is_empty() {
            return Err("请先在设置中配置 S3 存储桶".to_owned());
        }
        let region = s3::Region::Custom {
            region: region.to_owned(),
            endpoint: endpoint.to_owned(),
        };
        let credentials = s3::creds::Credentials::new(
            Some(cfg.access_key.trim()),
            Some(cfg.secret_key.trim()),
            None,
            None,
            None,
        )
        .map_err(|e| format!("S3 凭据无效: {e}"))?;
        let mut bucket = s3::Bucket::new(bucket_name, region, credentials)
            .map_err(|e| format!("S3 配置无效: {e}"))?
            .with_request_timeout(S3_CONNECT_TIMEOUT)
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

/// Command body for `s3_list_objects`: lists all objects under the configured
/// prefix, `.kdbx` files first, then key descending. `S3Storage`'s sync
/// methods block on their own runtime, which panics when called from an async
/// runtime worker thread (the command future then aborts and the invoke never
/// resolves). Hop to the blocking pool first, exactly like the open/create
/// commands do.
pub async fn list_objects_async(cfg: RemoteSettings) -> Result<Vec<RemoteObject>, String> {
    let storage = S3Storage::new(&cfg)?;
    let prefix = cfg.prefix.clone();
    let mut objects = tokio::task::spawn_blocking(move || storage.list(&prefix))
        .await
        .map_err(|e| format!("S3 列表任务异常: {e}"))??;
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

/// Base directory for local copies: `<app_data>/Storage/remote/<local_dir>`.
/// `local_dir` is user-defined in settings ("在定义一个目录"); the name is
/// sanitized so it cannot escape the remote storage tree.
pub fn local_storage_dir(app: &tauri::AppHandle, local_dir: &str) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位应用数据目录: {e}"))?;
    let name = sanitize_dir_name(local_dir);
    Ok(base.join("Storage").join("remote").join(name))
}

fn sanitize_dir_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "remote".to_owned();
    }
    let mut out = String::new();
    for ch in trimmed.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
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
}
