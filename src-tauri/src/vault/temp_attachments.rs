//! Controlled temporary extraction of attachments for external viewing.
//!
//! Files land in `<system temp>/secpivot-attachments/<random token>/<sanitized
//! name>` so a malicious attachment name can never escape the directory.
//! Every file is registered by token so it can be removed on discard, lock,
//! or close; nothing here logs or persists attachment content elsewhere.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct AttachmentTempStore {
    inner: Mutex<HashMap<String, PathBuf>>,
}

impl AttachmentTempStore {
    /// Write `data` into a fresh random directory and register it. Returns
    /// `(token, absolute file path)`. The caller opens the file with the
    /// system viewer and eventually calls [`Self::discard`].
    pub fn create(&self, name: &str, data: &[u8]) -> Result<(String, PathBuf), String> {
        let base = std::env::temp_dir().join("secpivot-attachments");
        let token = uuid::Uuid::new_v4().to_string();
        let dir = base.join(&token);
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建临时目录失败: {e}"))?;
        let path = dir.join(sanitize_file_name(name));
        std::fs::write(&path, data).map_err(|e| format!("写入临时附件失败: {e}"))?;
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "附件临时存储锁已损坏".to_owned())?;
        inner.insert(token.clone(), path.clone());
        Ok((token, path))
    }

    /// Remove the registered temp file and its random directory. Unknown
    /// tokens (already discarded) are a no-op.
    pub fn discard(&self, token: &str) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "附件临时存储锁已损坏".to_owned())?;
        let Some(dir) = inner.remove(token) else {
            return Ok(());
        };
        if let Some(parent) = dir.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
        Ok(())
    }

    /// Discard every registered temp file (lock/close path).
    pub fn discard_all(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            for (_, dir) in inner.drain() {
                if let Some(parent) = dir.parent() {
                    let _ = std::fs::remove_dir_all(parent);
                }
            }
        }
    }
}

/// Keep only a single file-name segment with safe characters; fall back to a
/// fixed name so the extracted file cannot escape the token directory.
fn sanitize_file_name(name: &str) -> String {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "._- ".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').to_owned();
    if cleaned.is_empty() {
        "attachment".into()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_writes_registered_file_and_discard_removes_it() {
        let store = AttachmentTempStore::default();
        let (token, path) = store.create("note.txt", b"hello").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"hello");
        assert!(path.starts_with(std::env::temp_dir().join("secpivot-attachments")));

        store.discard(&token).unwrap();
        assert!(!path.exists());
        // Discarding again is a no-op.
        store.discard(&token).unwrap();
    }

    #[test]
    fn discard_all_clears_every_registered_file() {
        let store = AttachmentTempStore::default();
        let (_, path_a) = store.create("a.txt", b"a").unwrap();
        let (_, path_b) = store.create("b.txt", b"b").unwrap();
        store.discard_all();
        assert!(!path_a.exists());
        assert!(!path_b.exists());
    }

    #[test]
    fn sanitize_keeps_safe_names_and_blocks_path_escape() {
        let store = AttachmentTempStore::default();
        let (_, path) = store.create("..\\..\\evil.txt", b"x").unwrap();
        let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
        // Only the last segment survives; it stays inside the random token dir.
        assert_eq!(file_name, "evil.txt");
        assert!(path
            .parent()
            .unwrap()
            .starts_with(std::env::temp_dir().join("secpivot-attachments")));
    }
}
