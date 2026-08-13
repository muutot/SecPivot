//! Group CRUD IPC commands (extracted from commands.rs).

use super::with_vault_session;
use crate::vault::{
    GroupAutoTypeInput, GroupInput, MutationDelta, VaultSession, VaultSessions, VaultState,
};
use std::sync::Mutex;
#[tauri::command]
pub(crate) fn add_group(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    input: GroupInput,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.add_group(&input),
    )
}

#[tauri::command]
pub(crate) fn rename_group(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    name: String,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.rename_group(&uuid, &name),
    )
}

#[tauri::command]
pub(crate) fn set_group_icon(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    icon: Option<u32>,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.set_group_icon(&uuid, icon),
    )
}

/// Update group metadata: `notes`/`tags` absent = keep, empty string = clear;
/// `enableSearching` absent = keep, present = set.
#[tauri::command]
pub(crate) fn update_group_meta(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    notes: Option<String>,
    tags: Option<String>,
    enable_searching: Option<bool>,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.update_group_meta(&uuid, notes, tags, enable_searching),
    )
}

#[tauri::command]
pub(crate) fn set_group_expanded(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    expanded: bool,
) -> Result<MutationDelta, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.set_groups_expanded_delta(&[uuid], expanded),
    )
}

#[tauri::command]
pub(crate) fn set_groups_expanded(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuids: Vec<String>,
    expanded: bool,
) -> Result<MutationDelta, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.set_groups_expanded_delta(&uuids, expanded),
    )
}

/// Update a group's Auto-Type settings.
#[tauri::command]
pub(crate) fn update_group_autotype(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
    input: GroupAutoTypeInput,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.update_group_autotype(&uuid, &input),
    )
}

#[tauri::command]
pub(crate) fn update_db_meta(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    name: Option<String>,
    description: Option<String>,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.update_db_meta(name, description),
    )
}

#[tauri::command]
pub(crate) fn delete_group(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.delete_group(&uuid),
    )
}

#[tauri::command]
pub(crate) fn restore_group(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
    uuid: String,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.restore_group(&uuid),
    )
}

#[tauri::command]
pub(crate) fn empty_recycle_bin(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: Option<String>,
) -> Result<VaultState, String> {
    with_vault_session(
        vaults.inner(),
        session.inner(),
        session_id.as_deref(),
        |target| target.empty_recycle_bin(),
    )
}
