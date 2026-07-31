pub mod config;
pub mod vault;

use crate::config::ConfigStore;
use crate::vault::{EntryInput, GroupInput, TotpCode, VaultSession, VaultState};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

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
    config: config::AppConfig,
) -> Result<config::AppConfig, String> {
    store.set(config)
}

// ---------------------------------------------------------------------------
// Vault commands
// ---------------------------------------------------------------------------

#[tauri::command]
fn open_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
    password: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .open(Path::new(&path), &password)
}

#[tauri::command]
fn create_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
    password: String,
    kdf: String,
    cipher: String,
    compression: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .create(Path::new(&path), &password, &kdf, &cipher, &compression)
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

// ---------------------------------------------------------------------------
// App entry point
// ---------------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            app.manage(store);
            app.manage(Mutex::new(VaultSession::default()));
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
            add_entry,
            update_entry,
            delete_entry,
            totp_code,
            add_group,
            rename_group,
            delete_group
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, _event| {});
}
