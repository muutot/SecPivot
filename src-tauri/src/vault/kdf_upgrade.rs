//! Approved weak-KDF upgrades stay in memory until the normal save flow.
use super::{helpers::default_kdf_config, KdfParameters, VaultSession, VaultSessions, VaultState};
use serde::Serialize;
use std::sync::Mutex;

pub(crate) const CHANGED: &str = "KDF_UPGRADE_CHANGED:";

pub(crate) async fn stage_weak_kdf(
    vaults: &VaultSessions,
    active: &Mutex<VaultSession>,
    session_id: &str,
    token: &str,
    expected: &KdfParameters,
) -> Result<VaultState, String> {
    let _persistence = vaults.acquire_persistence_async().await?;
    let mut current = active
        .lock()
        .map_err(|_| "Database lock is damaged".to_owned())?;
    vaults.with_session_mut(&mut current, Some(session_id), |target| {
        target.upgrade_weak_kdf(token, expected)
    })
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KdfUpgradeOffer {
    /// Nonsecret decrypted-database lifetime, renewed even on same-path reopen.
    pub token: String,
    pub file_name: String,
    pub parameters: KdfParameters,
    pub recommended: Option<KdfParameters>,
    pub read_only: bool,
}

impl VaultSession {
    pub fn kdf_upgrade_offer(&self) -> Result<KdfUpgradeOffer, String> {
        let db = self.require_db()?;
        let parameters = KdfParameters::from_config(&db.config.kdf_config)?;
        let recommended = if parameters.is_weak()? {
            // Reuse the shipped algorithm defaults. This preserves the algorithm
            // and gives Argon2 stronger costs than KeePass's warning threshold.
            Some(KdfParameters::from_config(&default_kdf_config(
                parameters.algorithm(),
            )?)?)
        } else {
            None
        };
        let path = self.path.as_deref().ok_or("Database is not open")?;
        Ok(KdfUpgradeOffer {
            token: self.history_lifetime.to_string(),
            file_name: std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
                .to_owned(),
            parameters,
            recommended,
            read_only: self.is_read_only(),
        })
    }

    /// Called under the registry persistence permit: an older save job cannot
    /// complete after this mutation and restore the weak KDF configuration.
    pub fn upgrade_weak_kdf(
        &mut self,
        token: &str,
        expected: &KdfParameters,
    ) -> Result<VaultState, String> {
        if uuid::Uuid::parse_str(token).ok() != Some(self.history_lifetime)
            || self.history_lifetime.is_nil()
        {
            return Err(CHANGED.into());
        }
        self.ensure_writable()?;
        let offer = self.kdf_upgrade_offer()?;
        if offer.parameters != *expected || offer.recommended.is_none() {
            return Err(CHANGED.into());
        }
        let config = default_kdf_config(offer.parameters.algorithm())?;
        let original = std::mem::replace(&mut self.require_db_mut()?.config.kdf_config, config);
        let dirty = self.dirty;
        let revision = self.revision;
        let modified = self.modified_at.clone();
        let cache = self.cached_snapshot.take();
        self.mark_dirty();
        match self.snapshot_without_icons() {
            Ok(state) => Ok(state),
            Err(error) => {
                if let Some(db) = self.db.as_mut() {
                    db.config.kdf_config = original;
                }
                self.dirty = dirty;
                self.revision = revision;
                self.modified_at = modified;
                self.cached_snapshot = cache;
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod tests;
