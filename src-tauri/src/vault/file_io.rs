//! Vault file I/O across desktop paths and mobile document URIs.
//!
//! Android's document picker returns `content://` URIs (not filesystem paths),
//! and `std::fs` cannot open them. The `fs` plugin resolves those URIs through
//! the app's ContentResolver, so both reads and writes go through it on mobile.
//! The app handle is captured once at startup so the lock-free persistence
//! helpers stay app-handle-free (they run in `spawn_blocking`).

use std::path::Path;

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

/// Read a vault or keyfile, resolving mobile document URIs via the fs plugin.
/// `what` names the file type in user-facing error messages.
pub(crate) fn read(path: &Path, what: &str) -> Result<Vec<u8>, String> {
    #[cfg(mobile)]
    if is_document_uri(path) {
        return read_uri(path, what);
    }
    std::fs::read(path).map_err(|e| format!("无法读取{what}: {e}"))
}

/// Write a vault file. Desktop paths keep the atomic temp-and-rename behavior;
/// mobile document URIs are written through the ContentResolver (SAF offers no
/// rename, so the target is truncated in place).
pub(crate) fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    #[cfg(mobile)]
    if is_document_uri(path) {
        return write_uri(path, bytes);
    }
    crate::util::atomic_write(path, bytes, "数据库")
}

#[cfg(mobile)]
fn document_path(path: &Path) -> Result<tauri_plugin_fs::FilePath, String> {
    use std::str::FromStr;
    let text = path.to_string_lossy();
    tauri_plugin_fs::FilePath::from_str(&text).map_err(|e| format!("文档 URI 无效: {e}"))
}

#[cfg(mobile)]
fn read_uri(path: &Path, what: &str) -> Result<Vec<u8>, String> {
    let app = APP
        .get()
        .ok_or_else(|| format!("无法读取{what}: 应用尚未初始化"))?;
    read_uri_with(app, path, what)
}

#[cfg(mobile)]
fn read_uri_with(app: &AppHandle, path: &Path, what: &str) -> Result<Vec<u8>, String> {
    use std::io::Read;
    use tauri_plugin_fs::FsExt;

    let mut options = tauri_plugin_fs::OpenOptions::new();
    options.read(true);
    let mut file = app
        .fs()
        .open(document_path(path)?, options)
        .map_err(|e| format!("无法读取{what}: {e}"))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取{what}: {e}"))?;
    Ok(bytes)
}

/// Write the document content in place, truncating the target first.
#[cfg(mobile)]
fn write_uri_with(app: &AppHandle, path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    use tauri_plugin_fs::FsExt;

    let mut options = tauri_plugin_fs::OpenOptions::new();
    options.write(true).truncate(true);
    let mut file = app
        .fs()
        .open(document_path(path)?, options)
        .map_err(|e| format!("写入数据库失败: {e}"))?;
    file.write_all(bytes)
        .map_err(|e| format!("写入数据库失败: {e}"))?;
    let _ = file.sync_all();
    Ok(())
}

/// Put the previous content back after a failed write.
///
/// `None` means there was nothing to restore (the document was empty or
/// unreadable before the write), `Some(true)` means the previous content is
/// back, `Some(false)` means the provider is still broken and the caller must
/// tell the user to re-pick the file.
#[cfg(mobile)]
fn restore_uri(app: &AppHandle, path: &Path, previous: Option<&[u8]>) -> Option<bool> {
    let previous = previous?;
    Some(write_uri_with(app, path, previous).is_ok())
}

/// Message suffix describing the rollback outcome.
#[cfg(mobile)]
fn rollback_note(restored: Option<bool>) -> &'static str {
    match restored {
        None => "，文档此前为空，未覆盖已有内容",
        Some(true) => "，已回滚到上一次内容",
        Some(false) => "，且回滚失败，请重新选择数据库文件",
    }
}

/// Write a mobile document.
///
/// SAF has no rename, so the document can only be rewritten in place and the
/// write truncates the target first: a provider error halfway through would
/// leave the user with an empty or half-written database and no other copy.
/// The write is therefore transactional at the application level: the current
/// content is read first, the new content is written, the stored bytes are
/// read back and compared, and any failure rolls the previous content back.
/// A provider that serves a stale copy makes the verification fail; the
/// rollback then keeps the previous database intact and the save is reported
/// as failed instead of silently losing data.
#[cfg(mobile)]
fn write_uri(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let app = APP
        .get()
        .ok_or_else(|| "写入数据库失败: 应用尚未初始化".to_owned())?;
    let previous = read_uri_with(app, path, "数据库文件").ok();
    match write_uri_with(app, path, bytes) {
        Ok(()) => match read_uri_with(app, path, "数据库文件") {
            Ok(stored) if stored == bytes => Ok(()),
            Ok(_) => {
                let restored = restore_uri(app, path, previous.as_deref());
                Err(format!(
                    "写入数据库失败: 写入内容校验不一致{}",
                    rollback_note(restored)
                ))
            }
            Err(e) => {
                let restored = restore_uri(app, path, previous.as_deref());
                Err(format!("写入数据库失败: {e}{}", rollback_note(restored)))
            }
        },
        Err(e) => {
            let restored = restore_uri(app, path, previous.as_deref());
            Err(format!("{e}{}", rollback_note(restored)))
        }
    }
}
