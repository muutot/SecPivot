//! IPC command for opening an entry URL through the KeePass override chain.
//!
//! Resolution order (KeePass semantics):
//! 1. The entry's `OverrideURL` template, when set (entry-specific).
//! 2. The single global template, gated only by UrlOverrideEnabled.
//! 3. Expand, then match enabled built-in/custom scheme rules once.
//! 4. An unchanged plain URL returns to the frontend system handler.
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
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, PartialEq, Eq)]
enum LaunchTarget {
    Command { program: String, args: Vec<String> },
    Url(String),
}

fn launch_target(compiled: url_override::CompiledUrl) -> Result<LaunchTarget, String> {
    let url = compiled.url.trim_start();
    if url
        .get(..6)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("cmd://"))
    {
        if !compiled.allow_command {
            return Err("条目 URL 不能直接执行命令；请使用显式 URL 覆盖".to_owned());
        }
        let (program, args) = url_override::parse_command_line(&url[6..])?;
        return Ok(LaunchTarget::Command { program, args });
    }
    let valid = url::Url::parse(url).is_ok_and(|parsed| {
        matches!(parsed.scheme(), "http" | "https") && parsed.host_str().is_some()
    });
    if !valid {
        return Err("URL 覆盖只允许 http/https 或 cmd:// 命令".to_owned());
    }
    Ok(LaunchTarget::Url(url.to_owned()))
}

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

/// Open one entry's URL honoring entry/global templates and scheme overrides.
#[tauri::command]
pub(crate) fn open_entry_url(
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<crate::vault::VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<UrlOpenResult, String> {
    let (ctx, entry_override, integration) = {
        let cfg = config.get()?;
        let integration = cfg.integration.clone();
        let data = with_vault_session(
            vaults.inner(),
            session.inner(),
            session_id.as_deref(),
            |target| {
                let ctx = target.autotype_context(&uuid)?;
                let entry_override = target.get_entry_override_url(&uuid)?;
                Ok((ctx, entry_override))
            },
        )?;
        (data.0, data.1, integration)
    };
    // Ordinary literal URLs need no browser lookup or DPAPI operation.
    if ctx.url.trim().is_empty()
        || (entry_override.is_none()
            && (!integration.url_override_enabled || integration.url_override.trim().is_empty())
            && !ctx.url.contains('{')
            && url_override::resolve_override(&integration, &ctx.url).is_none())
    {
        return Ok(UrlOpenResult { handled: false });
    }

    let ovr_ctx = UrlOverrideContext {
        app_dir: std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_string_lossy().into_owned()))
            .unwrap_or_default(),
        browsers: url_override::detect_browser_paths(),
        password_enc: url_override::password_enc(&ctx.password)
            .map_err(|_| "无法生成加密密码占位符".to_owned())?,
    };

    let Some(compiled) =
        url_override::compile_entry_url(&integration, entry_override.as_deref(), &ctx, &ovr_ctx)
            .map_err(|e| e.to_string())?
    else {
        return Ok(UrlOpenResult { handled: false });
    };
    match launch_target(compiled)? {
        LaunchTarget::Command { program, args } => {
            Command::new(program)
                .args(args)
                .spawn()
                .map_err(|_| "启动命令失败".to_owned())?;
        }
        LaunchTarget::Url(url) => {
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|_| "打开 URL 失败".to_owned())?;
        }
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

    fn compiled(url: &str, allow_command: bool) -> url_override::CompiledUrl {
        url_override::CompiledUrl {
            url: url.into(),
            allow_command,
        }
    }

    #[test]
    fn url_destination_is_preserved_as_one_opener_argument() {
        let url = "https://example.test/?a=1&b=%22&c=|calc";
        assert_eq!(
            launch_target(compiled(url, true)).unwrap(),
            LaunchTarget::Url(url.into())
        );
    }

    #[test]
    fn only_explicit_overrides_can_launch_commands() {
        assert_eq!(
            launch_target(compiled(
                "CMD://\"C:\\My App\\client.exe\" \"https://example.test/?a=1&b=2\"",
                true
            ))
            .unwrap(),
            LaunchTarget::Command {
                program: "C:\\My App\\client.exe".into(),
                args: vec!["https://example.test/?a=1&b=2".into()]
            }
        );
        assert!(launch_target(compiled("cmd://client", false)).is_err());
    }

    #[test]
    fn unsafe_empty_and_malformed_destinations_fail_closed() {
        for url in [
            "",
            "file:///C:/example",
            "javascript:alert(1)",
            "https://",
            "cmd://",
            "cmd://client \"unterminated",
        ] {
            assert!(
                launch_target(compiled(url, true)).is_err(),
                "accepted synthetic destination"
            );
        }
    }
}
