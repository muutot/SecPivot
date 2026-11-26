//! Read-only master-key expiry assessment. Opening-time UI enforcement is separate.
use super::VaultSession;
use chrono::{Local, NaiveDate, NaiveDateTime, Utc};
use serde::Serialize;

mod rules;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InvalidExpiryRule {
    Recommended,
    Required,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MasterKeyExpiryOffer {
    /// Nonsecret decrypted-database lifetime; renewed on same-path reopen.
    pub token: String,
    pub file_name: String,
    pub changed_at: Option<String>,
    pub recommended: bool,
    pub required: bool,
    pub force_once: bool,
    pub read_only: bool,
    pub invalid_rules: Vec<InvalidExpiryRule>,
}

impl VaultSession {
    pub fn master_key_expiry_offer(
        &self,
        recommended_rule: &str,
        required_rule: &str,
    ) -> Result<MasterKeyExpiryOffer, String> {
        self.master_key_expiry_offer_at(
            recommended_rule,
            required_rule,
            Utc::now().naive_utc(),
            Local::now().date_naive(),
        )
    }

    fn master_key_expiry_offer_at(
        &self,
        recommended_rule: &str,
        required_rule: &str,
        now: NaiveDateTime,
        local_date: NaiveDate,
    ) -> Result<MasterKeyExpiryOffer, String> {
        let meta = &self.require_db()?.meta;
        let mut invalid_rules = Vec::new();
        // Parse even when the date is missing so malformed settings remain
        // visible. Never invent a key age for an imported missing timestamp.
        let rec = rules::expired(meta.master_key_changed, recommended_rule, now, local_date)
            .unwrap_or_else(|()| {
                invalid_rules.push(InvalidExpiryRule::Recommended);
                false
            });
        let force = rules::expired(meta.master_key_changed, required_rule, now, local_date)
            .unwrap_or_else(|()| {
                invalid_rules.push(InvalidExpiryRule::Required);
                false
            });
        let days = meta.master_key_changed.map(|date| (now - date).num_days());
        let day_expired = |threshold: Option<isize>| {
            days.zip(threshold).is_some_and(|(days, threshold)| {
                threshold >= 0 && i128::from(days) >= threshold as i128
            })
        };
        let force_once = meta.master_key_change_force_once == Some(true);
        let required = force_once || force || day_expired(meta.master_key_change_force);
        let recommended = !required && (rec || day_expired(meta.master_key_change_rec));
        let path = self.path.as_deref().ok_or("Database is not open")?;
        Ok(MasterKeyExpiryOffer {
            token: self.history_lifetime.to_string(),
            file_name: std::path::Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(path)
                .to_owned(),
            changed_at: meta
                .master_key_changed
                .map(|date| date.format("%Y-%m-%dT%H:%M:%SZ").to_string()),
            recommended,
            required,
            force_once,
            read_only: self.is_read_only(),
            invalid_rules,
        })
    }
}

#[cfg(test)]
mod tests;
