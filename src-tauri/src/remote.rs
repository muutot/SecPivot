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
use tauri::Manager;

/// Prefix used for the display path of remote vaults (`s3://<key>`).
pub const REMOTE_URI_PREFIX: &str = "s3://";

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
}

impl S3Storage {
    pub fn new(cfg: &RemoteSettings) -> Result<Self, String> {
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
            .map_err(|e| format!("S3 配置无效: {e}"))?;
        bucket.set_path_style();
        Ok(Self {
            bucket,
            runtime: shared_runtime(),
        })
    }

    fn object_key(key: &str) -> String {
        format!("/{}", key.trim_start_matches('/'))
    }
}

impl RemoteStorage for S3Storage {
    fn list(&self, prefix: &str) -> Result<Vec<RemoteObject>, String> {
        self.runtime.block_on(async {
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
    }

    fn get(&self, key: &str) -> Result<Vec<u8>, String> {
        self.runtime.block_on(async {
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
    }

    fn put(&self, key: &str, data: &[u8]) -> Result<(), String> {
        self.runtime.block_on(async {
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
}
