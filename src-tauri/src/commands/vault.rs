//! Vault lifecycle + entry/group CRUD IPC commands. Thin wrappers; passwords
//! and keys never cross IPC (extracted from commands.rs).

use crate::config::ConfigStore;
use crate::platform::autotype;
use crate::platform::shield;
use crate::vault;
use crate::vault::{
    EntryAutoTypeInput, EntryInput, EntryPatch, HistoryVersion, TotpCode, VaultOpenResult,
    VaultSession, VaultSessions, VaultState,
};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;
use zeroize::Zeroize;

/// Fallback sequence when an entry resolves no explicit Auto-Type sequence
/// (mirrors `lib.rs::GLOBAL_AUTOTYPE_SEQUENCE`).
const GLOBAL_AUTOTYPE_SEQUENCE: &str = "{USERNAME}{TAB}{PASSWORD}{ENTER}";
// ---------------------------------------------------------------------------

#[tauri::command]
pub(crate) fn open_vault(
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
    mut password: String,
    keyfile: Option<String>,
) -> Result<VaultOpenResult, String> {
    // Slow work (file read, KDF, parse) runs outside the session lock; only
    // the state adoption is locked.
    let prepared = vault::prepare_local_open(
        Path::new(&path),
        &password,
        keyfile.as_deref().map(Path::new),
    );
    let result = match prepared {
        Ok((db, keyfile_bytes)) => {
            let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            vaults.open(&mut active, |fresh| {
                fresh.adopt_local(db, Path::new(&path), &password, keyfile_bytes)
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

/// Apply the screen-capture guard only when the user enabled it in settings.
pub(crate) fn apply_capture_guard(app: &tauri::AppHandle, config: &ConfigStore) {
    let Ok(cfg) = config.get() else { return };
    if cfg.security.screen_capture_guard {
        shield::set_capture_guard(app, true);
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_vault(
    app: tauri::AppHandle,
    config: tauri::State<'_, ConfigStore>,
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
    mut password: String,
    kdf: String,
    cipher: String,
    compression: String,
    keyfile: Option<String>,
) -> Result<VaultOpenResult, String> {
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
            let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            vaults.open(&mut active, |fresh| {
                fresh.adopt_local(db, Path::new(&path), &password, keyfile_bytes)
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

#[tauri::command]
pub(crate) fn close_vault(
    app: tauri::AppHandle,
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<(), String> {
    let keep_rpc = app
        .try_state::<crate::config::ConfigStore>()
        .map(|store| {
            store
                .get()
                .map(|cfg| cfg.rpc.keep_session_after_lock)
                .unwrap_or(true)
        })
        .unwrap_or(true);
    let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    vaults.close(&mut active, session_id.as_deref(), keep_rpc)?;
    // The capture guard stays on while any other session is still open.
    if !vaults.any_open(&active) {
        shield::set_capture_guard(&app, false);
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn get_vault_state(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<Option<VaultState>, String> {
    let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    vaults.state(&mut active, session_id.as_deref())
}

/// Switch the active session to a parked one (multi-database tabs). Returns
/// the newly active state; every subsequent command targets it.
#[tauri::command]
pub(crate) fn set_active_session(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: String,
) -> Result<VaultState, String> {
    let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    vaults.switch_active(&mut active, &session_id)
}

/// Read the open database's storage settings (KDF/cipher/compression/etc).
#[tauri::command]
pub(crate) fn get_database_settings(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<Option<vault::DatabaseSettings>, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .database_settings()
}

/// Apply database-level settings changes (history cap, recycle-bin flag).
#[tauri::command]
pub(crate) fn update_database_settings(
    session: tauri::State<'_, Mutex<VaultSession>>,
    patch: vault::DatabaseSettingsPatch,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .update_database_settings(&patch)
}

#[tauri::command]
pub(crate) fn save_vault(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<VaultState, String> {
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
pub(crate) fn save_vault_as(
    session: tauri::State<'_, Mutex<VaultSession>>,
    path: String,
) -> Result<VaultState, String> {
    // Capture a cheap job (db clone + new path) under the lock, run the
    // re-encrypt (KDF) + serialization + disk write outside it, then switch
    // the session target under the lock again.
    let job = session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .prepare_save_as(Path::new(&path))?;
    let revision = job.revision;
    vault::persist_save(job)?;
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .complete_save_as(PathBuf::from(path), revision)
}

#[tauri::command]
pub(crate) fn change_master_key(
    session: tauri::State<'_, Mutex<VaultSession>>,
    mut password: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    // Keyfile read, KDF and persistence all happen without the session lock.
    let mut keyfile_bytes = vault::read_keyfile(keyfile.as_deref().map(Path::new))?;
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
        // The failure path must not leave the new master password (or keyfile
        // bytes) on the heap; the success path moved them into the session,
        // which zeroizes them on close.
        Err(e) => {
            password.zeroize();
            if let Some(bytes) = keyfile_bytes.as_deref_mut() {
                bytes.zeroize();
            }
            Err(e)
        }
    };
    result
}

#[tauri::command]
pub(crate) fn add_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    input: EntryInput,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .add_entry(&input)
}

/// Bulk-import many entries in a single IPC call (used by the CSV/XML importer).
#[tauri::command]
pub(crate) fn import_entries(
    session: tauri::State<'_, Mutex<VaultSession>>,
    inputs: Vec<EntryInput>,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .add_entries(&inputs)
}

#[tauri::command]
pub(crate) fn update_entry(
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
pub(crate) fn update_entries(
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
pub(crate) fn delete_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .delete_entry(&uuid)
}

#[tauri::command]
pub(crate) fn move_entry(
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
pub(crate) fn delete_entries(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuids: Vec<String>,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .delete_entries(&uuids)
}

#[tauri::command]
pub(crate) fn get_entry_history(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<Vec<HistoryVersion>, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .get_entry_history(&uuid)
}

#[tauri::command]
pub(crate) fn restore_entry_version(
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
pub(crate) fn delete_entry_history(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    index: usize,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .delete_entry_history(&uuid, index)
}

#[tauri::command]
pub(crate) fn restore_entry(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .restore_entry(&uuid)
}

#[tauri::command]
pub(crate) fn save_attachment(
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
pub(crate) fn totp_code(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<TotpCode, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .totp_code(&uuid)
}

#[tauri::command]
pub(crate) fn toggle_favorite(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<vault::MutationDelta, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .toggle_favorite_delta(&uuid)
}

/// Replace an entry's Auto-Type configuration.
#[tauri::command]
pub(crate) fn update_entry_autotype(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    input: EntryAutoTypeInput,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .update_entry_autotype(&uuid, &input)
}

/// Resolve and replay a KeePass-style auto-type sequence for an entry.
/// Executes on a background thread; returns once parsing succeeds.
///
/// The main window is minimized first so keystrokes land in the window the
/// user switches to during the replay delay, never in SecPivot itself.
#[tauri::command]
#[cfg_attr(not(desktop), allow(unused_variables))]
pub(crate) fn auto_type(
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
    #[cfg(desktop)]
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.minimize();
    }
    autotype::run_sequence(&expanded, &ctx).map_err(|e| e.to_string())
}

/// Run auto-type for the entry the user picked from the global-hotkey
/// multi-match dialog. The focused window title was captured when the hotkey
/// fired, so window associations resolve correctly here.
#[tauri::command]
pub(crate) fn autotype_pick(
    app: tauri::AppHandle,
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<(), String> {
    let (expanded, ctx) = {
        let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        let window_title = session
            .take_pending_autotype_window()
            .ok_or_else(|| "没有待处理的自动填充请求".to_owned())?;
        let sequence = match session.resolve_autotype_sequence_for_window(&uuid, &window_title)? {
            Some(sequence) => sequence,
            None => return Err("条目自动填充已禁用".to_owned()),
        };
        let sequence = if sequence.trim().is_empty() {
            GLOBAL_AUTOTYPE_SEQUENCE.to_owned()
        } else {
            sequence
        };
        let ctx = session.autotype_context(&uuid)?;
        let expanded = session.expand_autotype_sequence(&sequence)?;
        (expanded, ctx)
    };
    #[cfg(desktop)]
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.minimize();
    }
    std::thread::spawn(move || {
        if let Err(e) = autotype::run_sequence(&expanded, &ctx) {
            eprintln!("autotype pick: {e}");
        }
    });
    Ok(())
}
