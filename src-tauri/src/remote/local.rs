//! Local mirror helpers (`save to local` mode; extracted from remote/mod.rs).

use std::path::PathBuf;
use tauri::Manager;
// ---------------------------------------------------------------------------
// Local mirror helpers ("保存到本地" mode)
// ---------------------------------------------------------------------------

/// Base directory for local copies: `<app_data>/Storage/remote/<profile_name>`.
/// `local_dir` is the remote profile name (the frontend's 远程名/配置名); the
/// name is sanitized so it cannot escape the remote storage tree.
pub fn local_storage_dir(app: &tauri::AppHandle, local_dir: &str) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位应用数据目录: {e}"))?;
    let name = sanitize_dir_name(local_dir);
    Ok(base.join("Storage").join("remote").join(name))
}

/// Sanitize a profile name into a safe folder name: keeps letters/digits
/// (Unicode-aware, so Chinese names survive) plus `-`/`_`; anything else
/// (spaces, `.`, `/`, `\`, `:` …) becomes `_`. Empty/whitespace → `remote`.
pub(crate) fn sanitize_dir_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return "remote".to_owned();
    }
    let mut out = String::new();
    for ch in trimmed.chars() {
        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
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
