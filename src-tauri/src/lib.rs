pub mod autotype;
pub mod bridge;
pub mod bridge_server;
pub mod clipboard;
pub mod config;
pub mod credential;
pub mod dpapi;
pub mod focus;
pub mod remote;
pub mod rpc;
pub mod rpc_server;
pub mod shield;
pub mod vault;

use crate::bridge::BridgeHost;
use crate::config::ConfigStore;
use crate::remote::{local_storage_dir, RemoteObject, RemoteStorage, S3Storage};
use crate::vault::{
    EntryInput, EntryPatch, GroupInput, HistoryVersion, RemoteMode, SecurityReport, TotpCode,
    VaultSession, VaultState,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use zeroize::Zeroize;

/// Sequence replayed by the global auto-type hotkey.
const GLOBAL_AUTOTYPE_SEQUENCE: &str = "{USERNAME}{TAB}{PASSWORD}{ENTER}";

/// CSV import cap (8 MiB): guards the read-text command against oversized
/// files; the `.csv` extension whitelist stops arbitrary file exfiltration.
const MAX_CSV_IMPORT_BYTES: u64 = 8 * 1024 * 1024;

// ---------------------------------------------------------------------------
// Config commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn get_config(store: tauri::State<'_, ConfigStore>) -> Result<config::AppConfig, String> {
    store.get()
}

#[tauri::command]
fn set_config(
    store: tauri::State<'_, ConfigStore>,
    app: tauri::AppHandle,
    config: config::AppConfig,
) -> Result<config::AppConfig, String> {
    let saved = store.set(config)?;
    register_global_hotkey(&app, &saved.keyboard.auto_type_global);
    sync_bridge(&app, &saved);
    sync_rpc(&app, &saved);
    Ok(saved)
}

/// Start or stop the loopback bridge to match `bridge.enabled`; failures are
/// logged, never fatal (the app stays usable without browser integration).
fn sync_bridge(app: &tauri::AppHandle, config: &config::AppConfig) {
    let state = app.state::<bridge_server::BridgeState>();
    if config.bridge.enabled {
        if let Err(e) = state.start(app) {
            eprintln!("bridge: {e}");
        }
    } else {
        state.stop();
    }
}

/// Start or stop the loopback KeePassRPC server to match `rpc.enabled`;
/// failures are logged, never fatal (the app stays usable without Kee).
fn sync_rpc(app: &tauri::AppHandle, config: &config::AppConfig) {
    let state = app.state::<rpc_server::RpcState>();
    if config.rpc.enabled {
        if let Err(e) = state.start(app) {
            eprintln!("rpc: {e}");
        }
    } else {
        state.stop();
    }
}

// ---------------------------------------------------------------------------
// Browser bridge commands (KeePassHttp)
// ---------------------------------------------------------------------------

/// Whether the loopback server is currently listening.
#[derive(serde::Serialize)]
struct BridgeStatus {
    running: bool,
    port: u16,
    error: Option<String>,
}

#[tauri::command]
fn bridge_status(
    state: tauri::State<'_, bridge_server::BridgeState>,
) -> Result<BridgeStatus, String> {
    Ok(BridgeStatus {
        running: state.running(),
        port: bridge::BRIDGE_PORT,
        error: state.last_error(),
    })
}

/// Whether the KeePassRPC loopback server is currently listening.
#[derive(serde::Serialize)]
struct RpcStatus {
    running: bool,
    port: u16,
    error: Option<String>,
}

#[tauri::command]
fn rpc_status(state: tauri::State<'_, rpc_server::RpcState>) -> Result<RpcStatus, String> {
    Ok(RpcStatus {
        running: state.running(),
        port: rpc::RPC_PORT,
        error: state.last_error(),
    })
}

/// Authorized browser clients of the open session (id only — never keys).
#[tauri::command]
fn bridge_clients(session: tauri::State<'_, Mutex<VaultSession>>) -> Result<Vec<String>, String> {
    Ok(session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .list_clients())
}

/// Deauthorize one browser client; returns the remaining list.
#[tauri::command]
fn bridge_remove_client(
    session: tauri::State<'_, Mutex<VaultSession>>,
    id: String,
) -> Result<Vec<String>, String> {
    let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    if !session.remove_client(&id) {
        return Err("未找到该客户端".to_owned());
    }
    Ok(session.list_clients())
}

/// Answer a pending browser-association approval from the settings UI.
#[tauri::command]
fn bridge_approve(
    board: tauri::State<'_, bridge_server::ApprovalBoard>,
    token: String,
    allowed: bool,
) -> Result<(), String> {
    if board.decide(&token, allowed) {
        Ok(())
    } else {
        Err("审批已过期或不存在".to_owned())
    }
}

// ---------------------------------------------------------------------------
// Global auto-type hotkey
// ---------------------------------------------------------------------------

/// Register (or replace) the global hotkey from `shortcut`; empty disables it.
/// Failure to register is logged, never fatal: the app stays usable.
fn register_global_hotkey(app: &tauri::AppHandle, shortcut: &str) {
    let global = app.global_shortcut();
    if let Err(e) = global.unregister_all() {
        eprintln!("failed to unregister global shortcuts: {e}");
        return;
    }
    let shortcut = shortcut.trim();
    if shortcut.is_empty() {
        return;
    }
    let result = global.on_shortcut(shortcut, |app, _shortcut, event| {
        if event.state == ShortcutState::Pressed {
            handle_global_hotkey(app);
        }
    });
    if let Err(e) = result {
        eprintln!("failed to register global auto-type hotkey `{shortcut}`: {e}");
    }
}

/// Replay the auto-type sequence of the entry matching the focused window.
/// Runs on a background thread; failures are logged only.
fn handle_global_hotkey(app: &tauri::AppHandle) {
    let Some(window_title) = focus::foreground_window_title() else {
        return;
    };
    let Some(session) = app.try_state::<Mutex<VaultSession>>() else {
        return;
    };
    let ctx = {
        let session = match session.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let uuid = match session.autotype_match(&window_title) {
            Ok(uuid) => uuid,
            Err(e) => {
                eprintln!("global auto-type: {e}");
                return;
            }
        };
        match session.autotype_context(&uuid) {
            Ok(ctx) => ctx,
            Err(e) => {
                eprintln!("global auto-type: {e}");
                return;
            }
        }
    };
    std::thread::spawn(move || {
        if let Err(e) = autotype::run_sequence(GLOBAL_AUTOTYPE_SEQUENCE, &ctx) {
            eprintln!("global auto-type: {e}");
        }
    });
}

// ---------------------------------------------------------------------------
// Credential-store commands (Windows Hello quick unlock)
// ---------------------------------------------------------------------------

/// Store the master password for a vault path in the OS credential store.
#[tauri::command]
fn remember_credential(path: String, mut password: String) -> Result<(), String> {
    let result = credential::remember(&path, &password);
    password.zeroize();
    result
}

/// Fetch the stored master password for a vault path, if any.
#[tauri::command]
fn get_saved_credential(path: String) -> Result<Option<String>, String> {
    credential::get(&path)
}

/// Remove the stored master password for a vault path.
#[tauri::command]
fn clear_saved_credential(path: String) -> Result<(), String> {
    credential::forget(&path)
}

// ---------------------------------------------------------------------------
// Vault commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn open_vault(
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
    mut password: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    // Slow work (file read, KDF, parse) runs outside the session lock; only
    // the state adoption is locked.
    let prepared = vault::prepare_local_open(
        Path::new(&path),
        &password,
        keyfile.as_deref().map(Path::new),
    );
    let result = match prepared {
        Ok((db, keyfile_bytes)) => {
            let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            let result = session.adopt_local(db, Path::new(&path), &password, keyfile_bytes);
            drop(session);
            result
        }
        Err(e) => Err(e),
    };
    password.zeroize();
    if result.is_ok() {
        apply_capture_guard(&app, &config);
    }
    result
}

/// Apply the screen-capture guard only when the user enabled it in settings.
fn apply_capture_guard(app: &tauri::AppHandle, config: &ConfigStore) {
    let Ok(cfg) = config.get() else { return };
    if cfg.security.screen_capture_guard {
        shield::set_capture_guard(app, true);
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn create_vault(
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
    mut password: String,
    kdf: String,
    cipher: String,
    compression: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    // Slow work (KDF, serialization, file write) runs outside the session
    // lock; only the state adoption is locked.
    let prepared = vault::prepare_local_create(
        Path::new(&path),
        &password,
        &kdf,
        &cipher,
        &compression,
        keyfile.as_deref().map(Path::new),
    );
    let result = match prepared {
        Ok((db, keyfile_bytes)) => {
            let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            let result = session.adopt_local(db, Path::new(&path), &password, keyfile_bytes);
            drop(session);
            result
        }
        Err(e) => Err(e),
    };
    password.zeroize();
    if result.is_ok() {
        apply_capture_guard(&app, &config);
    }
    result
}

#[tauri::command]
fn close_vault(
    app: tauri::AppHandle,
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<(), String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .close();
    shield::set_capture_guard(&app, false);
    Ok(())
}

#[tauri::command]
fn get_vault_state(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<Option<VaultState>, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .state()
}

#[tauri::command]
fn save_vault(session: tauri::State<'_, Mutex<VaultSession>>) -> Result<VaultState, String> {
    // Capture a cheap job under the lock, then run KDF + serialization +
    // transport outside it, then mark clean under the lock again.
    let job = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .prepare_save()?;
    let revision = job.revision;
    vault::persist_save(job)?;
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .complete_save(revision)
}

#[tauri::command]
fn save_vault_as(
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .save_as(Path::new(&path))
}

#[tauri::command]
fn change_master_key(
    session: tauri::State<'_, Mutex<VaultSession>>,
    password: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    // Keyfile read, KDF and persistence all happen without the session lock.
    let keyfile_bytes = vault::read_keyfile(keyfile.as_deref().map(Path::new))?;
    let (db, target, revision) = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .prepare_change()?;
    let persisted = vault::persist_change(&db, &password, keyfile_bytes.as_deref(), &target);
    let result = match persisted {
        Ok(()) => session
            .lock()
            .map_err(|_| "数据库锁已损坏".to_owned())?
            .complete_change(password, keyfile_bytes, revision),
        Err(e) => Err(e),
    };
    result
}

#[tauri::command]
fn add_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    input: EntryInput,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .add_entry(&input)
}

#[tauri::command]
fn update_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    input: EntryInput,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .update_entry(&uuid, &input)
}

#[tauri::command]
fn update_entries(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuids: Vec<String>,
    patch: EntryPatch,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .update_entries(&uuids, &patch)
}

#[tauri::command]
fn delete_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .delete_entry(&uuid)
}

#[tauri::command]
fn move_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    group_uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .move_entry(&uuid, &group_uuid)
}

#[tauri::command]
fn delete_entries(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuids: Vec<String>,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .delete_entries(&uuids)
}

#[tauri::command]
fn get_entry_history(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<Vec<HistoryVersion>, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .get_entry_history(&uuid)
}

#[tauri::command]
fn restore_entry_version(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    index: usize,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .restore_entry_version(&uuid, index)
}

#[tauri::command]
fn restore_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .restore_entry(&uuid)
}

#[tauri::command]
fn save_attachment(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    name: String,
    dest: String,
) -> Result<(), String> {
    // Extract under the lock, write the file outside it.
    let data = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .attachment_data(&uuid, &name)?;
    vault::write_attachment_file(&data, &dest)
}

#[tauri::command]
fn totp_code(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<TotpCode, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .totp_code(&uuid)
}

#[tauri::command]
fn toggle_favorite(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .toggle_favorite(&uuid)
}

/// Resolve and replay a KeePass-style auto-type sequence for an entry.
/// Executes on a background thread; returns once parsing succeeds.
///
/// The main window is minimized first so keystrokes land in the window the
/// user switches to during the replay delay, never in KeyVault itself.
#[tauri::command]
fn auto_type(
    app: tauri::AppHandle,
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    sequence: String,
) -> Result<(), String> {
    let (ctx, expanded) = {
        let session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        let ctx = session.autotype_context(&uuid)?;
        let expanded = session.expand_autotype_sequence(&sequence)?;
        (ctx, expanded)
    };
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.minimize();
    }
    autotype::run_sequence(&expanded, &ctx).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// TCATO (two-channel auto-type overlay)
// ---------------------------------------------------------------------------

/// UUID of the entry the TCATO overlay currently targets; never the password.
struct TcatoTarget(Mutex<Option<String>>);

/// Lightweight info shown in the TCATO overlay; secrets never leave the
/// backend, so the password itself is only reported as a boolean.
#[derive(serde::Serialize)]
struct TcatoInfo {
    title: String,
    username: String,
    has_password: bool,
}

const TCATO_WINDOW_LABEL: &str = "tcato";

/// Open (or focus) the small always-on-top overlay that sends one channel of
/// credentials to the window in focus without simulated key presses.
#[tauri::command]
fn open_tcato_overlay(
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
fn tcato_state(
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
fn tcato_send(
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
fn close_tcato_overlay(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window(TCATO_WINDOW_LABEL) {
        let _ = window.close();
    }
}

#[tauri::command]
fn add_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    input: GroupInput,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .add_group(&input)
}

#[tauri::command]
fn rename_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    name: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .rename_group(&uuid, &name)
}

#[tauri::command]
fn delete_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .delete_group(&uuid)
}

#[tauri::command]
fn restore_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .restore_group(&uuid)
}

#[tauri::command]
fn empty_recycle_bin(session: tauri::State<'_, Mutex<VaultSession>>) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .empty_recycle_bin()
}

/// Fetch one entry's password on demand; passwords are never part of `VaultState`.
#[tauri::command]
fn get_entry_password(
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
fn get_entry_totp(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<Option<String>, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .get_entry_totp(&uuid)
}

/// Server-side security analysis; no passwords leave the session.
#[tauri::command]
fn security_report(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<SecurityReport, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .security_report()
}

/// Export all entries as CSV to a user-picked path (passwords resolved server-side).
#[tauri::command]
fn export_csv(session: tauri::State<'_, Mutex<VaultSession>>, path: String) -> Result<(), String> {
    // Build the payload under the lock, write the file outside it.
    let content = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .export_csv_content()?;
    vault::write_csv_file(&path, &content)
}

/// Read a UTF-8 text file from a user-picked path (CSV import). Only `.csv`
/// files are accepted: the command must never serve as an arbitrary local
/// file reader (e.g. for config.json, credentials, or other vaults).
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let path = Path::new(&path);
    let is_csv = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("csv"));
    if !is_csv {
        return Err("仅支持导入 .csv 文件".to_owned());
    }
    let meta = std::fs::metadata(path).map_err(|e| format!("读取文件失败: {e}"))?;
    if meta.len() > MAX_CSV_IMPORT_BYTES {
        return Err(format!(
            "CSV 文件过大 (最大 {} MiB)",
            MAX_CSV_IMPORT_BYTES / 1024 / 1024
        ));
    }
    std::fs::read_to_string(path).map_err(|e| format!("读取文件失败: {e}"))
}

// ---------------------------------------------------------------------------
// Clipboard commands
// ---------------------------------------------------------------------------

/// Current clipboard text (or `null` when it holds non-text content).
/// Used by the scheduled wipe to avoid destroying text the user copied in
/// another app after ours.
#[tauri::command]
fn clipboard_read_text() -> Result<Option<String>, String> {
    clipboard::read_clipboard_text()
}

/// Empty the clipboard. The frontend calls this only after verifying the
/// clipboard still holds our own text (or on lock with `clearOnLock`).
#[tauri::command]
fn clipboard_clear() -> Result<(), String> {
    clipboard::clear_clipboard()
}

// ---------------------------------------------------------------------------
// S3 remote commands
// ---------------------------------------------------------------------------

/// List `.kdbx` files under the active profile's prefix (newest key first).
/// `profile` is the index into `remoteProfiles`; settings (including the
/// decrypted credentials) are resolved from `ConfigStore`, never sent over IPC.
#[tauri::command]
async fn s3_list_objects(
    profile: usize,
    config: tauri::State<'_, ConfigStore>,
) -> Result<Vec<RemoteObject>, String> {
    let cfg = config.remote_settings(profile)?;
    crate::remote::list_objects_async(cfg).await
}

/// Download a vault from S3 and open it. `mode` is `"memory"` (upload back
/// only) or `"local"` (also mirror locally under `Storage/remote/<local_dir>`).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn open_remote_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    profile: usize,
    key: String,
    password: String,
    keyfile: Option<String>,
    mode: String,
) -> Result<VaultState, String> {
    let cfg = config.remote_settings(profile)?;
    let storage: Arc<dyn RemoteStorage> = Arc::new(S3Storage::new(&cfg)?);
    let mode = RemoteMode::parse(&mode)?;
    let local_dir = local_storage_dir(&app, &cfg.local_dir)?;
    let backup_count = cfg.backup_count.clamp(0, 10) as usize;
    let keyfile_path = keyfile.map(PathBuf::from);
    let local_dir_for_network = local_dir.clone();
    let storage_for_network = storage.clone();
    // Network download, KDF and parse run without the session lock, off the
    // async worker thread: the S3 transport blocks on its own runtime, which
    // panics on a runtime worker (the command future would abort and the
    // invoke would never resolve — the UI would stay on "正在加载…").
    let (prepared, mut password) = tauri::async_runtime::spawn_blocking(move || {
        let result = vault::prepare_remote_open(
            storage_for_network,
            &key,
            &password,
            keyfile_path.as_deref(),
            mode,
            &local_dir_for_network,
            backup_count,
        );
        (result, password)
    })
    .await
    .map_err(|e| format!("远程打开任务异常: {e}"))?;
    let result = match prepared {
        Ok((db, keyfile_bytes, key)) => {
            let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            let result = session.adopt_remote(
                db,
                storage,
                &key,
                &password,
                keyfile_bytes,
                mode,
                &local_dir,
                cfg.backup_count.clamp(0, 10) as usize,
            );
            drop(session);
            result
        }
        Err(e) => Err(e),
    };
    password.zeroize();
    if result.is_ok() {
        apply_capture_guard(&app, &config);
    }
    result
}

/// Create an empty vault and upload it to S3 immediately.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn create_remote_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    profile: usize,
    key: String,
    password: String,
    kdf: String,
    cipher: String,
    compression: String,
    keyfile: Option<String>,
    mode: String,
) -> Result<VaultState, String> {
    let cfg = config.remote_settings(profile)?;
    let storage: Arc<dyn RemoteStorage> = Arc::new(S3Storage::new(&cfg)?);
    let mode = RemoteMode::parse(&mode)?;
    let local_dir = local_storage_dir(&app, &cfg.local_dir)?;
    let backup_count = cfg.backup_count.clamp(0, 10) as usize;
    let keyfile_path = keyfile.map(PathBuf::from);
    let local_dir_for_network = local_dir.clone();
    let storage_for_network = storage.clone();
    // KDF, serialization, upload and local mirror run without the lock, off
    // the async worker thread (the S3 transport must not block a runtime
    // worker — see the comment in `open_remote_vault`).
    let (prepared, mut password) = tauri::async_runtime::spawn_blocking(move || {
        let result = vault::prepare_remote_create(
            storage_for_network,
            &key,
            &password,
            &kdf,
            &cipher,
            &compression,
            keyfile_path.as_deref(),
            mode,
            &local_dir_for_network,
            backup_count,
        );
        (result, password)
    })
    .await
    .map_err(|e| format!("远程创建任务异常: {e}"))?;
    let result = match prepared {
        Ok((db, keyfile_bytes, key)) => {
            let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            let result = session.adopt_remote(
                db,
                storage,
                &key,
                &password,
                keyfile_bytes,
                mode,
                &local_dir,
                cfg.backup_count.clamp(0, 10) as usize,
            );
            drop(session);
            result
        }
        Err(e) => Err(e),
    };
    password.zeroize();
    if result.is_ok() {
        apply_capture_guard(&app, &config);
    }
    result
}

// ---------------------------------------------------------------------------
// App entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(|app| {
            let project_dir = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| {
                    app.path()
                        .app_data_dir()
                        .unwrap_or_else(|_| PathBuf::from("."))
                });
            let store = ConfigStore::load(project_dir)?;
            let config = store.get()?;
            register_global_hotkey(app.handle(), &config.keyboard.auto_type_global);
            app.manage(store);
            app.manage(Mutex::new(VaultSession::default()));
            app.manage(TcatoTarget(Mutex::new(None)));
            app.manage(bridge_server::BridgeState::default());
            app.manage(bridge_server::ApprovalBoard::default());
            app.manage(rpc_server::RpcState::default());
            sync_bridge(app.handle(), &config);
            sync_rpc(app.handle(), &config);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            set_config,
            open_vault,
            create_vault,
            close_vault,
            get_vault_state,
            save_vault,
            save_vault_as,
            change_master_key,
            add_entry,
            update_entry,
            update_entries,
            delete_entry,
            delete_entries,
            move_entry,
            get_entry_history,
            restore_entry_version,
            restore_entry,
            delete_group,
            restore_group,
            empty_recycle_bin,
            save_attachment,
            totp_code,
            toggle_favorite,
            auto_type,
            open_tcato_overlay,
            tcato_state,
            tcato_send,
            close_tcato_overlay,
            add_group,
            rename_group,
            get_entry_password,
            get_entry_totp,
            security_report,
            export_csv,
            read_text_file,
            clipboard_read_text,
            clipboard_clear,
            remember_credential,
            get_saved_credential,
            clear_saved_credential,
            s3_list_objects,
            open_remote_vault,
            create_remote_vault,
            bridge_status,
            bridge_clients,
            bridge_remove_client,
            bridge_approve,
            rpc_status
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, _event| {});
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_file(dir: &TempDir, name: &str, content: &str) -> String {
        let path = dir.path().join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn read_text_file_accepts_csv_and_rejects_others() {
        let dir = TempDir::new().unwrap();
        let csv = write_file(&dir, "import.csv", "title,username,password\n");
        assert_eq!(read_text_file(csv).unwrap(), "title,username,password\n");

        let txt = write_file(&dir, "notes.txt", "secret local text");
        let err = read_text_file(txt).unwrap_err();
        assert!(err.contains(".csv"), "unexpected error: {err}");

        let no_ext = write_file(&dir, "config", "{}");
        assert!(read_text_file(no_ext).unwrap_err().contains(".csv"));
    }

    #[test]
    fn read_text_file_rejects_missing_path() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("nope.csv").to_string_lossy().into_owned();
        assert!(read_text_file(missing).unwrap_err().contains("失败"));
    }
}
