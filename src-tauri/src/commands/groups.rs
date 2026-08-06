//! Group CRUD IPC commands (extracted from commands.rs).

use crate::vault::{GroupInput, VaultSession, VaultState};
use std::sync::Mutex;
#[tauri::command]
pub(crate) fn add_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    input: GroupInput,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .add_group(&input)
}

#[tauri::command]
pub(crate) fn rename_group(
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
pub(crate) fn set_group_icon(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    icon: Option<u32>,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .set_group_icon(&uuid, icon)
}

#[tauri::command]
pub(crate) fn set_group_expanded(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
    expanded: bool,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .set_group_expanded(&uuid, expanded)
}

#[tauri::command]
pub(crate) fn delete_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .delete_group(&uuid)
}

#[tauri::command]
pub(crate) fn restore_group(
    session: tauri::State<'_, Mutex<VaultSession>>,
    uuid: String,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .restore_group(&uuid)
}

#[tauri::command]
pub(crate) fn empty_recycle_bin(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<VaultState, String> {
    session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .empty_recycle_bin()
}
