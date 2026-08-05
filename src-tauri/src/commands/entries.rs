//! Entry read-only + export IPC commands: password/TOTP fetch on demand,
//! security report, CSV export, CSV/XML import file reader (extracted from
//! commands.rs).

use crate::vault;
use crate::vault::{SecurityReport, VaultSession};
use std::path::Path;
use std::sync::Mutex;

/// CSV/XML import cap (8 MiB): guards the read-text command against oversized
/// files; the `.csv`/`.xml` extension whitelist stops arbitrary file
/// exfiltration.
const MAX_TEXT_IMPORT_BYTES: u64 = 8 * 1024 * 1024;

/// Fetch one entry's password on demand; passwords are never part of `VaultState`.
#[tauri::command]
pub(crate) fn get_entry_password(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<String, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .get_entry_password(&uuid)
}

/// Fetch one entry's TOTP seed on demand; seeds are never part of `VaultState`.
#[tauri::command]
pub(crate) fn get_entry_totp(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<Option<String>, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .get_entry_totp(&uuid)
}

/// Fetch one custom field's value on demand; protected custom fields are never
/// part of `VaultState`/`VaultEntry`.
#[tauri::command]
pub(crate) fn get_custom_field_value(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    name: String,
) -> Result<Option<String>, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .get_custom_field_value(&uuid, &name)
}

/// Server-side security analysis; no passwords leave the session.
#[tauri::command]
pub(crate) fn security_report(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<SecurityReport, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .security_report()
}

/// Export all entries as CSV to a user-picked path (passwords resolved server-side).
#[tauri::command]
pub(crate) fn export_csv(
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
) -> Result<(), String> {
    // Build the payload under the lock, write the file outside it.
    let content = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .export_csv_content()?;
    vault::write_csv_file(&path, &content)
}

/// Read a UTF-8 text file from a user-picked path (CSV / KeePass XML import).
/// Only `.csv` and `.xml` files are accepted: the command must never serve as
/// an arbitrary local file reader (e.g. for config.json, credentials, or other
/// vaults).
#[tauri::command]
pub(crate) fn read_text_file(path: String) -> Result<String, String> {
    let path = Path::new(&path);
    let allowed = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("csv") || e.eq_ignore_ascii_case("xml"));
    if !allowed {
        return Err("仅支持导入 .csv 或 .xml 文件".to_owned());
    }
    let meta = std::fs::metadata(path).map_err(|e| format!("读取文件失败: {e}"))?;
    if meta.len() > MAX_TEXT_IMPORT_BYTES {
        return Err(format!(
            "导入文件过大 (最大 {} MiB)",
            MAX_TEXT_IMPORT_BYTES / 1024 / 1024
        ));
    }
    std::fs::read_to_string(path).map_err(|e| format!("读取文件失败: {e}"))
}
