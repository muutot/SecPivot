//! S3/WebDAV remote vault commands: list objects, open remote, create + upload
//! (extracted from commands.rs).

use super::config::normalize_backup_template;
use super::vault::apply_capture_guard;
use crate::config::ConfigStore;
use crate::remote::{local_storage_dir, make_storage, RemoteObject};
use crate::vault;
use crate::vault::{RemoteMode, VaultOpenResult, VaultSession, VaultSessions};
use std::path::PathBuf;
use std::sync::Mutex;
use zeroize::Zeroize;
// ---------------------------------------------------------------------------
// Remote vault commands
// ---------------------------------------------------------------------------

/// List remote files under the selected profile's prefix. `profile` is the
/// canonical `<kind>/<name>` path; decrypted credentials are resolved from
/// `ConfigStore` and never sent over IPC.
#[tauri::command]
pub(crate) async fn s3_list_objects(
    profile: String,
    config: tauri::State<'_, ConfigStore>,
) -> Result<Vec<RemoteObject>, String> {
    let cfg = config.remote_settings(&profile)?;
    crate::remote::list_objects_async(cfg).await
}

/// Download a vault from the selected remote transport and open it. `mode` is
/// `"memory"` (upload back
/// only) or `"local"` (also mirror locally under
/// `Storage/remote/<kind>/<sanitized profile name>`).
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn open_remote_vault(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    profile: String,
    key: String,
    password: String,
    keyfile: Option<String>,
    mode: String,
) -> Result<VaultOpenResult, String> {
    let (profile_path, cfg) = config.remote_profile(&profile)?;
    let storage = make_storage(&cfg)?;
    let mode = RemoteMode::parse(&mode)?;
    let local_dir = local_storage_dir(&app, &profile_path)?;
    let backup_count = cfg.backup_count.clamp(0, 10) as usize;
    let backup_template = normalize_backup_template(&cfg.backup_template);
    let keyfile_path = keyfile.map(PathBuf::from);
    let local_dir_for_network = local_dir.clone();
    let storage_for_network = storage.clone();
    let backup_template_for_network = backup_template.clone();
    // Network download, KDF and parse run without the session lock, off the
    // async worker thread: the remote transport blocks on its own runtime, which
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
            &backup_template_for_network,
        );
        (result, password)
    })
    .await
    .map_err(|e| format!("远程打开任务异常: {e}"))?;
    let result = match prepared {
        Ok((db, keyfile_bytes, key)) => {
            let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            vaults.open(&mut active, |fresh| {
                fresh.adopt_remote(
                    db,
                    storage,
                    &key,
                    &password,
                    keyfile_bytes,
                    mode,
                    &local_dir,
                    cfg.backup_count.clamp(0, 10) as usize,
                    &backup_template,
                )
            })
        }
        Err(e) => Err(e),
    };
    password.zeroize();
    if result.is_ok() {
        apply_capture_guard(&app, &config);
    }
    result
}

/// Create an empty vault and upload it to the selected transport immediately.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn create_remote_vault(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    profile: String,
    key: String,
    password: String,
    kdf: String,
    cipher: String,
    compression: String,
    keyfile: Option<String>,
    mode: String,
) -> Result<VaultOpenResult, String> {
    let (profile_path, cfg) = config.remote_profile(&profile)?;
    let storage = make_storage(&cfg)?;
    let mode = RemoteMode::parse(&mode)?;
    let local_dir = local_storage_dir(&app, &profile_path)?;
    let backup_count = cfg.backup_count.clamp(0, 10) as usize;
    let backup_template = normalize_backup_template(&cfg.backup_template);
    let keyfile_path = keyfile.map(PathBuf::from);
    let local_dir_for_network = local_dir.clone();
    let storage_for_network = storage.clone();
    let backup_template_for_network = backup_template.clone();
    // KDF, serialization, upload and local mirror run without the lock, off
    // the async worker thread (the remote transport must not block a runtime
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
            &backup_template_for_network,
        );
        (result, password)
    })
    .await
    .map_err(|e| format!("远程创建任务异常: {e}"))?;
    let result = match prepared {
        Ok((db, keyfile_bytes, key)) => {
            let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            vaults.open(&mut active, |fresh| {
                fresh.adopt_remote(
                    db,
                    storage,
                    &key,
                    &password,
                    keyfile_bytes,
                    mode,
                    &local_dir,
                    cfg.backup_count.clamp(0, 10) as usize,
                    &backup_template,
                )
            })
        }
        Err(e) => Err(e),
    };
    password.zeroize();
    if result.is_ok() {
        apply_capture_guard(&app, &config);
    }
    result
}
