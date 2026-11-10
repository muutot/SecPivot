//! Validated, per-website native policies. Legacy routing does not use these yet.
use crate::util::{url_authority, UrlAuthority};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const MAX_JSON_BYTES: usize = 64 * 1024;
const MAX_RULES: usize = 128;
const MAX_URL_BYTES: usize = 4096;
const INVALID: &str = "网址规则无效；未应用任何规则";
const LIMIT: &str = "网址规则超过大小或数量上限；未应用任何规则";

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

#[cfg(test)]
#[path = "website_rules_tests.rs"]
mod tests;
