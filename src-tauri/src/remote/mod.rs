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
/// `reqwest::blocking::Client::build` also *blocks* (`wait::enter`), so it must
/// run off any async worker or it panics on first use — hence a dedicated
/// init thread for the one-time construction.
pub(crate) fn shared_blocking_client() -> &'static reqwest::blocking::Client {
    static CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        std::thread::Builder::new()
            .name("remote-client-init".into())
            .spawn(|| {
                reqwest::blocking::Client::builder()
                    .connect_timeout(REMOTE_CONNECT_TIMEOUT)
                    .build()
                    .expect("无法初始化远程存储客户端: reqwest 资源不足")
            })
            .expect("spawn remote client init thread")
            .join()
            .expect("remote client init thread panicked")
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
