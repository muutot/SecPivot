//! Atomic editor writes always address their originating session explicitly.
use super::with_vault_session;
use crate::vault::{EntrySaveRequest, EntrySaveResult, VaultSession, VaultSessions};
use std::sync::Mutex;

#[tauri::command]
pub(crate) fn save_entry(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: String,
    request: EntrySaveRequest,
) -> Result<EntrySaveResult, String> {
    save_for_session(vaults.inner(), session.inner(), &session_id, request)
}

fn save_for_session(
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    session_id: &str,
    request: EntrySaveRequest,
) -> Result<EntrySaveResult, String> {
    with_vault_session(vaults, session, Some(session_id), |target| {
        target.save_entry(&request)
    })
}

#[cfg(test)]
#[path = "entry_save_tests.rs"]
mod tests;
