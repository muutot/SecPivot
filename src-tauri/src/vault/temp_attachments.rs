//! Controlled temporary extraction of attachments for external viewing.
//!
//! Files land in `<system temp>/secpivot-attachments/<random token>/<sanitized
//! name>` so a malicious attachment name can never escape the directory.
//! Every file is registered by token so it can be removed on discard, lock,
//! or close; nothing here logs or persists attachment content elsewhere.

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Clone)]
struct TempAttachment {
    path: PathBuf,
    session_id: String,
}

#[derive(Default)]
pub struct AttachmentTempStore {
    inner: Mutex<HashMap<String, TempAttachment>>,
}

impl AttachmentTempStore {
    const MAX_IMPORT_BYTES: u64 = 64 * 1024 * 1024;

    /// Write `data` into a fresh random directory and register it. Returns
    /// `(token, absolute file path)`. The caller opens the file with the
    /// system viewer and eventually calls [`Self::discard`].
    pub fn create(
        &self,
        session_id: &str,
        name: &str,
        data: &[u8],
    ) -> Result<(String, PathBuf), String> {
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
        inner.insert(
            token.clone(),
            TempAttachment {
                path: path.clone(),
                session_id: session_id.to_owned(),
            },
        );
        Ok((token, path))
    }

    /// Remove the registered temp file and its random directory. Unknown
    /// tokens (already discarded) are a no-op.
    pub fn discard(&self, token: &str) -> Result<(), String> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| "附件临时存储锁已损坏".to_owned())?;
        let Some(temp) = inner.remove(token) else {
            return Ok(());
        };
        if let Some(parent) = temp.path.parent() {
            let _ = std::fs::remove_dir_all(parent);
        }
        Ok(())
    }

    /// Resolve a token to its registered temp file path (the caller reads the
    /// file). Unknown tokens are rejected so only files we extracted can be
    /// imported back.
    pub fn path_for_session(&self, token: &str, session_id: &str) -> Result<PathBuf, String> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| "附件临时存储锁已损坏".to_owned())?;
        let temp = inner
            .get(token)
            .ok_or_else(|| "临时附件已清理或不存在".to_owned())?;
        if temp.session_id != session_id {
            return Err("临时附件不属于当前数据库会话".to_owned());
        }
        Ok(temp.path.clone())
    }

    /// Resolve and read one registered file without holding the vault-session
    /// lock. The token stays registered on failure so the user can retry.
    pub fn read_for_session(&self, token: &str, session_id: &str) -> Result<Vec<u8>, String> {
        let path = self.path_for_session(token, session_id)?;
        let meta = std::fs::metadata(&path).map_err(|e| format!("读取临时附件失败: {e}"))?;
        if meta.len() > Self::MAX_IMPORT_BYTES {
            return Err(format!("附件过大（{} 字节，上限 64 MiB）", meta.len()));
        }
        let file = File::open(&path).map_err(|e| format!("读取临时附件失败: {e}"))?;
        let mut data = Vec::with_capacity(meta.len() as usize);
        file.take(Self::MAX_IMPORT_BYTES + 1)
            .read_to_end(&mut data)
            .map_err(|e| format!("读取临时附件失败: {e}"))?;
        if data.len() as u64 > Self::MAX_IMPORT_BYTES {
            return Err(format!("附件过大（{} 字节，上限 64 MiB）", data.len()));
        }
        Ok(data)
    }

    /// Discard every registered temp file (lock/close path).
    pub fn discard_all(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            for (_, temp) in inner.drain() {
                if let Some(parent) = temp.path.parent() {
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
        let (token, path) = store.create("s1", "note.txt", b"hello").unwrap();
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
        let (_, path_a) = store.create("s1", "a.txt", b"a").unwrap();
        let (_, path_b) = store.create("s2", "b.txt", b"b").unwrap();
        store.discard_all();
        assert!(!path_a.exists());
        assert!(!path_b.exists());
    }

    #[test]
    fn sanitize_keeps_safe_names_and_blocks_path_escape() {
        let store = AttachmentTempStore::default();
        let (_, path) = store.create("s1", "..\\..\\evil.txt", b"x").unwrap();
        let file_name = path.file_name().unwrap().to_string_lossy().into_owned();
        // Only the last segment survives; it stays inside the random token dir.
        assert_eq!(file_name, "evil.txt");
        assert!(path
            .parent()
            .unwrap()
            .starts_with(std::env::temp_dir().join("secpivot-attachments")));
    }

    #[test]
    fn token_is_bound_to_its_originating_session() {
        let store = AttachmentTempStore::default();
        let (token, path) = store.create("s1", "note.txt", b"secret").unwrap();
        assert_eq!(store.path_for_session(&token, "s1").unwrap(), path);
        assert_eq!(
            store.path_for_session(&token, "s2").unwrap_err(),
            "临时附件不属于当前数据库会话"
        );
        assert!(
            path.exists(),
            "a failed cross-session lookup must not discard the token"
        );
        store.discard(&token).unwrap();
    }

    #[test]
    fn read_for_session_returns_registered_bytes_and_preserves_retry_token() {
        let store = AttachmentTempStore::default();
        let (token, path) = store.create("s1", "note.txt", b"edited").unwrap();

        assert_eq!(store.read_for_session(&token, "s1").unwrap(), b"edited");
        assert_eq!(
            store.read_for_session(&token, "s2").unwrap_err(),
            "临时附件不属于当前数据库会话"
        );
        assert!(path.exists());
        store.discard(&token).unwrap();
    }
}
