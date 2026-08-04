//! TCATO (two-channel auto-type overlay) commands + managed target state
//! (extracted from commands.rs).

use crate::focus;
use crate::vault::VaultSession;
use std::sync::Mutex;
use tauri::Manager;
// ---------------------------------------------------------------------------
// TCATO (two-channel auto-type overlay)
// ---------------------------------------------------------------------------

/// UUID of the entry the TCATO overlay currently targets; never the password.
pub(crate) struct TcatoTarget(pub(crate) Mutex<Option<String>>);

/// Lightweight info shown in the TCATO overlay; secrets never leave the
/// backend, so the password itself is only reported as a boolean.
#[derive(serde::Serialize)]
pub(crate) struct TcatoInfo {
    title: String,
    username: String,
    has_password: bool,
}

const TCATO_WINDOW_LABEL: &str = "tcato";

/// Open (or focus) the small always-on-top overlay that sends one channel of
/// credentials to the window in focus without simulated key presses.
#[tauri::command]
pub(crate) fn open_tcato_overlay(
    app: tauri::AppHandle,
    session: tauri::State<'_, Mutex<VaultSession>>,
    target: tauri::State<'_, TcatoTarget>,
    uuid: String,
) -> Result<(), String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .autotype_context(&uuid)?;
    let mut slot = target.0.lock().map_err(|_| "覆盖层状态已损坏".to_owned())?;
    *slot = Some(uuid);
    drop(slot);
    if let Some(window) = app.get_webview_window(TCATO_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }
    tauri::WebviewWindowBuilder::new(
        &app,
        TCATO_WINDOW_LABEL,
        tauri::WebviewUrl::App("index.html".into()),
    )
    .title("TCATO 两通道填充")
    .inner_size(360.0, 190.0)
    .min_inner_size(360.0, 190.0)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .initialization_script("window.location.hash = '#/tcato';")
    .build()
    .map_err(|e| format!("无法打开 TCATO 窗口: {e}"))?;
    Ok(())
}

/// Info for the overlay UI: entry title and which channels are available.
#[tauri::command]
pub(crate) fn tcato_state(
    session: tauri::State<'_, Mutex<VaultSession>>,
    target: tauri::State<'_, TcatoTarget>,
) -> Result<Option<TcatoInfo>, String> {
    let uuid = target
        .0
        .lock()
        .map_err(|_| "覆盖层状态已损坏".to_owned())?
        .clone();
    let Some(uuid) = uuid else { return Ok(None) };
    let ctx = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .autotype_context(&uuid)?;
    Ok(Some(TcatoInfo {
        title: ctx.title,
        username: ctx.username,
        has_password: !ctx.password.is_empty(),
    }))
}

/// Send one channel (`username` or `password`) to the window in focus.
#[tauri::command]
pub(crate) fn tcato_send(
    session: tauri::State<'_, Mutex<VaultSession>>,
    target: tauri::State<'_, TcatoTarget>,
    channel: String,
) -> Result<(), String> {
    let uuid = target
        .0
        .lock()
        .map_err(|_| "覆盖层状态已损坏".to_owned())?
        .clone()
        .ok_or_else(|| "TCATO 覆盖层尚未指定条目".to_owned())?;
    let ctx = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .autotype_context(&uuid)?;
    let text = match channel.as_str() {
        "username" => ctx.username,
        "password" => ctx.password,
        _ => return Err("无效的 TCATO 通道".to_owned()),
    };
    focus::send_text_to_foreground(&text)
}

/// Close the TCATO overlay.
#[tauri::command]
pub(crate) fn close_tcato_overlay(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(TCATO_WINDOW_LABEL) {
        let _ = window.close();
    }
}
