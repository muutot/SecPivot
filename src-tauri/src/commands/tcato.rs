//! TCATO (two-channel auto-type overlay) commands + managed target state
//! (extracted from commands.rs).

use super::with_vault_session;
use crate::platform::focus;
use crate::vault::{VaultSession, VaultSessions};
use std::sync::Mutex;
use tauri::Emitter;
use tauri::Manager;
// ---------------------------------------------------------------------------
// TCATO (two-channel auto-type overlay)
// ---------------------------------------------------------------------------

/// Stable session + entry target of the TCATO overlay; never the password.
/// `pending` holds multi-match summon candidates for pick mode (cleared
/// whenever an explicit target is set or the overlay closes).
pub(crate) struct TcatoTarget {
    target: Mutex<Option<(String, String)>>,
    pending: Mutex<Vec<crate::vault::AutotypeCandidate>>,
}

impl TcatoTarget {
    /// Empty target with no pending candidates.
    pub(crate) fn new() -> Self {
        Self {
            target: Mutex::new(None),
            pending: Mutex::new(Vec::new()),
        }
    }

    /// Session id of the overlay's current target, if any.
    pub(crate) fn target_session(&self) -> Option<String> {
        self.target
            .lock()
            .ok()
            .and_then(|slot| slot.clone().map(|(sid, _)| sid))
    }

    /// Clone the current (session, entry) target, if any.
    pub(crate) fn current_target(&self) -> Option<(String, String)> {
        self.target.lock().ok().and_then(|slot| slot.clone())
    }
}

/// Lightweight info shown in the TCATO overlay; secrets never leave the
/// backend, so the password itself is only reported as a boolean.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TcatoInfo {
    title: String,
    username: String,
    has_password: bool,
    has_username: bool,
    has_totp: bool,
    last_window: Option<String>,
    /// Multi-match summon candidates; non-empty means the overlay renders
    /// pick mode instead of the inject buttons.
    pending: Vec<crate::vault::AutotypeCandidate>,
}

pub(crate) const TCATO_WINDOW_LABEL: &str = "tcato";

/// Emitted to the main window when the TCATO overlay is closed, so its
/// focus-loss lock re-arms.
pub(crate) const TCATO_CLOSE_EVENT: &str = "tcato-overlay-close";

/// Emitted to the main window when the TCATO overlay is open; desktop-only
/// (the overlay window itself does not exist on mobile).
#[cfg(desktop)]
pub(crate) const TCATO_OPEN_EVENT: &str = "tcato-overlay-open";

/// Open (or re-show) the small always-on-top overlay that sends one channel of
/// credentials to the window in focus without simulated key presses. Reopening
/// never takes focus: the existing window is only shown, so the target
/// application keeps keyboard focus (the overlay is `WS_EX_NOACTIVATE`).
#[tauri::command]
pub(crate) fn open_tcato_overlay(
    app: tauri::AppHandle,
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    target: tauri::State<'_, TcatoTarget>,
    session_id: String,
    uuid: String,
) -> Result<(), String> {
    open_tcato_overlay_for(
        &app,
        vaults.inner(),
        session.inner(),
        target.inner(),
        session_id,
        uuid,
    )
}

/// Shared backend for `open_tcato_overlay` and the global summon hotkey:
/// validates the target against the addressed session, records it, and shows
/// (or creates) the overlay window.
pub(crate) fn open_tcato_overlay_for(
    app: &tauri::AppHandle,
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    target: &TcatoTarget,
    session_id: String,
    uuid: String,
) -> Result<(), String> {
    with_vault_session(vaults, session, Some(&session_id), |target| {
        target.ensure_tcato_allowed(&uuid)?;
        target.autotype_context(&uuid).map(|_| ())
    })?;
    let mut slot = target
        .target
        .lock()
        .map_err(|_| "覆盖层状态已损坏".to_owned())?;
    *slot = Some((session_id, uuid));
    drop(slot);
    if let Ok(mut pending) = target.pending.lock() {
        pending.clear();
    }
    show_tcato_window(app)
}

/// Multi-match summon path: stash the candidates for pick mode (clearing any
/// previous target) and show the overlay; the chosen entry re-enters through
/// `open_tcato_overlay`.
pub(crate) fn open_tcato_pick(
    app: &tauri::AppHandle,
    target: &TcatoTarget,
    candidates: Vec<crate::vault::AutotypeCandidate>,
) -> Result<(), String> {
    if let Ok(mut slot) = target.target.lock() {
        *slot = None;
    }
    *target
        .pending
        .lock()
        .map_err(|_| "覆盖层状态已损坏".to_owned())? = candidates;
    show_tcato_window(app)
}

/// Show the overlay window without touching the target: shared by the
/// target path and the multi-match pick path.
fn show_tcato_window(app: &tauri::AppHandle) -> Result<(), String> {
    #[cfg(desktop)]
    {
        if let Some(window) = app.get_webview_window(TCATO_WINDOW_LABEL) {
            let _ = window.show();
            focus::apply_tcato_no_activate(&window);
            let _ = app.emit(TCATO_OPEN_EVENT, ());
            return Ok(());
        }
        let window = tauri::WebviewWindowBuilder::new(
            app,
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
        focus::apply_tcato_no_activate(&window);
        let _ = app.emit(TCATO_OPEN_EVENT, ());
        Ok(())
    }
    #[cfg(not(desktop))]
    {
        let _ = app;
        Err("TCATO 两通道填充仅桌面端支持".to_owned())
    }
}

/// Info for the overlay UI: entry title and which channels are available.
#[tauri::command]
pub(crate) fn tcato_state(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    target: tauri::State<'_, TcatoTarget>,
) -> Result<Option<TcatoInfo>, String> {
    let target_ref = target
        .target
        .lock()
        .map_err(|_| "覆盖层状态已损坏".to_owned())?
        .clone();
    let pending = target
        .pending
        .lock()
        .map_err(|_| "覆盖层状态已损坏".to_owned())?
        .clone();
    let Some((session_id, uuid)) = target_ref else {
        if pending.is_empty() {
            return Ok(None);
        }
        // Pick mode: no target yet, the overlay lists the candidates.
        return Ok(Some(TcatoInfo {
            title: String::new(),
            username: String::new(),
            has_password: false,
            has_username: false,
            has_totp: false,
            last_window: None,
            pending,
        }));
    };
    let (ctx, has_totp, last_window) = with_vault_session(
        vaults.inner(),
        session.inner(),
        Some(&session_id),
        |target| {
            target.ensure_tcato_allowed(&uuid)?;
            let ctx = target.autotype_context(&uuid)?;
            let has_totp = target.entry_has_totp(&uuid)?;
            let last_window = target.tcato_last_window(&uuid);
            Ok((ctx, has_totp, last_window))
        },
    )?;
    Ok(Some(TcatoInfo {
        title: ctx.title.clone(),
        username: ctx.username.clone(),
        has_password: !ctx.password.is_empty(),
        has_username: !ctx.username.is_empty(),
        has_totp,
        last_window,
        pending: Vec::new(),
    }))
}

/// Send one channel (`username`, `password`, or `totp`) to the window in focus.
#[tauri::command]
pub(crate) fn tcato_send(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    target: tauri::State<'_, TcatoTarget>,
    channel: String,
) -> Result<(), String> {
    let (session_id, uuid) = target
        .current_target()
        .ok_or_else(|| "TCATO 覆盖层尚未指定条目".to_owned())?;
    let text = match channel.as_str() {
        "username" | "password" => {
            let ctx = with_vault_session(
                vaults.inner(),
                session.inner(),
                Some(&session_id),
                |target| {
                    target.ensure_tcato_allowed(&uuid)?;
                    target.autotype_context(&uuid)
                },
            )?;
            let text = if channel == "username" {
                ctx.username
            } else {
                ctx.password
            };
            if text.is_empty() {
                return Err(if channel == "username" {
                    "用户名为空，无法注入".to_owned()
                } else {
                    "密码为空，无法注入".to_owned()
                });
            }
            text
        }
        "totp" => with_vault_session(
            vaults.inner(),
            session.inner(),
            Some(&session_id),
            |target| {
                target.ensure_tcato_allowed(&uuid)?;
                // HOTP counters advance here with the same semantics as the
                // detail widget; the vault is marked dirty so the next
                // explicit save persists them.
                target.totp_code(&uuid).map(|code| code.code)
            },
        )?,
        _ => return Err("无效的 TCATO 通道".to_owned()),
    };
    if let Some(window_title) = focus::foreground_window_title() {
        let _ = with_vault_session(
            vaults.inner(),
            session.inner(),
            Some(&session_id),
            |target| {
                target.note_tcato_window(&uuid, &window_title);
                Ok::<(), String>(())
            },
        );
    }
    focus::send_text_to_foreground(&text)
}

/// Clear the in-memory TCATO target (session + entry) and any pending pick
/// candidates, without touching the window.
pub(crate) fn clear_tcato_target(app: &tauri::AppHandle) {
    if let Some(target) = app.try_state::<TcatoTarget>() {
        if let Ok(mut slot) = target.target.lock() {
            *slot = None;
        }
        if let Ok(mut pending) = target.pending.lock() {
            pending.clear();
        }
    }
}

/// Close the TCATO overlay.
#[tauri::command]
pub(crate) fn close_tcato_overlay(app: tauri::AppHandle) {
    clear_tcato_target(&app);
    if let Some(window) = app.get_webview_window(TCATO_WINDOW_LABEL) {
        let _ = window.close();
    }
    let _ = app.emit(TCATO_CLOSE_EVENT, ());
}
