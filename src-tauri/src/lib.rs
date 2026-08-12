pub mod bridge;
pub mod commands;
pub mod config;
pub mod crypto;
pub mod platform;
pub mod remote;
pub mod rpc;
pub mod util;
pub mod vault;

use crate::config::ConfigStore;
use crate::vault::{VaultSession, VaultSessions};
use std::path::PathBuf;
use std::sync::Mutex;
#[cfg(desktop)]
use tauri::menu::{Menu, MenuItem};
#[cfg(desktop)]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
#[cfg(desktop)]
use tauri::Emitter;
use tauri::Manager;
#[cfg(desktop)]
use tauri::WindowEvent;
#[cfg(desktop)]
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

/// Default sequence replayed by the global auto-type hotkey when neither the
/// entry nor any ancestor group defines its own AutoType sequence.
#[cfg(desktop)]
const GLOBAL_AUTOTYPE_SEQUENCE: &str = "{USERNAME}{TAB}{PASSWORD}{ENTER}";

/// Tray icon identifiers.
#[cfg(desktop)]
const TRAY_ID: &str = "main";
#[cfg(desktop)]
const TRAY_MENU_SHOW: &str = "tray-show";
#[cfg(desktop)]
const TRAY_MENU_LOCK: &str = "tray-lock";
#[cfg(desktop)]
const TRAY_MENU_QUIT: &str = "tray-quit";
/// Emitted to the frontend when the user picks "锁定" from the tray.
#[cfg(desktop)]
const TRAY_LOCK_EVENT: &str = "tray-lock";

// ---------------------------------------------------------------------------
// Global auto-type hotkey
// ---------------------------------------------------------------------------

/// Register (or replace) the global hotkey from `shortcut`; empty disables it.
/// Failure to register is logged, never fatal: the app stays usable.
#[cfg(desktop)]
pub(crate) fn register_global_hotkey(app: &tauri::AppHandle, shortcut: &str) {
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
#[cfg(desktop)]
fn handle_global_hotkey(app: &tauri::AppHandle) {
    let Some(window_title) = platform::focus::foreground_window_title() else {
        return;
    };
    let Some(session) = app.try_state::<Mutex<VaultSession>>() else {
        return;
    };
    let ctx = {
        let mut session = match session.lock() {
            Ok(s) => s,
            Err(_) => return,
        };
        let candidates = match session.autotype_match_candidates(&window_title) {
            Ok(candidates) => candidates,
            Err(e) => {
                eprintln!("global auto-type: {e}");
                return;
            }
        };
        if candidates.len() > 1 {
            session.set_pending_autotype_window(Some(window_title.clone()));
            let _ = app.emit("autotype-pick-request", &candidates);
            #[cfg(desktop)]
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            return;
        }
        session.set_pending_autotype_window(None);
        let uuid = candidates[0].uuid.clone();
        // Honor window associations first, then the entry's / ancestor
        // group's stored default sequence; `None` means auto-type is
        // disabled for this entry.
        let sequence = match session.resolve_autotype_sequence_for_window(&uuid, &window_title) {
            Ok(seq) => match seq {
                Some(seq) => seq,
                None => {
                    eprintln!("global auto-type: entry auto-type disabled");
                    return;
                }
            },
            Err(e) => {
                eprintln!("global auto-type: {e}");
                return;
            }
        };
        let sequence = if sequence.trim().is_empty() {
            GLOBAL_AUTOTYPE_SEQUENCE.to_owned()
        } else {
            sequence
        };
        let ctx = match session.autotype_context(&uuid) {
            Ok(ctx) => ctx,
            Err(e) => {
                eprintln!("global auto-type: {e}");
                return;
            }
        };
        (sequence, ctx)
    };
    std::thread::spawn(move || {
        if let Err(e) = platform::autotype::run_sequence(&ctx.0, &ctx.1) {
            eprintln!("global auto-type: {e}");
        }
    });
}

// ---------------------------------------------------------------------------
// System tray (KeePassTray-like)
// ---------------------------------------------------------------------------

/// Show (or toggle, when `force_show` is false) the main window.
#[cfg(desktop)]
fn toggle_main_window(app: &tauri::AppHandle, force_show: bool) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let visible = window.is_visible().unwrap_or(false);
    let focused = window.is_focused().unwrap_or(false);
    if force_show || !visible || !focused {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    } else {
        let _ = window.hide();
    }
}

/// Build the tray icon with Show / Lock / Quit actions. Left-clicking the icon
/// toggles the main window; the menu always forces a show.
#[cfg(desktop)]
fn setup_tray(app: &tauri::AppHandle) -> tauri::Result<()> {
    let icon = app
        .default_window_icon()
        .cloned()
        .expect("bundle icon must exist");
    let show = MenuItem::with_id(app, TRAY_MENU_SHOW, "显示主窗口", true, None::<&str>)?;
    let lock = MenuItem::with_id(app, TRAY_MENU_LOCK, "锁定数据库", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, TRAY_MENU_QUIT, "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &lock, &quit])?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .tooltip("SecPivot")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            TRAY_MENU_SHOW => toggle_main_window(app, true),
            TRAY_MENU_LOCK => {
                let _ = app.emit(TRAY_LOCK_EVENT, ());
            }
            TRAY_MENU_QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main_window(tray.app_handle(), false);
            }
        })
        .build(app)?;
    Ok(())
}

/// When `minimizeToTray` is enabled, the window close button hides the window
/// instead of exiting the app; the tray "退出" menu item is the way out.
#[cfg(desktop)]
fn handle_close_requested(window: &tauri::Window, api: &tauri::CloseRequestApi) {
    let app = window.app_handle();
    let minimize_to_tray = app
        .state::<ConfigStore>()
        .get()
        .map(|cfg| cfg.security.minimize_to_tray)
        .unwrap_or(false);
    if minimize_to_tray {
        api.prevent_close();
        let _ = window.hide();
    }
}
// ---------------------------------------------------------------------------
// App entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
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
            #[cfg(desktop)]
            register_global_hotkey(app.handle(), &config.keyboard.auto_type_global);
            #[cfg(desktop)]
            setup_tray(app.handle())?;
            app.manage(store);
            app.manage(Mutex::new(VaultSession::default()));
            app.manage(VaultSessions::default());
            app.manage(commands::TcatoTarget(Mutex::new(None)));
            app.manage(crate::bridge::server::BridgeState::default());
            app.manage(crate::bridge::server::ApprovalBoard::default());
            app.manage(crate::rpc::server::RpcState::default());
            commands::sync_bridge(app.handle(), &config);
            commands::sync_rpc(app.handle(), &config);
            Ok(())
        });

    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_global_shortcut::Builder::new().build());

    #[cfg(desktop)]
    let builder = builder.on_window_event(|window, event| {
        if let WindowEvent::CloseRequested { api, .. } = event {
            // The TCATO overlay can be dismissed directly (Alt+F4); tell
            // the main window so its focus-loss lock re-arms.
            if window.label() == commands::tcato::TCATO_WINDOW_LABEL {
                let _ = window
                    .app_handle()
                    .emit(commands::tcato::TCATO_CLOSE_EVENT, ());
            }
            handle_close_requested(window, api);
        }
    });

    let app = builder
        .invoke_handler(tauri::generate_handler![
            commands::get_config,
            commands::set_config,
            commands::open_vault,
            commands::create_vault,
            commands::close_vault,
            commands::get_vault_state,
            commands::set_active_session,
            commands::list_sessions,
            commands::get_database_settings,
            commands::update_database_settings,
            commands::save_vault,
            commands::save_vault_as,
            commands::change_master_key,
            commands::add_entry,
            commands::import_entries,
            commands::update_entry,
            commands::update_entries,
            commands::delete_entry,
            commands::delete_entries,
            commands::move_entry,
            commands::get_entry_history,
            commands::restore_entry_version,
            commands::delete_entry_history,
            commands::restore_entry,
            commands::delete_group,
            commands::restore_group,
            commands::empty_recycle_bin,
            commands::save_attachment,
            commands::totp_code,
            commands::toggle_favorite,
            commands::update_entry_autotype,
            commands::auto_type,
            commands::autotype_pick,
            commands::open_tcato_overlay,
            commands::tcato_state,
            commands::tcato_send,
            commands::close_tcato_overlay,
            commands::add_group,
            commands::rename_group,
            commands::set_group_icon,
            commands::set_group_expanded,
            commands::set_groups_expanded,
            commands::update_group_autotype,
            commands::update_db_meta,
            commands::get_entry_password,
            commands::get_entry_totp,
            commands::get_custom_field_value,
            commands::get_entry_storage,
            commands::security_report,
            commands::export_csv,
            commands::download_favicons,
            commands::read_text_file,
            commands::clipboard_read_text,
            commands::clipboard_clear,
            commands::remember_credential,
            commands::get_saved_credential,
            commands::clear_saved_credential,
            commands::s3_list_objects,
            commands::open_remote_vault,
            commands::create_remote_vault,
            commands::bridge_status,
            commands::bridge_clients,
            commands::bridge_remove_client,
            commands::bridge_approve,
            commands::rpc_status
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        // The scheduled clipboard wipe lives in the renderer (a JS timer that
        // dies with the process); quitting via the tray must clear any copied
        // password now, or it would stay on the system clipboard forever.
        if let tauri::RunEvent::Exit = event {
            let _ = crate::platform::clipboard::clear_clipboard();
        }
    });
}
