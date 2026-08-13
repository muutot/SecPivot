//! Tauri IPC command handlers, grouped by domain. Thin wrappers around the
//! backend services; passwords and keys never cross IPC.

use crate::vault::{VaultSession, VaultSessions};
use std::sync::Mutex;

pub(crate) mod bridge;
pub(crate) mod clipboard;
pub(crate) mod config;
pub(crate) mod credential;
pub(crate) mod entries;
pub(crate) mod favicon;
pub(crate) mod groups;
pub(crate) mod remote;
pub(crate) mod tcato;
#[cfg(test)]
mod tests;
pub(crate) mod vault;

pub(crate) use self::bridge::*;
pub(crate) use self::clipboard::*;
pub(crate) use self::config::*;
pub(crate) use self::credential::*;
pub(crate) use self::entries::*;
pub(crate) use self::favicon::*;
pub(crate) use self::groups::*;
pub(crate) use self::remote::*;
pub(crate) use self::tcato::*;
pub(crate) use self::vault::*;

/// Resolve one renderer-originated command against the session id captured by
/// the frontend. Omitting the id remains a compatibility fallback to the
/// backend-active session for non-renderer callers.
pub(crate) fn with_vault_session<T>(
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    session_id: Option<&str>,
    operation: impl FnOnce(&mut VaultSession) -> Result<T, String>,
) -> Result<T, String> {
    let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    vaults.with_session_mut(&mut active, session_id, operation)
}

/// Variant that also returns the stable resolved id. Used when a command
/// creates a deferred capability (for example an extracted attachment) that
/// must stay bound to its originating session.
pub(crate) fn with_resolved_vault_session<T>(
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    session_id: Option<&str>,
    operation: impl FnOnce(&mut VaultSession) -> Result<T, String>,
) -> Result<(String, T), String> {
    let mut active = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    vaults.with_resolved_session_mut(&mut active, session_id, operation)
}
