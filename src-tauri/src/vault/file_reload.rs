//! Read-only external-change checks and guarded in-memory reload/merge.

use super::{helpers::build_database_key, serialize::now_iso, VaultSession, VaultState};
use crate::remote::RemoteStorage;
use keepass::Database;
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, OnceLock},
};
use zeroize::Zeroizing;

pub(crate) const STALE: &str = "FILE_RELOAD_CHANGED: 数据库会话已变化，请稍后重试";
pub(crate) const INVALID: &str =
    "FILE_RELOAD_INVALID: 外部文件无法使用当前密钥打开，当前会话已保留";
pub(crate) const CONFLICT: &str =
    "FILE_RELOAD_CONFLICT: 外部变化与未保存修改存在冲突，当前会话已保留";

static WORKERS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();
pub(crate) fn try_worker() -> Option<tokio::sync::OwnedSemaphorePermit> {
    WORKERS
        .get_or_init(|| Arc::new(tokio::sync::Semaphore::new(1)))
        .clone()
        .try_acquire_owned()
        .ok()
}

enum Source {
    Local(PathBuf),
    Remote {
        storage: Arc<dyn RemoteStorage>,
        key: String,
    },
}

/// Contains no credential or decrypted database; unchanged reads avoid KDF/clones.
pub(crate) struct FileCheck {
    source: Source,
    hash: [u8; 32],
    revision: u64,
    lifetime: uuid::Uuid,
}

pub(crate) struct ChangedFile {
    bytes: Vec<u8>,
    hash: [u8; 32],
}

pub(crate) struct ReloadJob {
    password: Zeroizing<String>,
    keyfile: Zeroizing<Option<Vec<u8>>>,
    local: Option<Database>,
    changed: ChangedFile,
}

pub(crate) struct ReloadedFile {
    db: Database,
    hash: [u8; 32],
    merged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum ReloadStatus {
    Disabled,
    Busy,
    Unchanged,
    Ready,
    Reloaded,
    Merged,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileReloadResult {
    pub status: ReloadStatus,
    pub state: Option<VaultState>,
    pub token: Option<String>,
}
impl FileReloadResult {
    pub(crate) fn status(status: ReloadStatus) -> Self {
        Self {
            status,
            state: None,
            token: None,
        }
    }
}

/// One candidate per open session. No password or keyfile lives in the offer.
pub(crate) struct PendingReload {
    ticket: FileCheck,
    result: ReloadedFile,
    token: String,
    created: std::time::Instant,
}

pub(crate) fn check_file(ticket: &FileCheck) -> Result<Option<ChangedFile>, String> {
    let bytes = match &ticket.source {
        Source::Local(path) => super::file_io::read(path, "数据库文件"),
        Source::Remote { storage, key } => storage.get(key),
    }
    .map_err(|_| "FILE_RELOAD_UNAVAILABLE: 无法读取外部文件，当前会话已保留".to_owned())?;
    let hash = crate::crypto::sha256_bytes(&bytes);
    Ok((hash != ticket.hash).then_some(ChangedFile { bytes, hash }))
}

pub(crate) fn reload_file(job: ReloadJob) -> Result<ReloadedFile, String> {
    let key = build_database_key(&job.password, job.keyfile.as_deref())
        .map_err(|_| INVALID.to_owned())?;
    let external = Database::parse(&job.changed.bytes, key).map_err(|_| INVALID.to_owned())?;
    let merged = job.local.is_some();
    let db = match job.local {
        Some(local) => {
            super::merge::merge_databases(&local, &external).map_err(|_| CONFLICT.to_owned())?
        }
        None => external,
    };
    Ok(ReloadedFile {
        db,
        hash: job.changed.hash,
        merged,
    })
}

impl VaultSession {
    pub(crate) fn stage_file_reload(
        &mut self,
        ticket: FileCheck,
        result: ReloadedFile,
    ) -> Result<FileReloadResult, String> {
        self.validate_file_check(&ticket)?;
        let token = uuid::Uuid::new_v4().to_string();
        self.pending_file_reload = Some(PendingReload {
            ticket,
            result,
            token: token.clone(),
            created: std::time::Instant::now(),
        });
        Ok(FileReloadResult {
            status: ReloadStatus::Ready,
            state: None,
            token: Some(token),
        })
    }

    pub(crate) fn apply_file_reload(&mut self, token: &str) -> Result<FileReloadResult, String> {
        if self
            .pending_file_reload
            .as_ref()
            .is_none_or(|pending| pending.token != token)
        {
            return Err(STALE.into());
        }
        let pending = self.pending_file_reload.take().unwrap();
        if pending.created.elapsed() > std::time::Duration::from_secs(10) {
            return Err(STALE.into());
        }
        self.complete_file_reload(&pending.ticket, pending.result)
    }
    pub(crate) fn prepare_file_check(&self) -> Result<FileCheck, String> {
        self.require_db()?;
        let (source, hash) = match &self.remote {
            Some(remote) => (
                Source::Remote {
                    storage: remote.storage.clone(),
                    key: remote.key.clone(),
                },
                remote.base_hash,
            ),
            None => (
                Source::Local(PathBuf::from(self.path.as_deref().ok_or(STALE)?)),
                self.local_base_hash.ok_or(STALE)?,
            ),
        };
        Ok(FileCheck {
            source,
            hash,
            revision: self.revision,
            lifetime: self.history_lifetime,
        })
    }

    fn validate_file_check(&self, ticket: &FileCheck) -> Result<(), String> {
        let current = self.prepare_file_check()?;
        if current.revision != ticket.revision
            || current.lifetime != ticket.lifetime
            || current.hash != ticket.hash
        {
            return Err(STALE.into());
        }
        Ok(())
    }

    pub(crate) fn prepare_file_reload(
        &self,
        ticket: &FileCheck,
        changed: ChangedFile,
    ) -> Result<ReloadJob, String> {
        self.validate_file_check(ticket)?;
        Ok(ReloadJob {
            password: Zeroizing::new(self.password.as_deref().ok_or(STALE)?.to_owned()),
            keyfile: Zeroizing::new(self.keyfile.clone()),
            local: self.dirty.then(|| self.db.as_ref().unwrap().clone()),
            changed,
        })
    }

    pub(crate) fn complete_file_reload(
        &mut self,
        ticket: &FileCheck,
        result: ReloadedFile,
    ) -> Result<FileReloadResult, String> {
        self.validate_file_check(ticket)?;
        // Snapshot refusal must restore the exact previous database/publication state.
        let previous_db = self.db.replace(result.db);
        let previous = (
            self.dirty,
            self.revision,
            self.history_lifetime,
            self.modified_at.clone(),
            self.cached_snapshot.take(),
        );
        self.dirty = result.merged;
        self.revision += 1;
        self.history_lifetime = uuid::Uuid::new_v4();
        self.modified_at = now_iso();
        let state = match self.snapshot() {
            Ok(state) => state,
            Err(error) => {
                self.db = previous_db;
                (
                    self.dirty,
                    self.revision,
                    self.history_lifetime,
                    self.modified_at,
                    self.cached_snapshot,
                ) = previous;
                return Err(error);
            }
        };
        if let Some(remote) = self.remote.as_mut() {
            remote.base_hash = result.hash;
        } else {
            self.local_base_hash = Some(result.hash);
        }
        Ok(FileReloadResult {
            status: if result.merged {
                ReloadStatus::Merged
            } else {
                ReloadStatus::Reloaded
            },
            state: Some(state),
            token: None,
        })
    }
}

#[cfg(test)]
mod tests;
