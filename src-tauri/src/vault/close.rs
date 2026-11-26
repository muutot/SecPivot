//! Save/discard/cancel transaction shared by tab close, workspace lock and exit.
//! Dialog state contains only nonsecret identity. No vault lock spans a prompt
//! or persistence; late edits/replacements/opened tabs abort the final close.

use super::{
    persist::{PersistOutcome, SaveJob},
    persist_save, VaultSession, VaultSessions,
};
use std::{
    future::Future,
    sync::{atomic::Ordering, Mutex},
};

pub(crate) const CANCELLED: &str = "VAULT_CLOSE_CANCELLED:";
pub(crate) const BUSY: &str = "VAULT_CLOSE_BUSY:";
pub(crate) const CHANGED: &str = "VAULT_CLOSE_CHANGED:";

pub(crate) enum CloseTarget {
    One(Option<String>),
    All { keep_rpc: bool },
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseDecision {
    Save,
    Discard,
    Cancel,
}

pub(crate) struct ClosePrompt {
    pub file_name: String,
}

struct Ticket {
    id: String,
    lifetime: uuid::Uuid,
    revision: u64,
    dirty: bool,
    file_name: String,
}

impl Ticket {
    fn same_lifetime(&self, session: &VaultSession) -> bool {
        session.is_open() && self.lifetime == session.history_lifetime
    }
    fn unchanged(&self, session: &VaultSession) -> Result<(), String> {
        if self.same_lifetime(session) && self.revision == session.revision {
            Ok(())
        } else {
            Err(CHANGED.into())
        }
    }
}

struct ClosePermit<'a>(&'a VaultSessions);
impl Drop for ClosePermit<'_> {
    fn drop(&mut self) {
        self.0.closing.store(false, Ordering::Release);
    }
}

pub(crate) struct ClosedSessions {
    pub ids: Vec<String>,
    pub any_open: bool,
}

pub(crate) async fn close_with_save<P, PF>(
    vaults: &VaultSessions,
    active: &Mutex<VaultSession>,
    target: CloseTarget,
    auto_save: bool,
    prompt: P,
) -> Result<ClosedSessions, String>
where
    P: FnMut(ClosePrompt) -> PF,
    PF: Future<Output = Result<CloseDecision, String>>,
{
    close_with_writer(
        vaults,
        active,
        target,
        auto_save,
        prompt,
        |job| async move {
            tokio::task::spawn_blocking(move || persist_save(job))
                .await
                .map_err(|error| format!("数据库保存任务异常: {error}"))?
        },
    )
    .await
}

// The injected writer only supplies scheduling/failure controls in tests; the
// ordinary entry point always runs the actual KDBX/local/remote save consumer.
async fn close_with_writer<P, PF, W, WF>(
    vaults: &VaultSessions,
    active: &Mutex<VaultSession>,
    target: CloseTarget,
    auto_save: bool,
    mut prompt: P,
    mut writer: W,
) -> Result<ClosedSessions, String>
where
    P: FnMut(ClosePrompt) -> PF,
    PF: Future<Output = Result<CloseDecision, String>>,
    W: FnMut(SaveJob) -> WF,
    WF: Future<Output = Result<PersistOutcome, String>>,
{
    vaults
        .closing
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| BUSY.to_owned())?;
    let _closing = ClosePermit(vaults);
    let tickets = {
        let mut session = active.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        let ids = match &target {
            CloseTarget::One(id) => vec![vaults.resolve_id(id.as_deref())?],
            CloseTarget::All { .. } => vaults.close_session_ids()?,
        };
        ids.into_iter()
            .map(|id| {
                vaults.with_session_mut(&mut session, Some(&id), |current| {
                    let (_, file_name, dirty) =
                        current.tab_summary().ok_or_else(|| CHANGED.to_owned())?;
                    Ok(Ticket {
                        id: id.clone(),
                        lifetime: current.history_lifetime,
                        revision: current.revision,
                        dirty,
                        file_name,
                    })
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    };
    // Gather every decision first: cancelling a later dialog never partially
    // closes a workspace or starts an earlier requested save.
    let mut decisions = Vec::with_capacity(tickets.len());
    for ticket in &tickets {
        let decision = if !ticket.dirty {
            CloseDecision::Discard
        } else if auto_save {
            CloseDecision::Save
        } else {
            prompt(ClosePrompt {
                file_name: ticket.file_name.clone(),
            })
            .await?
        };
        if decision == CloseDecision::Cancel {
            return Err(CANCELLED.into());
        }
        decisions.push(decision);
    }
    let _persistence = vaults.acquire_persistence_async().await?;
    {
        let mut session = active.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
        if matches!(target, CloseTarget::All { .. })
            && vaults.close_session_ids()?
                != tickets
                    .iter()
                    .map(|ticket| ticket.id.clone())
                    .collect::<Vec<_>>()
        {
            return Err(CHANGED.into());
        }
        for ticket in &tickets {
            vaults.with_session_mut(&mut session, Some(&ticket.id), |current| {
                ticket.unchanged(current)
            })?;
        }
    }
    for (ticket, decision) in tickets.iter().zip(&decisions) {
        let job = {
            let mut session = active.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            vaults.with_session_mut(&mut session, Some(&ticket.id), |current| {
                ticket.unchanged(current)?;
                if *decision == CloseDecision::Save && current.dirty {
                    current.prepare_save(false).map(Some)
                } else {
                    Ok(None)
                }
            })?
        };
        if let Some(job) = job {
            let result = writer(job).await;
            let mut session = active.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
            vaults.with_session_mut(&mut session, Some(&ticket.id), |current| {
                // A refresh can replace this ID's database; never adopt an old
                // hash/status into its new decrypted lifetime.
                if !ticket.same_lifetime(current) {
                    return Err(CHANGED.into());
                }
                match result {
                    Ok(outcome) => {
                        current.complete_save(ticket.revision, outcome)?;
                        Ok(())
                    }
                    Err(error) => {
                        if matches!(
                            super::classify_save_error(&error),
                            super::SaveErrorClass::Persistent
                        ) {
                            current.note_save_failure();
                        }
                        Err(error)
                    }
                }
            })?;
        }
    }
    // Validate the entire set under the active lock before wiping any session.
    // Successful earlier saves remain durable if a later save/validation fails,
    // but all sessions stay open for review and retry.
    let mut session = active.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    if matches!(target, CloseTarget::All { .. })
        && vaults.close_session_ids()?
            != tickets
                .iter()
                .map(|ticket| ticket.id.clone())
                .collect::<Vec<_>>()
    {
        return Err(CHANGED.into());
    }
    for (ticket, decision) in tickets.iter().zip(&decisions) {
        vaults.with_session_mut(&mut session, Some(&ticket.id), |current| {
            ticket.unchanged(current)?;
            if (*decision == CloseDecision::Save || !ticket.dirty) && current.dirty {
                return Err(CHANGED.into());
            }
            Ok(())
        })?;
    }
    match target {
        CloseTarget::One(_) => {
            vaults.close(&mut session, Some(&tickets[0].id))?;
        }
        CloseTarget::All { keep_rpc } => vaults.close_all(&mut session, keep_rpc)?,
    }
    Ok(ClosedSessions {
        ids: tickets.into_iter().map(|ticket| ticket.id).collect(),
        any_open: !vaults.close_session_ids()?.is_empty(),
    })
}

#[cfg(test)]
mod tests;
