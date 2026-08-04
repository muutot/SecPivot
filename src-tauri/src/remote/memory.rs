//! In-memory fake storage for offline tests (extracted from remote/mod.rs).

use super::RemoteObject;
use super::RemoteStorage;
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
}
