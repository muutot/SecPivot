//! IPC command for opening an entry URL through the KeePass override chain.
//!
//! Resolution order (KeePass semantics):
//! 1. The entry's `OverrideURL` template, when set (entry-specific).
//! 2. Global scheme overrides: enabled built-ins, then custom rules.
//! 3. No override → the frontend opens the URL with the system handler.
//!
//! Only `http:`/`https:` are allowed without an override (defense against
//! attacker-controlled `javascript:`/`file:` URLs in shared vaults); an
//! override represents an explicit user choice and is executed as a command
//! when it uses the `cmd://` virtual scheme.

use super::{with_vault_session, VaultSessions};
use crate::config::ConfigStore;
use crate::platform::url_override::{self, UrlOverrideContext};
use std::process::Command;
use std::sync::Mutex;

/// One built-in override row for the settings panel (read-only metadata).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BuiltInOverrideInfo {
    pub flag: u64,
    pub scheme: String,
    pub url_override: String,
}

/// The 27 official built-in overrides, in KeePass source order.
#[tauri::command]
pub(crate) fn list_builtin_url_overrides() -> Vec<BuiltInOverrideInfo> {
    crate::config::settings::BUILT_IN_URL_OVERRIDES
        .iter()
        .map(|o| BuiltInOverrideInfo {
            flag: o.flag,
            scheme: o.scheme.to_owned(),
            url_override: o.url_override.to_owned(),
        })
        .collect()
}

/// What the frontend should do after this command returns.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UrlOpenResult {
    /// `true` when a command/URL override was executed here; `false` means the
    /// frontend should open the plain URL with its normal handler.
    pub handled: bool,
}

/// Open one entry's URL honoring its OverrideURL and global scheme overrides.
#[tauri::command]
pub(crate) fn open_entry_url(
    config: tauri::State<'_, ConfigStore>,
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<crate::vault::VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<UrlOpenResult, String> {
    let (ctx, entry_override, password, integration) = {
        let cfg = config.get()?;
        let integration = cfg.integration.clone();
        let data = with_vault_session(
            vaults.inner(),
            session.inner(),
            session_id.as_deref(),
            |target| {
                let ctx = target.autotype_context(&uuid)?;
                let entry_override = target.get_entry_override_url(&uuid)?;
                let password = target.get_entry_password(&uuid)?;
                Ok((ctx, entry_override, password))
            },
        )?;
        (data.0, data.1, data.2, integration)
    };
    let url = ctx.url.clone();

    // Entry-level OverrideURL takes precedence over global scheme overrides.
    let template = if let Some(tpl) = entry_override {
        Some(tpl)
    } else {
        url_override::resolve_override(&integration, &url).map(str::to_owned)
    };
    let Some(template) = template else {
        return Ok(UrlOpenResult { handled: false });
    };

    let ovr_ctx = UrlOverrideContext {
        app_dir: std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_string_lossy().into_owned()))
            .unwrap_or_default(),
        browsers: url_override::detect_browser_paths(),
        password_enc: url_override::password_enc(&password)?,
    };

    let expanded =
        url_override::expand(&template, &url, &ctx, &ovr_ctx).map_err(|e| e.to_string())?;

    if let Some(command) = expanded.strip_prefix("cmd://") {
        let (program, args) = url_override::parse_command_line(command)?;
        Command::new(program)
            .args(args)
            .spawn()
            .map_err(|e| format!("启动命令失败: {e}"))?;
    } else {
        // A plain URL override: only http/https may reach the OS handler.
        let scheme = url_override::scheme_of(&expanded).to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err("URL 覆盖只允许 http/https 或 cmd:// 命令".to_owned());
        }
        Command::new("cmd")
            .args(["/C", "start", "", &expanded])
            .spawn()
            .map_err(|e| format!("打开 URL 失败: {e}"))?;
    }
    Ok(UrlOpenResult { handled: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_command_line_groups_quoted_arguments() {
        let (program, args) =
            url_override::parse_command_line("mstsc.exe /v:host /f \"C:\\My Files\\Conn.rdp\"")
                .unwrap();
        assert_eq!(program, "mstsc.exe");
        assert_eq!(args, vec!["/v:host", "/f", "C:\\My Files\\Conn.rdp"]);
    }

    #[test]
    fn parse_command_line_rejects_unclosed_quote_and_empty_program() {
        assert!(url_override::parse_command_line("notepad.exe \"unterminated").is_err());
        assert!(url_override::parse_command_line("   ").is_err());
    }
}
