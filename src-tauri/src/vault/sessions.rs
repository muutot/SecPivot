//! Multi-database session registry (tabs backend).
//!
//! The app keeps exactly one `VaultSession` in managed state — the *active*
//! session that every mutation command addresses. Opening another vault parks
//! the previous active session here under a generated `sessionId`, so several
//! databases stay decrypted in memory at once. Lifecycle commands
//! (`open`/`create`/`close`/`get_vault_state`) resolve a session by id
//! (default: active); closing the active session promotes the most recently
//! parked one. Mutation commands keep targeting the active session until the
//! session-addressing sub-item lands.

use super::{VaultOpenResult, VaultSession, VaultState};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Default)]
struct SessionsInner {
    /// Non-active sessions, keyed by generated id. The active session lives in
    /// the app's `Mutex<VaultSession>` and is never duplicated here.
    parked: HashMap<String, VaultSession>,
    /// Parked ids in park order; the last entry is the next promoted session.
    order: Vec<String>,
    /// Id of the session currently held by the active `VaultSession`.
    active_id: Option<String>,
    next_id: u64,
}

impl SessionsInner {
    fn alloc_id(&mut self) -> String {
        self.next_id += 1;
        format!("s{}", self.next_id)
    }
}

/// Managed state backing multi-database tabs. All methods follow the lock
/// order `active mutex -> registry` so callers that hold the active session
/// lock first never deadlock against registry operations.
#[derive(Default)]
pub struct VaultSessions {
    inner: Mutex<SessionsInner>,
}

impl VaultSessions {
    /// Adopt a freshly opened database as the active session. The previously
    /// active session (if any) is parked under a new id and stays decrypted.
    /// `adopt` runs on a fresh session first, so a failed open leaves the
    /// current active session untouched.
    pub fn open(
        &self,
        active: &mut VaultSession,
        adopt: impl FnOnce(&mut VaultSession) -> Result<VaultState, String>,
    ) -> Result<VaultOpenResult, String> {
        let mut fresh = VaultSession::default();
        let state = adopt(&mut fresh)?;
        let mut inner = self.inner.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        // Park the current active session under its existing id so ids stay
        // stable for the frontend across tab switches.
        if let Some(current_id) = inner.active_id.take() {
            inner
                .parked
                .insert(current_id.clone(), std::mem::take(active));
            inner.order.push(current_id);
        }
        let id = inner.alloc_id();
        *active = fresh;
        inner.active_id = Some(id.clone());
        Ok(VaultOpenResult {
            session_id: id,
            state,
        })
    }

    /// Close the addressed session (default: active). Closing the active
    /// session promotes the most recently parked one, so at least one vault
    /// stays open when others remain.
    pub fn close(
        &self,
        active: &mut VaultSession,
        session_id: Option<&str>,
        keep_rpc: bool,
    ) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        let target = match session_id {
            Some(id) => id.to_owned(),
            None => inner
                .active_id
                .clone()
                .ok_or_else(|| "没有打开的数据库".to_owned())?,
        };
        if inner.active_id.as_deref() == Some(target.as_str()) {
            if keep_rpc {
                active.close_keeping_rpc_session();
            } else {
                active.close();
            }
            inner.active_id = None;
            if let Some(promoted) = inner.order.pop() {
                if let Some(session) = inner.parked.remove(&promoted) {
                    *active = session;
                    inner.active_id = Some(promoted);
                }
            }
            Ok(())
        } else {
            let mut removed = inner
                .parked
                .remove(&target)
                .ok_or_else(|| "找不到数据库会话".to_owned())?;
            inner.order.retain(|id| id != &target);
            if keep_rpc {
                removed.close_keeping_rpc_session();
            } else {
                removed.close();
            }
            Ok(())
        }
    }

    /// Read the addressed session's state (default: active).
    pub fn state(
        &self,
        active: &mut VaultSession,
        session_id: Option<&str>,
    ) -> Result<Option<VaultState>, String> {
        match session_id {
            Some(id) => {
                let mut inner = self.inner.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
                let session = inner
                    .parked
                    .get_mut(id)
                    .ok_or_else(|| "找不到数据库会话".to_owned())?;
                session.state()
            }
            None => active.state(),
        }
    }

    /// Whether any session (active or parked) is open.
    pub fn any_open(&self, active: &VaultSession) -> bool {
        let has_parked = self
            .inner
            .lock()
            .map(|inner| !inner.parked.is_empty() || inner.active_id.is_some())
            .unwrap_or(false);
        has_parked || active.is_open()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_vault<'a>(
        dir: &'a TempDir,
        name: &'a str,
    ) -> impl FnOnce(&mut VaultSession) -> Result<VaultState, String> + 'a {
        move |fresh| {
            let path = dir.path().join(name);
            fresh.create(&path, "master", "Aes", "Aes256", "None", None)
        }
    }

    #[test]
    fn opening_second_vault_parks_first_and_close_promotes() {
        let dir = TempDir::new().unwrap();
        let registry = VaultSessions::default();
        let mut active = VaultSession::default();

        let first = registry
            .open(&mut active, create_vault(&dir, "a.kdbx"))
            .unwrap();
        assert_eq!(active.state().unwrap().unwrap().file_name, "a.kdbx");
        assert!(registry.any_open(&active));

        let second = registry
            .open(&mut active, create_vault(&dir, "b.kdbx"))
            .unwrap();
        assert_eq!(active.state().unwrap().unwrap().file_name, "b.kdbx");
        assert_ne!(first.session_id, second.session_id);
        // The parked first session is still readable by id.
        let parked = registry
            .state(&mut active, Some(&first.session_id))
            .unwrap()
            .unwrap();
        assert_eq!(parked.file_name, "a.kdbx");
        // Closing the active session promotes the parked one.
        registry
            .close(&mut active, Some(&second.session_id), true)
            .unwrap();
        let promoted = registry.state(&mut active, None).unwrap().unwrap();
        assert_eq!(promoted.file_name, "a.kdbx");
        assert!(registry.any_open(&active));

        registry.close(&mut active, None, true).unwrap();
        assert!(registry.state(&mut active, None).unwrap().is_none());
        assert!(!registry.any_open(&active));
    }

    #[test]
    fn close_parked_keeps_active_and_unknown_ids_error() {
        let dir = TempDir::new().unwrap();
        let registry = VaultSessions::default();
        let mut active = VaultSession::default();

        let first = registry
            .open(&mut active, create_vault(&dir, "a.kdbx"))
            .unwrap();
        let second = registry
            .open(&mut active, create_vault(&dir, "b.kdbx"))
            .unwrap();
        // Closing the parked first session leaves the active second intact.
        registry
            .close(&mut active, Some(&first.session_id), false)
            .unwrap();
        let current = registry.state(&mut active, None).unwrap().unwrap();
        assert_eq!(current.file_name, "b.kdbx");

        assert!(registry.close(&mut active, Some("s999"), false).is_err());
        assert!(registry.state(&mut active, Some("s999")).is_err());
        // Ids stay stable when a session is parked and promoted.
        assert_eq!(first.session_id, "s1");
        assert_eq!(second.session_id, "s2");
    }

    #[test]
    fn failed_open_does_not_disturb_active_session() {
        let dir = TempDir::new().unwrap();
        let registry = VaultSessions::default();
        let mut active = VaultSession::default();
        registry
            .open(&mut active, create_vault(&dir, "a.kdbx"))
            .unwrap();

        let err = registry
            .open(&mut active, |_fresh| Err("拒绝打开".to_owned()))
            .unwrap_err();
        assert_eq!(err, "拒绝打开");
        let current = registry.state(&mut active, None).unwrap().unwrap();
        assert_eq!(current.file_name, "a.kdbx");
        assert!(registry.any_open(&active));
    }
}
