//! Remote storage transports shared by the S3 and WebDAV backends: a transport
//! trait plus the real S3 and WebDAV implementations and an in-memory fake used
//! by offline tests. The payloads are ordinary KDBX files; only the transport
//! differs. S3 keys / WebDAV credentials are sent from the frontend config on
//! every command (never cached in the session) and live in `conf/config.json`
//! per the approved security model 鈥?they are secondary credentials, distinct
//! from vault master passwords.

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
