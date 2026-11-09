//! Vault file I/O across desktop paths and mobile document URIs.
//!
//! Android's document picker returns `content://` URIs (not filesystem paths),
//! and `std::fs` cannot open them. The `fs` plugin resolves those URIs through
//! the app's ContentResolver, so both reads and writes go through it on mobile.
//! The app handle is captured once at startup so the lock-free persistence
//! helpers stay app-handle-free (they run in `spawn_blocking`).

use std::path::Path;
use zeroize::Zeroizing;

#[cfg(mobile)]
use std::sync::OnceLock;
#[cfg(mobile)]
use tauri::AppHandle;

#[cfg(mobile)]
static APP: OnceLock<AppHandle> = OnceLock::new();

/// Capture the app handle used to resolve mobile document URIs. Called once from
/// the setup hook; later calls are ignored.
#[cfg(mobile)]
pub(crate) fn capture_app(app: AppHandle) {
    let _ = APP.set(app);
}

/// Whether `path` is a document URI rather than a filesystem path. Mobile
/// pickers return `content://` URIs; iOS also uses `file://` URLs.
pub(crate) fn is_document_uri(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.starts_with("content://") || text.starts_with("file://")
}

/// Largest local file that is read into memory. A KeePass database is a few
/// megabytes; a bigger file is a mis-picked or hostile path, and reading it
/// unconditionally turns it into an out-of-memory abort.
const MAX_LOCAL_FILE_BYTES: u64 = 512 * 1024 * 1024;

/// Read a vault, resolving mobile document URIs via the fs plugin.
/// `what` names the file type in user-facing error messages.
pub(crate) fn read(path: &Path, what: &str) -> Result<Vec<u8>, String> {
    read_with_limit(path, what, MAX_LOCAL_FILE_BYTES)
}

/// Bounded vault/keyfile/text-import read, including mobile document URIs.
pub(crate) fn read_with_limit(path: &Path, what: &str, limit: u64) -> Result<Vec<u8>, String> {
    #[cfg(mobile)]
    if is_document_uri(path) {
        return read_uri(path, what, limit);
    }
    if !is_document_uri(path) {
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.len() > limit {
                return Err(format!("{what}超过 {} MiB 上限", limit / 1024 / 1024));
            }
        }
    }
    let file = std::fs::File::open(path).map_err(|e| format!("无法读取{what}: {e}"))?;
    read_capped(file, what, limit)
}

fn read_capped(reader: impl std::io::Read, what: &str, limit: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = Zeroizing::new(Vec::new());
    reader
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取{what}: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("{what}超过 {} MiB 上限", limit / 1024 / 1024));
    }
    Ok(std::mem::take(&mut *bytes))
}

/// Write a vault file. Desktop paths keep the atomic temp-and-rename behavior;
/// mobile document URIs are written through the ContentResolver (SAF offers no
/// rename, so the target is truncated in place).
pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    write_named(path, bytes, "数据库")
}

/// Shared export/attachment writer; local paths are atomic, URI writes keep
/// verified rollback. `what` labels errors without describing file contents.
pub(crate) fn write_named(path: &Path, bytes: &[u8], what: &str) -> Result<(), String> {
    #[cfg(mobile)]
    if is_document_uri(path) {
        return write_uri(path, bytes, what);
    }
    crate::util::atomic_write(path, bytes, what)
}

#[cfg(mobile)]
fn document_path(path: &Path) -> Result<tauri_plugin_fs::FilePath, String> {
    use std::str::FromStr;
    let text = path.to_string_lossy();
    tauri_plugin_fs::FilePath::from_str(&text).map_err(|e| format!("文档 URI 无效: {e}"))
}

#[cfg(mobile)]
fn read_uri(path: &Path, what: &str, limit: u64) -> Result<Vec<u8>, String> {
    let app = APP
        .get()
        .ok_or_else(|| format!("无法读取{what}: 应用尚未初始化"))?;
    read_uri_with(app, path, what, limit)
}

#[cfg(mobile)]
fn read_uri_with(app: &AppHandle, path: &Path, what: &str, limit: u64) -> Result<Vec<u8>, String> {
    use tauri_plugin_fs::FsExt;

    let mut options = tauri_plugin_fs::OpenOptions::new();
    options.read(true);
    let file = app
        .fs()
        .open(document_path(path)?, options)
        .map_err(|e| format!("无法读取{what}: {e}"))?;
    read_capped(file, what, limit)
}

/// Write the document content in place, truncating the target first.
#[cfg(mobile)]
fn write_uri_with(app: &AppHandle, path: &Path, bytes: &[u8], what: &str) -> Result<(), String> {
    use std::io::Write;
    use tauri_plugin_fs::FsExt;

    let mut options = tauri_plugin_fs::OpenOptions::new();
    options.write(true).truncate(true);
    let mut file = app
        .fs()
        .open(document_path(path)?, options)
        .map_err(|e| format!("写入{what}失败: {e}"))?;
    file.write_all(bytes)
        .map_err(|e| format!("写入{what}失败: {e}"))?;
    file.sync_all().map_err(|e| format!("写入{what}失败: {e}"))
}

/// SAF cannot rename: refuse to truncate unless the old content was read.
/// A rollback is reported successful only after its bytes are read back.
#[cfg(any(mobile, test))]
fn write_verified(
    what: &str,
    bytes: &[u8],
    mut read: impl FnMut() -> Result<Vec<u8>, String>,
    mut write: impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let previous =
        Zeroizing::new(read().map_err(|e| format!("无法读取原{what}，已取消写入: {e}"))?);
    let result = write(bytes).and_then(|()| {
        if Zeroizing::new(read()?).as_slice() == bytes {
            Ok(())
        } else {
            Err("写入内容校验不一致".to_owned())
        }
    });
    if let Err(error) = result {
        let restored = write(&previous)
            .and_then(|()| read())
            .map(Zeroizing::new)
            .is_ok_and(|stored| stored.as_slice() == previous.as_slice());
        let note = if restored {
            "已回滚到上一次内容".to_owned()
        } else {
            format!("回滚失败，请重新选择{what}文件")
        };
        return Err(format!("写入{what}失败: {error}，{note}"));
    }
    Ok(())
}

#[cfg(mobile)]
fn write_uri(path: &Path, bytes: &[u8], what: &str) -> Result<(), String> {
    let app = APP
        .get()
        .ok_or_else(|| format!("写入{what}失败: 应用尚未初始化"))?;
    write_verified(
        what,
        bytes,
        || read_uri_with(app, path, what, MAX_LOCAL_FILE_BYTES),
        |data| write_uri_with(app, path, data, what),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    #[test]
    fn capped_read_stops_at_limit_plus_one() {
        struct Counted<'a>(&'a Cell<usize>);
        impl std::io::Read for Counted<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                buffer.fill(1);
                self.0.set(self.0.get() + buffer.len());
                Ok(buffer.len())
            }
        }
        let count = Cell::new(0);
        assert!(read_capped(Counted(&count), "fixture", 8).is_err());
        assert_eq!(count.get(), 9);
        assert_eq!(
            read_capped(&b"12345678"[..], "fixture", 8).unwrap(),
            b"12345678"
        );
    }

    #[test]
    fn capped_read_rejects_growth_after_a_successful_metadata_check() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("growing.csv");
        std::fs::write(&path, b"small").unwrap();
        assert!(std::fs::metadata(&path).unwrap().len() <= 8);
        let reader = std::fs::File::open(&path).unwrap();
        let mut writer = std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap();
        writer.write_all(b"now-too-large").unwrap();
        assert!(read_capped(reader, "导入文件", 8)
            .unwrap_err()
            .contains("上限"));
    }

    #[test]
    fn unreadable_document_is_never_truncated() {
        let writes = Cell::new(0);
        let result = write_verified(
            "数据库",
            b"new",
            || Err("provider failed".into()),
            |_| {
                writes.set(writes.get() + 1);
                Ok(())
            },
        );
        assert!(result.unwrap_err().contains("已取消写入"));
        assert_eq!(writes.get(), 0);
    }

    #[test]
    fn partial_write_restores_previous_bytes_and_empty_document_can_be_created() {
        let stored = RefCell::new(b"previous".to_vec());
        let writes = Cell::new(0);
        let error = write_verified(
            "数据库",
            b"new",
            || Ok(stored.borrow().clone()),
            |data| {
                writes.set(writes.get() + 1);
                if writes.get() == 1 {
                    *stored.borrow_mut() = b"partial".to_vec();
                    Err("partial failure".into())
                } else {
                    *stored.borrow_mut() = data.to_vec();
                    Ok(())
                }
            },
        )
        .unwrap_err();
        assert!(error.contains("已回滚"));
        assert_eq!(*stored.borrow(), b"previous");
        stored.borrow_mut().clear();
        write_verified(
            "数据库",
            b"new",
            || Ok(stored.borrow().clone()),
            |data| {
                *stored.borrow_mut() = data.to_vec();
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(*stored.borrow(), b"new");
    }

    #[test]
    fn rollback_success_requires_readback_verification() {
        let reads = Cell::new(0);
        let error = write_verified(
            "数据库",
            b"new",
            || {
                reads.set(reads.get() + 1);
                Ok(if reads.get() == 1 {
                    b"previous".to_vec()
                } else {
                    b"wrong".to_vec()
                })
            },
            |_| Ok(()),
        )
        .unwrap_err();
        assert!(error.contains("回滚失败"));
        assert!(!error.contains("已回滚"));
    }

    #[test]
    fn named_exports_refuse_unreadable_targets_and_restore_failed_writes() {
        for what in ["文件", "附件"] {
            let writes = Cell::new(0);
            let error = write_verified(
                what,
                b"new",
                || Err("provider failed".into()),
                |_| {
                    writes.set(writes.get() + 1);
                    Ok(())
                },
            )
            .unwrap_err();
            assert!(error.contains(what));
            assert!(error.contains("已取消写入"));
            assert_eq!(writes.get(), 0);

            let stored = RefCell::new(b"previous".to_vec());
            let error = write_verified(
                what,
                b"new",
                || Ok(stored.borrow().clone()),
                |data| {
                    writes.set(writes.get() + 1);
                    if writes.get() == 1 {
                        *stored.borrow_mut() = b"partial".to_vec();
                        Err("failed write".into())
                    } else {
                        *stored.borrow_mut() = data.to_vec();
                        Ok(())
                    }
                },
            )
            .unwrap_err();
            assert!(error.contains(what));
            assert!(error.contains("已回滚"));
            assert_eq!(*stored.borrow(), b"previous");
        }
    }
}
