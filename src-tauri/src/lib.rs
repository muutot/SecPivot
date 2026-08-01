pub mod autotype;
pub mod config;
pub mod credential;
pub mod focus;
pub mod remote;
pub mod vault;

use crate::config::ConfigStore;
use crate::remote::{local_storage_dir, RemoteObject, RemoteStorage, S3Storage};
use crate::vault::{
    EntryInput, GroupInput, HistoryVersion, RemoteMode, SecurityReport, TotpCode, VaultSession,
    VaultState,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Sequence replayed by the global auto-type hotkey.
const GLOBAL_AUTOTYPE_SEQUENCE: &str = "{USERNAME}{TAB}{PASSWORD}{ENTER}";

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
    register_global_hotkey(&app, &saved.general.global_auto_type_shortcut);
    Ok(saved)
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
fn remember_credential(path: String, password: String) -> Result<(), String> {
    credential::remember(&path, &password)
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
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
    password: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .open(
            Path::new(&path),
            &password,
            keyfile.as_deref().map(Path::new),
        )
}

#[tauri::command]
fn create_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
    password: String,
    kdf: String,
    cipher: String,
    compression: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .create(
            Path::new(&path),
            &password,
            &kdf,
            &cipher,
            &compression,
            keyfile.as_deref().map(Path::new),
        )
}

#[tauri::command]
fn close_vault(session: tauri::State<'_, Mutex<VaultSession>>) -> Result<(), String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .close();
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
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .save()
}

#[tauri::command]
fn change_master_key(
    session: tauri::State<'_, Mutex<VaultSession>>,
    password: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .change_master_key(&password, keyfile.as_deref().map(Path::new))
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
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .save_attachment(&uuid, &name, &dest)
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
#[tauri::command]
fn auto_type(
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
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .export_csv(&path)
}

/// Read a UTF-8 text file from a user-picked path (CSV import).
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("读取文件失败: {e}"))
}

// ---------------------------------------------------------------------------
// S3 remote commands
// ---------------------------------------------------------------------------

/// List `.kdbx` files under the configured prefix (newest key first).
#[tauri::command]
async fn s3_list_objects(cfg: crate::config::RemoteSettings) -> Result<Vec<RemoteObject>, String> {
    let storage = S3Storage::new(&cfg)?;
    let mut objects = storage.list(&cfg.prefix)?;
    objects.retain(|o| o.key.ends_with(".kdbx"));
    objects.sort_by(|a, b| b.key.cmp(&a.key));
    Ok(objects)
}

/// Download a vault from S3 and open it. `mode` is `"memory"` (upload back
/// only) or `"local"` (also mirror locally under `Storage/remote/<local_dir>`).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn open_remote_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
    app: tauri::AppHandle,
    cfg: crate::config::RemoteSettings,
    key: String,
    password: String,
    keyfile: Option<String>,
    mode: String,
) -> Result<VaultState, String> {
    let storage: Arc<dyn RemoteStorage> = Arc::new(S3Storage::new(&cfg)?);
    let mode = RemoteMode::parse(&mode)?;
    let local_dir = local_storage_dir(&app, &cfg.local_dir)?;
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .open_remote(
            storage,
            &key,
            &password,
            keyfile.as_deref().map(Path::new),
            mode,
            &local_dir,
            cfg.backup_count.clamp(0, 10) as usize,
        )
}

/// Create an empty vault and upload it to S3 immediately.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn create_remote_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
    app: tauri::AppHandle,
    cfg: crate::config::RemoteSettings,
    key: String,
    password: String,
    kdf: String,
    cipher: String,
    compression: String,
    keyfile: Option<String>,
    mode: String,
) -> Result<VaultState, String> {
    let storage: Arc<dyn RemoteStorage> = Arc::new(S3Storage::new(&cfg)?);
    let mode = RemoteMode::parse(&mode)?;
    let local_dir = local_storage_dir(&app, &cfg.local_dir)?;
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .create_remote(
            storage,
            &key,
            &password,
            &kdf,
            &cipher,
            &compression,
            keyfile.as_deref().map(Path::new),
            mode,
            &local_dir,
            cfg.backup_count.clamp(0, 10) as usize,
        )
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
            register_global_hotkey(app.handle(), &config.general.global_auto_type_shortcut);
            app.manage(store);
            app.manage(Mutex::new(VaultSession::default()));
            app.manage(TcatoTarget(Mutex::new(None)));
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
            change_master_key,
            add_entry,
            update_entry,
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
            delete_group,
            get_entry_password,
            security_report,
            export_csv,
            read_text_file,
            remember_credential,
            get_saved_credential,
            clear_saved_credential,
            s3_list_objects,
            open_remote_vault,
            create_remote_vault
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, _event| {});
}
