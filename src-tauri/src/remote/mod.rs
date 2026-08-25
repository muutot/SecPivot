//! Remote storage transports shared by the S3 and WebDAV backends: a transport
//! trait plus the real S3 and WebDAV implementations and an in-memory fake used
//! by offline tests. The payloads are ordinary KDBX files; only the transport
//! differs. The frontend sends only a canonical profile path; S3 keys / WebDAV
//! credentials are resolved from backend config for each command (never cached
//! in the session) and live in `conf/config.json` per the approved security
//! model — they are secondary credentials, distinct from vault master passwords.

pub(crate) mod backup;
pub(crate) mod local;
pub(crate) mod memory;
pub(crate) mod s3;
#[cfg(test)]
mod tests;
pub(crate) mod webdav;

pub use self::local::local_storage_dir;
pub use self::memory::MemoryStorage;

use self::s3::S3Storage;
use self::webdav::WebDavStorage;
use crate::config::RemoteSettings;
use serde::Serialize;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// Prefix used for the display path of remote vaults (`s3://<key>`).
pub const REMOTE_URI_PREFIX: &str = "s3://";

/// Error prefix for a lost save race: the remote object changed between our
/// read and the preconditioned write (HTTP 412 / hash mismatch), so the
/// upload was rejected instead of silently clobbering the other writer.
/// The frontend matches this marker to offer 覆盖远程/下载远程/保留本地.
pub const REMOTE_CONFLICT_MARKER: &str = "REMOTE_CHANGED\n";

/// TCP connect timeout for remote storage requests (shared).
const REMOTE_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// Overall bound for the object listing call (small payloads).
const REMOTE_LIST_TIMEOUT: Duration = Duration::from_secs(30);
/// Overall bound for download/upload calls (vault files; generous for slow links).
const REMOTE_IO_TIMEOUT: Duration = Duration::from_secs(120);

/// One process-wide shared reqwest blocking client used by both the S3 and
/// WebDAV transports. The client owns a tokio runtime that must never be
/// dropped from an async context (dropping it on a runtime worker panics —
/// e.g. when a vault session closes); keeping the original alive forever means
/// per-storage clones never tear the runtime down.
///
/// `reqwest::blocking::Client::build` also *blocks* (`wait::enter`), so it
/// must run off any async worker or it panics on first use — hence a dedicated
/// init thread for the one-time construction. A failed construction (thread
/// spawn/join failure, resource exhaustion) is cached and reported as an
/// error instead of aborting the process: remote sync stays unavailable but
/// the vault session survives. The failure is sticky by design — retrying a
/// half-initialized runtime is not safe.
pub(crate) fn shared_blocking_client() -> Result<&'static reqwest::blocking::Client, String> {
    static CLIENT: OnceLock<Result<reqwest::blocking::Client, String>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            let handle = std::thread::Builder::new()
                .name("remote-client-init".into())
                .spawn(|| {
                    reqwest::blocking::Client::builder()
                        .connect_timeout(REMOTE_CONNECT_TIMEOUT)
                        .build()
                })
                .map_err(|e| format!("无法初始化远程存储客户端: 创建初始化线程失败: {e}"))?;
            match handle.join() {
                Ok(Ok(client)) => Ok(client),
                Ok(Err(e)) => Err(format!("无法初始化远程存储客户端: {e}")),
                Err(_) => Err("无法初始化远程存储客户端: 初始化线程异常退出".to_owned()),
            }
        })
        .as_ref()
        .map_err(|message| message.clone())
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

    /// Download plus the transport's content identity (HTTP `ETag` when the
    /// server exposes one). The identity feeds [`RemoteStorage::put_if_match`]
    /// so a concurrent writer's change is rejected instead of clobbered.
    /// Default: plain get with no identity.
    fn get_with_etag(&self, key: &str) -> Result<(Vec<u8>, Option<String>), String> {
        Ok((self.get(key)?, None))
    }

    /// Upload preconditioned on the identity observed by a matching
    /// `get_with_etag`. `None` (or an unsupported transport) degrades to a
    /// plain put. A lost race returns an error starting with
    /// [`REMOTE_CONFLICT_MARKER`].
    fn put_if_match(&self, key: &str, data: &[u8], etag: Option<&str>) -> Result<(), String> {
        let _ = etag;
        self.put(key, data)
    }
}
pub fn make_storage(cfg: &RemoteSettings) -> Result<Arc<dyn RemoteStorage>, String> {
    match cfg.kind.as_str() {
        "webdav" => Ok(Arc::new(WebDavStorage::new(cfg)?)),
        "s3" => Ok(Arc::new(S3Storage::new(cfg)?)),
        other => Err(format!("未知的远程存储类型: {other}（仅支持 s3 / webdav）")),
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
