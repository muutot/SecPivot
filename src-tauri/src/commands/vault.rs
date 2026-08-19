//! Vault lifecycle + entry/group CRUD IPC commands. Thin wrappers; passwords
//! and keys never cross IPC (extracted from commands.rs).

use super::{with_resolved_vault_session, with_vault_session};
use crate::config::ConfigStore;
#[cfg(desktop)]
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
#[cfg(desktop)]
const GLOBAL_AUTOTYPE_SEQUENCE: &str = "{USERNAME}{TAB}{PASSWORD}{ENTER}";
// ---------------------------------------------------------------------------

/// Whether KeePassRPC session keys survive a lock (`rpc.keep_session_after_lock`).
fn keep_rpc_session(app: &tauri::AppHandle) -> bool {
    app.try_state::<crate::config::ConfigStore>()
        .map(|store| {
            store
                .get()
                .map(|cfg| cfg.rpc.keep_session_after_lock)
                .unwrap_or(true)
        })
        .unwrap_or(true)
}

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
    let _persistence = vaults.acquire_persistence();
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
    store: tauri::State<'_, vault::AttachmentTempStore>,
    session_id: Option<String>,
) -> Result<(), String> {
    let (closed_session_id, any_open) = {
        let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        let closed_session_id = vaults.close(&mut active, session_id.as_deref())?;
        (closed_session_id, vaults.any_open(&active))
    };
    // The capture guard stays on while any other session is still open.
    if !any_open {
        shield::set_capture_guard(&app, false);
    }
    // Filesystem cleanup must not run while the vault-session mutex is held.
    store.discard_session(&closed_session_id);
    Ok(())
}

/// Close every open session (active + parked) and zeroize secrets. The lock
/// path (toolbar lock, idle auto-lock, lock-after-action) uses this so locking
/// never leaves other tabs decrypted in memory.
#[tauri::command]
pub(crate) fn close_all_vaults(
    app: tauri::AppHandle,
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    store: tauri::State<'_, vault::AttachmentTempStore>,
) -> Result<(), String> {
    let keep_rpc = keep_rpc_session(&app);
    {
        let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        vaults.close_all(&mut active, keep_rpc)?;
    }
    shield::set_capture_guard(&app, false);
    // Locking wipes extracted temp attachments (external viewers may still
    // hold the files open; failed removals stay registered for later retry).
    store.discard_all();
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

/// Open sessions for the tab bar: active first, then parked in park order.
#[tauri::command]
pub(crate) fn list_sessions(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<Vec<vault::SessionInfo>, String> {
    let active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    Ok(vaults.list(&active))
}

/// Read the open database's storage settings (KDF/cipher/compression/etc).
#[tauri::command]
pub(crate) fn get_database_settings(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<Option<vault::DatabaseSettings>, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.database_settings(),
    )
}

/// Apply database-level settings changes (history cap, recycle-bin flag).
#[tauri::command]
pub(crate) async fn update_database_settings(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    patch: vault::DatabaseSettingsPatch,
) -> Result<VaultState, String> {
    if !patch.changes_storage() {
        return with_vault_session(
            vaults.inner(),
            session.inner(),
            session_id.as_deref(),
            |target| target.update_database_settings(&patch),
        );
    }

    let _persistence = vaults.acquire_persistence_async().await?;
    let (session_id, job) = {
        let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        vaults.with_resolved_session_mut(&mut active, session_id.as_deref(), |target| {
            target.prepare_database_settings_update(&patch)
        })?
    };
    let revision = job.revision;
    let persisted = tauri::async_runtime::spawn_blocking(move || vault::persist_save_with_db(job))
        .await
        .map_err(|e| format!("数据库设置保存任务异常: {e}"))?;
    let (db, new_hash) = match persisted {
        Ok(result) => result,
        Err(e) => {
            if !e.starts_with(vault::REMOTE_CHANGED_MARKER) {
                if let Ok(mut active) = session.lock() {
                    let _ = vaults.with_session_mut(&mut active, Some(&session_id), |target| {
                        target.note_save_failure();
                        Ok(())
                    });
                }
            }
            return Err(e);
        }
    };
    let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    vaults.with_session_mut(&mut active, Some(&session_id), |target| {
        target.complete_database_settings_update(&patch, revision, db, new_hash)
    })
}

#[tauri::command]
pub(crate) fn save_vault(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    force: bool,
) -> Result<VaultState, String> {
    // Capture a cheap job under the lock, then run KDF + serialization +
    // transport outside it, then mark clean under the lock again.
    let _persistence = vaults.acquire_persistence();
    let (session_id, job) = {
        let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        vaults.with_resolved_session_mut(&mut active, session_id.as_deref(), |target| {
            target.prepare_save(force)
        })?
    };
    let revision = job.revision;
    match vault::persist_save(job) {
        Ok(new_hash) => {
            let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            vaults.with_session_mut(&mut active, Some(&session_id), |target| {
                target.complete_save(revision, new_hash)
            })
        }
        Err(e) => {
            if !e.starts_with(vault::REMOTE_CHANGED_MARKER) {
                if let Ok(mut active) = session.lock() {
                    let _ = vaults.with_session_mut(&mut active, Some(&session_id), |target| {
                        target.note_save_failure();
                        Ok(())
                    });
                }
            }
            Err(e)
        }
    }
}

/// Download the remote vault's latest bytes and replace the in-memory
/// session (discards local unsaved edits). Only for remote sessions.
#[tauri::command]
pub(crate) async fn refresh_remote_vault(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<VaultState, String> {
    let _persistence = vaults.acquire_persistence_async().await?;
    let (session_id, job) = {
        let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        vaults.with_resolved_session_mut(&mut active, session_id.as_deref(), |target| {
            target.prepare_remote_refresh()
        })?
    };
    let revision = job.revision;
    let result = tauri::async_runtime::spawn_blocking(move || vault::persist_remote_refresh(job))
        .await
        .map_err(|e| format!("远程刷新任务异常: {e}"))??;
    let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    vaults.with_session_mut(&mut active, Some(&session_id), |target| {
        target.complete_remote_refresh(revision, result)
    })
}

/// Merge the remote vault's latest bytes into the session by entry/group
/// UUID + last-modified (histories preserved, recycle bin excluded), persist
/// the merged database back and adopt it. Only for remote sessions.
#[tauri::command]
pub(crate) async fn merge_remote_vault(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<VaultState, String> {
    let _persistence = vaults.acquire_persistence_async().await?;
    let (session_id, job) = {
        let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        vaults.with_resolved_session_mut(&mut active, session_id.as_deref(), |target| {
            target.prepare_remote_merge()
        })?
    };
    let revision = job.revision;
    let persisted = tauri::async_runtime::spawn_blocking(move || vault::persist_remote_merge(job))
        .await
        .map_err(|e| format!("远程合并任务异常: {e}"))?;
    let result = match persisted {
        Ok(result) => result,
        Err(e) => {
            if e.persist_failure {
                if let Ok(mut active) = session.lock() {
                    let _ = vaults.with_session_mut(&mut active, Some(&session_id), |target| {
                        target.note_save_failure();
                        Ok(())
                    });
                }
            }
            return Err(e.message);
        }
    };
    let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    vaults.with_session_mut(&mut active, Some(&session_id), |target| {
        target.complete_remote_merge(revision, result)
    })
}

#[tauri::command]
pub(crate) fn save_vault_as(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    path: String,
) -> Result<VaultState, String> {
    // Capture a cheap job (db clone + new path) under the lock, run the
    // re-encrypt (KDF) + serialization + disk write outside it, then switch
    // the session target under the lock again.
    let _persistence = vaults.acquire_persistence();
    let (session_id, job) = {
        let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        vaults.with_resolved_session_mut(&mut active, session_id.as_deref(), |target| {
            target.prepare_save_as(Path::new(&path))
        })?
    };
    let revision = job.revision;
    let _ = vault::persist_save(job).inspect_err(|e| {
        if !e.starts_with(vault::REMOTE_CHANGED_MARKER) {
            if let Ok(mut active) = session.lock() {
                let _ = vaults.with_session_mut(&mut active, Some(&session_id), |target| {
                    target.note_save_failure();
                    Ok(())
                });
            }
        }
    })?;
    let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    vaults.with_session_mut(&mut active, Some(&session_id), |target| {
        target.complete_save_as(PathBuf::from(path), revision)
    })
}

#[tauri::command]
pub(crate) fn change_master_key(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    mut password: String,
    keyfile: Option<String>,
) -> Result<VaultState, String> {
    // Keyfile read, KDF and persistence all happen without the session lock.
    let mut keyfile_bytes = vault::read_keyfile(keyfile.as_deref().map(Path::new))?;
    let _persistence = vaults.acquire_persistence();
    let (session_id, (db, target, revision)) = {
        let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        vaults.with_resolved_session_mut(&mut active, session_id.as_deref(), |target| {
            target.prepare_change()
        })?
    };
    let persisted = vault::persist_change(&db, &password, keyfile_bytes.as_deref(), &target);
    let result = match persisted {
        Ok(new_hash) => {
            let mut password_slot = Some(password);
            let mut keyfile_slot = Some(keyfile_bytes);
            let result = match session.lock() {
                Ok(mut active) => {
                    vaults.with_session_mut(&mut active, Some(&session_id), |target| {
                        target.complete_change(
                            password_slot.take().expect("password is available"),
                            keyfile_slot.take().expect("keyfile is available"),
                            revision,
                            new_hash,
                        )
                    })
                }
                Err(_) => Err("数据库锁已损坏".to_owned()),
            };
            if let Some(mut remaining) = password_slot {
                remaining.zeroize();
            }
            if let Some(Some(mut remaining)) = keyfile_slot {
                remaining.zeroize();
            }
            result
        }
        // The failure path must not leave the new master password (or keyfile
        // bytes) on the heap; the success path moved them into the session,
        // which zeroizes them on close.
        Err(e) => {
            if !e.starts_with(vault::REMOTE_CHANGED_MARKER) {
                if let Ok(mut active) = session.lock() {
                    let _ = vaults.with_session_mut(&mut active, Some(&session_id), |target| {
                        target.note_save_failure();
                        Ok(())
                    });
                }
            }
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
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    input: EntryInput,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.add_entry(&input),
    )
}

/// Bulk-import many entries in a single IPC call (used by the CSV/XML importer).
#[tauri::command]
pub(crate) fn import_entries(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    inputs: Vec<EntryInput>,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.add_entries(&inputs),
    )
}

#[tauri::command]
pub(crate) fn update_entry(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    input: EntryInput,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.update_entry(&uuid, &input),
    )
}

/// Update matching/quality flags without rewriting stored fields:
/// `overrideUrl` absent = keep, empty string = clear, non-empty = set;
/// `qualityCheck` absent = keep, present = set.
#[tauri::command]
pub(crate) fn update_entry_flags(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    override_url: Option<String>,
    quality_check: Option<bool>,
    foreground_color: Option<String>,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.update_entry_flags(&uuid, override_url, quality_check, foreground_color),
    )
}

#[tauri::command]
pub(crate) fn update_entries(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuids: Vec<String>,
    patch: EntryPatch,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.update_entries(&uuids, &patch),
    )
}

#[tauri::command]
pub(crate) fn update_custom_field(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    name: String,
    value: String,
    protected: bool,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.update_custom_field(&uuid, &name, &value, protected),
    )
}

#[tauri::command]
pub(crate) fn delete_entry(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.delete_entry(&uuid),
    )
}

#[tauri::command]
pub(crate) fn move_entry(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    group_uuid: String,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.move_entry(&uuid, &group_uuid),
    )
}

#[tauri::command]
pub(crate) fn delete_entries(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuids: Vec<String>,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.delete_entries(&uuids),
    )
}

#[tauri::command]
pub(crate) fn get_entry_history(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<Vec<HistoryVersion>, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.get_entry_history(&uuid),
    )
}

#[tauri::command]
pub(crate) fn restore_entry_version(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    index: usize,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.restore_entry_version(&uuid, index),
    )
}

#[tauri::command]
pub(crate) fn delete_entry_history(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    index: usize,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.delete_entry_history(&uuid, index),
    )
}

#[tauri::command]
pub(crate) fn restore_entry(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.restore_entry(&uuid),
    )
}

#[tauri::command]
pub(crate) fn save_attachment(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    name: String,
    dest: String,
) -> Result<(), String> {
    // Extract under the lock, write the file outside it.
    let data = with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.attachment_data(&uuid, &name),
    )?;
    vault::write_attachment_file(&data, &dest)
}

/// In-memory attachment preview (text/image data URL/binary marker); never
/// writes the attachment to disk.
#[tauri::command]
pub(crate) fn preview_attachment(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    name: String,
) -> Result<vault::AttachmentPreview, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.attachment_preview(&uuid, &name),
    )
}

/// Extract an attachment into the controlled temp directory for external
/// viewing. The returned token removes the file via `cleanup_attachment_temp`.
#[tauri::command]
pub(crate) fn open_attachment_temp(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    store: tauri::State<'_, vault::AttachmentTempStore>,
    session_id: Option<String>,
    uuid: String,
    name: String,
) -> Result<vault::TempAttachmentRef, String> {
    let (session_id, data) = with_resolved_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.attachment_data(&uuid, &name),
    )?;
    let (token, path) = store.create(&session_id, &name, &data)?;
    if let Err(error) = vaults.resolve_id(Some(&session_id)) {
        let _ = store.discard(&token);
        return Err(error);
    }
    Ok(vault::TempAttachmentRef {
        token,
        path: path.to_string_lossy().into_owned(),
        name,
        session_id,
    })
}

/// Remove an attachment's temporary extraction (idempotent for unknown tokens).
#[tauri::command]
pub(crate) fn cleanup_attachment_temp(
    store: tauri::State<'_, vault::AttachmentTempStore>,
    token: String,
) -> Result<(), String> {
    store.discard(&token)
}

/// Import the external viewer's changes back into the attachment: replaces
/// the entry attachment's bytes with the registered temp file's content, then
/// discards the temp file. Arbitrary paths are never accepted.
#[tauri::command]
pub(crate) fn import_attachment_from_temp(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    store: tauri::State<'_, vault::AttachmentTempStore>,
    session_id: Option<String>,
    uuid: String,
    name: String,
    token: String,
) -> Result<VaultState, String> {
    let resolved_session_id = vaults.resolve_id(session_id.as_deref())?;
    // Validate the token and read up to 64 MiB before taking the vault lock.
    // Failed reads keep the token registered so the user can retry.
    let data = store.read_for_session(&token, &resolved_session_id)?;
    let result = with_vault_session(
        vaults.inner(),
        session.inner(),
        Some(&resolved_session_id),
        |target| target.import_attachment_bytes(&uuid, &name, data),
    )?;
    let _ = store.discard(&token);
    Ok(result)
}

#[tauri::command]
pub(crate) fn totp_code(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<TotpCode, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.totp_code(&uuid),
    )
}

#[tauri::command]
pub(crate) fn toggle_favorite(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<vault::MutationDelta, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.toggle_favorite_delta(&uuid),
    )
}

/// Replace an entry's Auto-Type configuration.
#[tauri::command]
pub(crate) fn update_entry_autotype(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    input: EntryAutoTypeInput,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.update_entry_autotype(&uuid, &input),
    )
}

/// Resolve and replay a KeePass-style auto-type sequence for an entry.
/// Executes on a background thread; returns once parsing succeeds.
///
/// The main window is minimized first so keystrokes land in the window the
/// user switches to during the replay delay, never in SecPivot itself.
#[tauri::command]
#[cfg(desktop)]
pub(crate) fn auto_type(
    app: tauri::AppHandle,
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    sequence: String,
) -> Result<(), String> {
    let (ctx, expanded) = with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| {
            let ctx = target.autotype_context(&uuid)?;
            let expanded = target.expand_autotype_sequence(&sequence)?;
            Ok((ctx, expanded))
        },
    )?;
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.minimize();
    }
    autotype::run_sequence(&expanded, &ctx).map_err(|e| e.to_string())
}

/// Run auto-type for the entry the user picked from the global-hotkey
/// multi-match dialog. The focused window title was captured when the hotkey
/// fired, so window associations resolve correctly here.
#[tauri::command]
#[cfg(desktop)]
pub(crate) fn autotype_pick(
    app: tauri::AppHandle,
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: String,
    uuid: String,
) -> Result<(), String> {
    let (expanded, ctx) = with_vault_session(
        vaults.inner(),
        session.inner(),
        Some(&session_id),
        |target| {
            let window_title = target
                .take_pending_autotype_window()
                .ok_or_else(|| "没有待处理的自动填充请求".to_owned())?;
            let sequence =
                match target.resolve_autotype_sequence_for_window(&uuid, &window_title)? {
                    Some(sequence) => sequence,
                    None => return Err("条目自动填充已禁用".to_owned()),
                };
            let sequence = if sequence.trim().is_empty() {
                GLOBAL_AUTOTYPE_SEQUENCE.to_owned()
            } else {
                sequence
            };
            let ctx = target.autotype_context(&uuid)?;
            let expanded = target.expand_autotype_sequence(&sequence)?;
            Ok((expanded, ctx))
        },
    )?;
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
