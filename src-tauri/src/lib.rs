pub mod autotype;
pub mod config;
pub mod credential;
pub mod remote;
pub mod vault;

use crate::autotype::AutotypeContext;
use crate::config::ConfigStore;
use crate::remote::{local_storage_dir, RemoteObject, RemoteStorage, S3Storage};
use crate::vault::{
    EntryInput, GroupInput, RemoteMode, SecurityReport, TotpCode, VaultSession, VaultState,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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
    let ctx: AutotypeContext = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .autotype_context(&uuid)?;
    autotype::run_sequence(&sequence, &ctx).map_err(|e| e.to_string())
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
            save_attachment,
            totp_code,
            toggle_favorite,
            auto_type,
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
