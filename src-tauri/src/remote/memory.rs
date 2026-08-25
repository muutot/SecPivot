//! In-memory fake storage for offline tests (extracted from remote/mod.rs).

use super::RemoteObject;
use super::RemoteStorage;
use crate::crypto::{hex, sha256_bytes};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
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

    /// The fake uses the content's SHA-256 hex as its ETag, so the
    /// conditional write is a real compare-and-swap under the lock — strong
    /// enough to unit-test the lost-race path of `persist_snapshot`.
    fn get_with_etag(&self, key: &str) -> Result<(Vec<u8>, Option<String>), String> {
        let guard = self.objects.read().map_err(|_| "存储锁已损坏".to_owned())?;
        let data = guard
            .get(key)
            .cloned()
            .ok_or_else(|| format!("远程文件不存在: {key}"))?;
        let etag = hex(&sha256_bytes(&data));
        Ok((data, Some(etag)))
    }

    fn put_if_match(&self, key: &str, data: &[u8], etag: Option<&str>) -> Result<(), String> {
        let Some(expected) = etag else {
            return self.put(key, data);
        };
        let mut guard = self
            .objects
            .write()
            .map_err(|_| "存储锁已损坏".to_owned())?;
        match guard.get(key) {
            Some(current) if hex(&sha256_bytes(current)) == expected => {
                guard.insert(key.to_owned(), data.to_vec());
                Ok(())
            }
            _ => Err(format!(
                "{}远程库已被其他设备修改（内容校验不匹配），请选择合并、覆盖远程、下载远程或保留本地",
                super::REMOTE_CONFLICT_MARKER
            )),
        }
    }
}
