//! Local mirror helpers (`save to local` mode; extracted from remote/mod.rs).

use std::path::PathBuf;
use tauri::Manager;
// ---------------------------------------------------------------------------
// Local mirror helpers ("保存到本地" mode)
// ---------------------------------------------------------------------------

/// Base directory for local copies:
/// `<app_data>/Storage/remote/<kind>/<profile_name>`. The input is the canonical
/// profile path (`s3/config_1` or `webdav/config_1`).
pub fn local_storage_dir(app: &tauri::AppHandle, profile_path: &str) -> Result<PathBuf, String> {
    let base = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法定位应用数据目录: {e}"))?;
    let (kind, name) = profile_storage_parts(profile_path)?;
    Ok(base.join("Storage").join("remote").join(kind).join(name))
}

pub(crate) fn profile_storage_parts(profile_path: &str) -> Result<(&str, String), String> {
    let (kind, name) = profile_path
        .split_once('/')
        .ok_or_else(|| format!("远程配置路径无效: {profile_path}"))?;
    if kind != "s3" && kind != "webdav" {
        return Err(format!("远程配置类型无效: {kind}"));
    }
    if name.is_empty() || name.contains('/') || name.contains('\\') {
        return Err(format!("远程配置名称无效: {name}"));
    }
    Ok((kind, sanitize_dir_name(name)))
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
