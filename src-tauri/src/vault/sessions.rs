//! Multi-database session registry (tabs backend).
//!
//! The app keeps exactly one `VaultSession` in managed state — the *active*
//! session that every mutation command addresses. Opening another vault parks
//! the previous active session here under a generated `sessionId`, so several
//! databases stay decrypted in memory at once. Lifecycle commands
//! (`open`/`create`/`close`/`get_vault_state`) resolve a session by id
//! (default: active); closing the active session promotes the most recently
//! parked one. Immediate mutation commands target the active session, while
//! long prepare/persist/complete commands retain the originating session id
//! so a tab switch cannot redirect their completion phase.

use super::{SessionInfo, VaultOpenResult, VaultSession, VaultState};
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
    /// Run one operation against a stable session id. Long-running commands
    /// use this for both their prepare and completion phases so switching tabs
    /// while persistence is in flight cannot complete against a different
    /// active database.
    pub(crate) fn with_session_mut<T>(
        &self,
        active: &mut VaultSession,
        session_id: Option<&str>,
        operation: impl FnOnce(&mut VaultSession) -> Result<T, String>,
    ) -> Result<T, String> {
        self.with_resolved_session_mut(active, session_id, operation)
            .map(|(_, result)| result)
    }

    /// Resolve the default active session exactly once and return its stable
    /// id together with the operation result. Prepare phases use this when the
    /// IPC argument is omitted, then pass the returned id to every later
    /// completion/failure phase.
    pub(crate) fn with_resolved_session_mut<T>(
        &self,
        active: &mut VaultSession,
        session_id: Option<&str>,
        operation: impl FnOnce(&mut VaultSession) -> Result<T, String>,
    ) -> Result<(String, T), String> {
        let mut inner = self.inner.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        let target = match session_id {
            Some(id) => id.to_owned(),
            None => inner
                .active_id
                .clone()
                .ok_or_else(|| "没有打开的数据库".to_owned())?,
        };
        let result = if inner.active_id.as_deref() == Some(target.as_str()) {
            operation(active)?
        } else {
            let session = inner
                .parked
                .get_mut(&target)
                .ok_or_else(|| "找不到数据库会话".to_owned())?;
            operation(session)?
        };
        Ok((target, result))
    }

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

    /// Close every open session (active + parked) and wipe secrets. Used by
    /// the lock path so idle/manual lock never leaves other tabs decrypted.
    pub fn close_all(&self, active: &mut VaultSession, keep_rpc: bool) -> Result<(), String> {
        let mut inner = self.inner.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        if keep_rpc {
            active.close_keeping_rpc_session();
        } else {
            active.close();
        }
        for (_, mut session) in inner.parked.drain() {
            if keep_rpc {
                session.close_keeping_rpc_session();
            } else {
                session.close();
            }
        }
        inner.order.clear();
        inner.active_id = None;
        Ok(())
    }

    /// Read the addressed session's state (default: active). The active
    /// session's own id resolves to the active session too — the frontend
    /// always addresses the current tab by its id, so rejecting it would
    /// break every post-mutation refresh (reported as "找不到数据库会话"
    /// after a favicon download).
    pub fn state(
        &self,
        active: &mut VaultSession,
        session_id: Option<&str>,
    ) -> Result<Option<VaultState>, String> {
        match session_id {
            Some(id) => {
                let mut inner = self.inner.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
                if inner.active_id.as_deref() == Some(id) {
                    return active.state();
                }
                let session = inner
                    .parked
                    .get_mut(id)
                    .ok_or_else(|| "找不到数据库会话".to_owned())?;
                session.state()
            }
            None => active.state(),
        }
    }

    /// Switch the active session to a parked one: the current active session
    /// is parked under its existing id and the addressed parked session moves
    /// into the active slot. Returns the newly active state.
    pub fn switch_active(
        &self,
        active: &mut VaultSession,
        session_id: &str,
    ) -> Result<VaultState, String> {
        let mut inner = self.inner.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        let target = inner
            .parked
            .get_mut(session_id)
            .ok_or_else(|| "找不到数据库会话".to_owned())?;
        // Validate before mutating so a failure leaves both sessions intact.
        let state = target
            .state()?
            .ok_or_else(|| "数据库会话未打开".to_owned())?;
        let target = inner.parked.remove(session_id).unwrap();
        if let Some(current_id) = inner.active_id.take() {
            inner
                .parked
                .insert(current_id.clone(), std::mem::take(active));
            inner.order.push(current_id);
        }
        *active = target;
        inner.active_id = Some(session_id.to_owned());
        inner.order.retain(|id| id != session_id);
        Ok(state)
    }

    /// All open sessions for the tab bar: active first, then parked in park
    /// order.
    pub fn list(&self, active: &VaultSession) -> Vec<SessionInfo> {
        let inner = match self.inner.lock() {
            Ok(inner) => inner,
            Err(_) => return Vec::new(),
        };
        let mut sessions = Vec::new();
        if let (Some(id), Some((path, file_name, dirty))) =
            (inner.active_id.as_ref(), active.tab_summary())
        {
            sessions.push(SessionInfo {
                session_id: id.clone(),
                file_name,
                path,
                dirty,
            });
        }
        for id in &inner.order {
            if let Some(session) = inner.parked.get(id) {
                if let Some((path, file_name, dirty)) = session.tab_summary() {
                    sessions.push(SessionInfo {
                        session_id: id.clone(),
                        file_name,
                        path,
                        dirty,
                    });
                }
            }
        }
        sessions
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
    use crate::vault::{persist_save, EntryInput};
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
    fn state_resolves_the_active_sessions_own_id() {
        let dir = TempDir::new().unwrap();
        let registry = VaultSessions::default();
        let mut active = VaultSession::default();

        let first = registry
            .open(&mut active, create_vault(&dir, "a.kdbx"))
            .unwrap();
        // Addressing the active session by its own id must work (the
        // frontend always passes the current tab's id); with several tabs
        // open the active id is not in `parked`.
        let current = registry
            .state(&mut active, Some(&first.session_id))
            .unwrap()
            .unwrap();
        assert_eq!(current.file_name, "a.kdbx");

        let second = registry
            .open(&mut active, create_vault(&dir, "b.kdbx"))
            .unwrap();
        let current = registry
            .state(&mut active, Some(&second.session_id))
            .unwrap()
            .unwrap();
        assert_eq!(current.file_name, "b.kdbx");
        // The now-parked first session still resolves by its id.
        let parked = registry
            .state(&mut active, Some(&first.session_id))
            .unwrap()
            .unwrap();
        assert_eq!(parked.file_name, "a.kdbx");
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

    #[test]
    fn switch_active_swaps_parked_and_active_with_stable_ids() {
        let dir = TempDir::new().unwrap();
        let registry = VaultSessions::default();
        let mut active = VaultSession::default();
        let first = registry
            .open(&mut active, create_vault(&dir, "a.kdbx"))
            .unwrap();
        let second = registry
            .open(&mut active, create_vault(&dir, "b.kdbx"))
            .unwrap();

        // Switch back to the first (parked) session.
        let state = registry
            .switch_active(&mut active, &first.session_id)
            .unwrap();
        assert_eq!(state.file_name, "a.kdbx");
        assert_eq!(
            registry
                .state(&mut active, None)
                .unwrap()
                .unwrap()
                .file_name,
            "a.kdbx"
        );
        // The previously active second session is now parked under its id.
        let parked = registry
            .state(&mut active, Some(&second.session_id))
            .unwrap()
            .unwrap();
        assert_eq!(parked.file_name, "b.kdbx");

        // Switch back; ids stay stable.
        let state = registry
            .switch_active(&mut active, &second.session_id)
            .unwrap();
        assert_eq!(state.file_name, "b.kdbx");
        assert!(registry
            .switch_active(&mut active, &first.session_id)
            .is_ok());
        assert!(registry
            .switch_active(&mut active, &second.session_id)
            .is_ok());

        // Unknown ids and the already-active id are rejected without mutation.
        assert!(registry.switch_active(&mut active, "s999").is_err());
        assert_eq!(
            registry
                .state(&mut active, None)
                .unwrap()
                .unwrap()
                .file_name,
            "b.kdbx"
        );
        assert!(registry
            .switch_active(&mut active, &second.session_id)
            .is_err());
        assert_eq!(
            registry
                .state(&mut active, None)
                .unwrap()
                .unwrap()
                .file_name,
            "b.kdbx"
        );
    }

    #[test]
    fn list_sessions_orders_active_first_and_reports_dirty() {
        let dir = TempDir::new().unwrap();
        let registry = VaultSessions::default();
        let mut active = VaultSession::default();
        let first = registry
            .open(&mut active, create_vault(&dir, "a.kdbx"))
            .unwrap();
        let second = registry
            .open(&mut active, create_vault(&dir, "b.kdbx"))
            .unwrap();

        // Dirty the active (second) session.
        active
            .add_entry(&EntryInput {
                group_uuid: crate::vault::ROOT_GROUP_UUID.to_owned(),
                title: "login".into(),
                username: "u".into(),
                password: "p".into(),
                url: String::new(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: Vec::new(),
                attachments: Vec::new(),
            })
            .unwrap();

        let sessions = registry.list(&active);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].session_id, second.session_id);
        assert_eq!(sessions[0].file_name, "b.kdbx");
        assert!(sessions[0].dirty);
        assert_eq!(sessions[1].session_id, first.session_id);
        assert_eq!(sessions[1].file_name, "a.kdbx");
        assert!(!sessions[1].dirty);

        // Switching keeps the tab order (new active first).
        registry
            .switch_active(&mut active, &first.session_id)
            .unwrap();
        let sessions = registry.list(&active);
        assert_eq!(sessions[0].session_id, first.session_id);
        assert!(!sessions[0].dirty);
        assert_eq!(sessions[1].session_id, second.session_id);
        assert!(sessions[1].dirty);
    }

    #[test]
    fn long_save_completion_stays_bound_to_originating_session() {
        let dir = TempDir::new().unwrap();
        let registry = VaultSessions::default();
        let mut active = VaultSession::default();
        let first = registry
            .open(&mut active, create_vault(&dir, "a.kdbx"))
            .unwrap();
        active
            .add_entry(&EntryInput {
                group_uuid: crate::vault::ROOT_GROUP_UUID.to_owned(),
                title: "a-entry".into(),
                username: "u".into(),
                password: "p".into(),
                url: String::new(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: Vec::new(),
                attachments: Vec::new(),
            })
            .unwrap();
        let second = registry
            .open(&mut active, create_vault(&dir, "b.kdbx"))
            .unwrap();
        active
            .add_entry(&EntryInput {
                group_uuid: crate::vault::ROOT_GROUP_UUID.to_owned(),
                title: "b-entry".into(),
                username: "u".into(),
                password: "p".into(),
                url: String::new(),
                notes: String::new(),
                totp: None,
                expires: None,
                icon: Some(None),
                color: None,
                tags: None,
                custom_fields: Vec::new(),
                attachments: Vec::new(),
            })
            .unwrap();

        registry
            .switch_active(&mut active, &first.session_id)
            .unwrap();
        let (originating_id, job) = registry
            .with_resolved_session_mut(&mut active, None, |target| target.prepare_save(false))
            .unwrap();
        assert_eq!(originating_id, first.session_id);
        let revision = job.revision;

        // Simulate the user switching tabs while disk/network persistence is
        // running outside the active-session lock.
        registry
            .switch_active(&mut active, &second.session_id)
            .unwrap();
        let new_hash = persist_save(job).unwrap();
        registry
            .with_session_mut(&mut active, Some(&originating_id), |target| {
                target.complete_save(revision, new_hash)
            })
            .unwrap();

        let current = registry
            .state(&mut active, Some(&second.session_id))
            .unwrap()
            .unwrap();
        assert!(current.dirty, "the newly active vault must stay dirty");
        let saved = registry
            .state(&mut active, Some(&first.session_id))
            .unwrap()
            .unwrap();
        assert!(
            !saved.dirty,
            "the originating parked vault must be marked saved"
        );
    }

    #[test]
    fn close_all_locks_every_session_and_clears_registry() {
        let dir = TempDir::new().unwrap();
        let registry = VaultSessions::default();
        let mut active = VaultSession::default();
        let first = registry
            .open(&mut active, create_vault(&dir, "a.kdbx"))
            .unwrap();
        let second = registry
            .open(&mut active, create_vault(&dir, "b.kdbx"))
            .unwrap();
        assert!(registry.any_open(&active));

        registry.close_all(&mut active, false).unwrap();
        assert!(!active.is_open());
        assert!(registry
            .state(&mut active, Some(&first.session_id))
            .is_err());
        assert!(registry
            .state(&mut active, Some(&second.session_id))
            .is_err());
        assert!(!registry.any_open(&active));
        assert!(registry.list(&active).is_empty());

        // A fresh open after lock starts from an empty registry.
        let again = registry
            .open(&mut active, create_vault(&dir, "c.kdbx"))
            .unwrap();
        assert_eq!(again.session_id, "s3");
    }
}
