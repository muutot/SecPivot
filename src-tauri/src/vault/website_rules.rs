//! Validated, managed per-website policies for native browser URL routing.
use super::{parse_entry_id, VaultSession, VaultState, FIELD_WEBSITE_RULES};
use crate::util::{url_authority, UrlAuthority};
use keepass::db::Value;
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const MAX_JSON_BYTES: usize = 64 * 1024;
const MAX_RULES: usize = 128;
const MAX_URL_BYTES: usize = 4096;
const INVALID: &str = "网址规则无效；未应用任何规则";
const LIMIT: &str = "网址规则超过大小或数量上限；未应用任何规则";

/// Creation/import may carry our own serialized policy through custom fields.
/// Validate before insertion; generic edits continue to preserve the managed field.
pub(super) fn input_rules(
    fields: &[super::CustomField],
) -> Result<Option<ValidatedWebsiteRules>, String> {
    let mut found = None;
    for field in fields {
        if field.name.trim() == FIELD_WEBSITE_RULES {
            if found.is_some() {
                return Err(INVALID.into());
            }
            found = Some(ValidatedWebsiteRules::from_json(&field.value)?);
        }
    }
    Ok(found)
}

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebsiteMode {
    /// Same PSL registrable site. No vendor-related-site aliases are inferred.
    Site,
    /// Same canonical host and explicitly written port, like native Hostname.
    Host,
    /// Identical complete URL bytes after both destinations validate.
    Exact,
    /// Ignore this rule; it does not veto another rule for the same destination.
    Never,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebsiteRule {
    pub url: String,
    pub mode: WebsiteMode,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebsiteRuleSet {
    pub version: u8,
    pub rules: Vec<WebsiteRule>,
}

/// Construction validates every rule before any caller can match a request.
/// No mutable configuration access or fallback to partially valid rules.
pub struct ValidatedWebsiteRules {
    config: WebsiteRuleSet,
    authorities: Vec<UrlAuthority>,
    json: Zeroizing<String>,
}

fn destination(value: &str) -> Option<UrlAuthority> {
    if value.is_empty()
        || value.len() > MAX_URL_BYTES
        || value
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace() || ch == '\\')
        || !value.contains("://")
    {
        return None;
    }
    let parsed = url::Url::parse(value).ok()?;
    if !matches!(parsed.scheme(), "http" | "https")
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return None;
    }
    url_authority(value)
}

impl ValidatedWebsiteRules {
    pub fn from_json(json: &str) -> Result<Self, String> {
        if json.len() > MAX_JSON_BYTES {
            return Err(LIMIT.into());
        }
        Self::new(serde_json::from_str(json).map_err(|_| INVALID)?)
    }

    pub fn new(config: WebsiteRuleSet) -> Result<Self, String> {
        if config.version != 1 {
            return Err(INVALID.into());
        }
        if config.rules.len() > MAX_RULES {
            return Err(LIMIT.into());
        }
        let mut total = 0usize;
        let mut authorities = Vec::with_capacity(config.rules.len());
        for rule in &config.rules {
            if rule.url.len() > MAX_URL_BYTES {
                return Err(LIMIT.into());
            }
            total = total
                .checked_add(rule.url.len())
                .filter(|bytes| *bytes <= MAX_JSON_BYTES)
                .ok_or(LIMIT)?;
            authorities.push(destination(&rule.url).ok_or(INVALID)?);
        }
        let json = Zeroizing::new(serde_json::to_string(&config).map_err(|_| INVALID)?);
        if json.len() > MAX_JSON_BYTES {
            return Err(LIMIT.into());
        }
        Ok(Self {
            config,
            authorities,
            json,
        })
    }

    pub fn config(&self) -> &WebsiteRuleSet {
        &self.config
    }

    pub fn as_json(&self) -> &str {
        &self.json
    }

    pub fn matches(&self, request_url: &str) -> bool {
        let Some(request) = destination(request_url) else {
            return false;
        };
        let request_site = super::helpers::registrable_domain(&request.host);
        self.config
            .rules
            .iter()
            .zip(&self.authorities)
            .any(|(rule, authority)| match rule.mode {
                WebsiteMode::Site => {
                    super::helpers::registrable_domain(&authority.host) == request_site
                }
                WebsiteMode::Host => authority == &request,
                WebsiteMode::Exact => rule.url == request_url,
                WebsiteMode::Never => false,
            })
    }
}

impl VaultSession {
    /// None denotes legacy matching; an empty rule set explicitly matches nothing.
    pub fn get_entry_website_rules(&self, uuid: &str) -> Result<Option<WebsiteRuleSet>, String> {
        let db = self.require_db()?;
        let id = parse_entry_id(uuid)?;
        let entry = db.entry(id).ok_or("条目不存在")?;
        entry
            .get(FIELD_WEBSITE_RULES)
            .map(|raw| ValidatedWebsiteRules::from_json(raw).map(|rules| rules.config().clone()))
            .transpose()
    }

    /// Validate first, track history, then publish once. Snapshot failure rolls back
    /// the database and all publication metadata. Clearing explicitly restores legacy.
    pub fn set_entry_website_rules(
        &mut self,
        uuid: &str,
        config: Option<WebsiteRuleSet>,
    ) -> Result<VaultState, String> {
        self.ensure_writable()?;
        let id = parse_entry_id(uuid)?;
        let rules = config.map(ValidatedWebsiteRules::new).transpose()?;
        let db = self.require_db()?;
        let entry = db.entry(id).ok_or("条目不存在")?;
        if rules.is_some()
            && entry
                .override_url
                .as_deref()
                .is_some_and(|u| !u.trim().is_empty())
        {
            return Err("逐网址规则与覆盖网址不能同时启用".into());
        }
        let unchanged = match (entry.fields.get(FIELD_WEBSITE_RULES), &rules) {
            (None, None) => true,
            (Some(value), Some(rules)) => value.is_protected() && value.get() == rules.as_json(),
            _ => false,
        };
        if unchanged {
            return self.snapshot_without_icons();
        }
        let (cap, max_size) = super::entries::history_limits(&db.meta);
        let mut candidate = db.clone();
        {
            let mut entry = candidate.entry_mut(id).ok_or("条目不存在")?;
            entry.edit_tracking(|tracked| {
                let mut current = tracked.as_mut();
                match &rules {
                    Some(rules) => {
                        current.set(
                            FIELD_WEBSITE_RULES,
                            Value::protected(rules.as_json().to_owned()),
                        );
                    }
                    None => {
                        current.fields.remove(FIELD_WEBSITE_RULES);
                    }
                }
            });
            super::serialize::trim_entry_history(&mut entry, cap);
            let sizes = super::entries::snapshot_sizes(&entry.as_ref());
            super::serialize::trim_entry_history_by_size(&mut entry, max_size, &sizes);
        }
        let original = self.db.replace(candidate);
        let dirty = self.dirty;
        let revision = self.revision;
        let modified = self.modified_at.clone();
        let cache = self.cached_snapshot.take();
        self.mark_dirty();
        match self.snapshot_without_icons() {
            Ok(state) => Ok(state),
            Err(error) => {
                self.db = original;
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
#[path = "website_rules_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "website_rules_storage_tests.rs"]
mod storage_tests;
