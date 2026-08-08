//! Config IPC commands: read/write settings plus bridge/RPC server sync
//! (extracted from commands.rs).

use crate::config;
use crate::config::ConfigStore;
#[cfg(desktop)]
use crate::register_global_hotkey;
use crate::vault;
use tauri::Manager;
// ---------------------------------------------------------------------------
// Config commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub(crate) fn get_config(
    store: tauri::State<'_, ConfigStore>,
) -> Result<config::AppConfig, String> {
    store.get()
}

/// Trim a stored backup template, falling back to the default when empty.
pub(crate) fn normalize_backup_template(template: &str) -> String {
    let trimmed = template.trim();
    if trimmed.is_empty() {
        vault::DEFAULT_BACKUP_TEMPLATE.to_owned()
    } else {
        trimmed.to_owned()
    }
}

#[tauri::command]
pub(crate) fn set_config(
    store: tauri::State<'_, ConfigStore>,
    app: tauri::AppHandle,
    config: config::AppConfig,
) -> Result<config::AppConfig, String> {
    let saved = store.set(config)?;
    #[cfg(desktop)]
    register_global_hotkey(&app, &saved.keyboard.auto_type_global);
    sync_bridge(&app, &saved);
    sync_rpc(&app, &saved);
    // Keep the session's URL-match mode in sync with the config so bridge and
    // RPC matching observe the latest `rpc.matchByRegistrableDomain`.
    if let Some(session) = app.try_state::<std::sync::Mutex<crate::vault::VaultSession>>() {
        if let Ok(mut session) = session.lock() {
            session.match_registrable_domain = saved.rpc.match_by_registrable_domain;
        }
    }
    Ok(saved)
}

/// Start or stop the loopback bridge to match `bridge.enabled`; failures are
/// logged, never fatal (the app stays usable without browser integration).
pub(crate) fn sync_bridge(app: &tauri::AppHandle, config: &config::AppConfig) {
    let state = app.state::<crate::bridge::server::BridgeState>();
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
pub(crate) fn sync_rpc(app: &tauri::AppHandle, config: &config::AppConfig) {
    let state = app.state::<crate::rpc::server::RpcState>();
    if config.rpc.enabled {
        if let Err(e) = state.start(app) {
            eprintln!("rpc: {e}");
        }
    } else {
        state.stop();
    }
}
