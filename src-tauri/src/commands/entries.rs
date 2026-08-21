//! Entry read-only + export IPC commands: password/TOTP fetch on demand,
//! security report, CSV export, CSV/XML import file reader (extracted from
//! commands.rs).

use super::with_vault_session;
use crate::vault;
use crate::vault::{EntryStorage, SecurityReport, VaultSession, VaultSessions};
use std::path::Path;
use std::sync::Mutex;

/// CSV/XML import cap (8 MiB): guards the read-text command against oversized
/// files; the `.csv`/`.xml` extension whitelist stops arbitrary file
/// exfiltration.
const MAX_TEXT_IMPORT_BYTES: u64 = 8 * 1024 * 1024;

/// Fetch one entry's password on demand; passwords are never part of `VaultState`.
#[tauri::command]
pub(crate) fn get_entry_password(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<String, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.get_entry_password(&uuid),
    )
}

/// Fetch one entry's TOTP seed on demand; seeds are never part of `VaultState`.
#[tauri::command]
pub(crate) fn get_entry_totp(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<Option<String>, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.get_entry_totp(&uuid),
    )
}

/// Fetch one custom field's value on demand; protected custom fields are never
/// part of `VaultState`/`VaultEntry`.
#[tauri::command]
pub(crate) fn get_custom_field_value(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    name: String,
) -> Result<Option<String>, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.get_custom_field_value(&uuid, &name),
    )
}

/// Server-side security analysis; no passwords leave the session.
#[tauri::command]
pub(crate) fn security_report(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<SecurityReport, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.security_report(),
    )
}

/// Group entries whose passwords are similar (server-side analysis; passwords
/// never cross IPC).
#[tauri::command]
pub(crate) fn similar_passwords(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<Vec<crate::vault::SimilarPasswordGroup>, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.similar_passwords(),
    )
}

/// Clear the stored history of every entry; returns how many entries had
/// history removed plus the refreshed state.
#[tauri::command]
pub(crate) fn clear_all_history(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<crate::vault::HistoryCleanResult, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.clear_all_history(),
    )
}

/// List entries whose expiry has passed (maintenance view; recycle bin
/// excluded, no secrets).
#[tauri::command]
pub(crate) fn expired_entries(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<Vec<crate::vault::ExpiredEntry>, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.expired_entries(),
    )
}

/// Vault-wide change timeline: transitions between consecutive snapshots,
/// newest first (recycle bin excluded, capped, no secrets).
#[tauri::command]
pub(crate) fn change_timeline(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<Vec<crate::vault::ChangeTimelineEvent>, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.change_timeline(),
    )
}

/// Check the selected (or every) entry's passwords against HIBP using
/// k-anonymity: only the first 5 hex chars of each SHA-1 leave the machine.
/// Strictly opt-in; network I/O runs off the async runtime.
#[tauri::command]
pub(crate) async fn check_hibp(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuids: Option<Vec<String>>,
) -> Result<Vec<crate::vault::BreachFinding>, String> {
    let entries = with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.hibp_entries(uuids.as_deref()),
    )?;
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("SecPivot/1.0 (HIBP k-anonymity range check)")
            .build()
            .map_err(|e| format!("构建 HIBP 客户端失败: {e}"))?;
        crate::vault::check_hibp(&entries, &client, crate::vault::HIBP_RANGE_URL)
    })
    .await
    .map_err(|e| format!("HIBP 任务异常: {e}"))?
}

/// Byte-size breakdown of an entry's stored data (fields, attachments, history).
#[tauri::command]
pub(crate) fn get_entry_storage(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<EntryStorage, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.get_entry_storage(&uuid),
    )
}

/// Export all entries as CSV to a user-picked path (passwords resolved server-side).
#[tauri::command]
pub(crate) fn export_csv(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    path: String,
) -> Result<(), String> {
    // Build the payload under the lock, write the file outside it.
    let content = with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.export_csv_content(),
    )?;
    vault::write_csv_file(&path, &content)
}

/// Export a self-contained HTML emergency sheet. `include_passwords` embeds
/// plaintext passwords and must only be set after an explicit user choice
/// (the UI shows a warning).
#[tauri::command]
pub(crate) fn export_emergency_sheet(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    path: String,
    include_passwords: bool,
) -> Result<(), String> {
    // Build the payload under the lock, write the file outside it.
    let content = with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.emergency_sheet_content(include_passwords),
    )?;
    vault::write_csv_file(&path, &content)
}

/// Read a UTF-8 text file from a user-picked path (CSV / KeePass XML import).
/// Only `.csv` and `.xml` files are accepted: the command must never serve as
/// an arbitrary local file reader (e.g. for config.json, credentials, or other
/// vaults).
#[tauri::command]
pub(crate) fn read_text_file(path: String) -> Result<String, String> {
    let path = Path::new(&path);
    let allowed = path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        e.eq_ignore_ascii_case("csv")
            || e.eq_ignore_ascii_case("xml")
            || e.eq_ignore_ascii_case("json")
            || e.eq_ignore_ascii_case("1pif")
    });
    if !allowed {
        return Err("仅支持导入 .csv / .xml / .json / .1pif 文件".to_owned());
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

/// Parse a Bitwarden `.json` export into normalized import rows. Strict:
/// malformed documents fail; cards/identities are skipped, logins and secure
/// notes map to rows (folders become the group path).
#[tauri::command]
pub(crate) fn parse_bitwarden_json(text: String) -> Result<Vec<crate::vault::ImportRow>, String> {
    crate::vault::parse_bitwarden_json(&text)
}

/// Parse a 1Password Interchange Format (`.1pif`) text export into rows.
#[tauri::command]
pub(crate) fn parse_1pif(text: String) -> Result<Vec<crate::vault::ImportRow>, String> {
    Ok(crate::vault::parse_1pif(&text))
}
