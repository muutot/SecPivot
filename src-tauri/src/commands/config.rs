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

/// Where the app keeps its data and whether it runs portably (data beside the
/// executable). Read-only introspection for the About panel.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub portable: bool,
    pub config_path: String,
    pub data_dir: String,
}

#[tauri::command]
pub(crate) fn app_info(store: tauri::State<'_, ConfigStore>) -> AppInfo {
    AppInfo {
        portable: store.is_portable(),
        config_path: store.config_path().to_string_lossy().to_string(),
        data_dir: store.data_dir().to_string_lossy().to_string(),
    }
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
    sync_vault_matching(&app, &saved);
    #[cfg(desktop)]
    sync_bridge(&app, &saved);
    #[cfg(desktop)]
    sync_rpc(&app, &saved);
    Ok(saved)
}

/// Keep URL matching consistent across the active slot, parked tabs and
/// sessions opened after the setting changes. Startup calls this after both
/// managed states are registered so persisted configuration applies before
/// either loopback server begins serving requests.
pub(crate) fn sync_vault_matching(app: &tauri::AppHandle, config: &config::AppConfig) {
    let Some(session) = app.try_state::<std::sync::Mutex<crate::vault::VaultSession>>() else {
        return;
    };
    let Some(vaults) = app.try_state::<crate::vault::VaultSessions>() else {
        return;
    };
    let Ok(mut active) = session.lock() else {
        return;
    };
    let _ =
        vaults.set_match_registrable_domain(&mut active, config.rpc.match_by_registrable_domain);
    let _ = vaults.set_rpc_session_timeout(&mut active, config.rpc.session_timeout_secs);
}

/// Start or stop the loopback bridge to match `bridge.enabled`; failures are
/// logged, never fatal (the app stays usable without browser integration).
#[cfg(desktop)]
pub(crate) fn sync_bridge(app: &tauri::AppHandle, config: &config::AppConfig) {
    let state = app.state::<crate::bridge::server::BridgeState>();
    state.set_generator(config.database.generator.clone());
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
#[cfg(desktop)]
pub(crate) fn sync_rpc(app: &tauri::AppHandle, config: &config::AppConfig) {
    let state = app.state::<crate::rpc::server::RpcState>();
    state.set_generator(config.database.generator.clone());
    if config.rpc.enabled {
        if let Err(e) = state.start(app) {
            eprintln!("rpc: {e}");
        }
    } else {
        state.stop();
    }
}
