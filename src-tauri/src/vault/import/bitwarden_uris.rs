//! Native URI mapping retains raw source text and resolves only explicit source defaults.
use crate::vault::{
    bitwarden_uri, BitwardenUriSettings, ValidatedWebsiteRules, WebsiteMode, WebsiteRule,
    WebsiteRuleSet,
};
use serde_json::Value as Json;

pub(super) fn prepare(
    login: Option<&Json>,
    settings: &BitwardenUriSettings,
) -> Result<(String, ValidatedWebsiteRules), String> {
    let invalid = || "Bitwarden 网址格式或匹配方式无效；未导入任何内容".to_owned();
    let uris = match login {
        None | Some(Json::Null) => &[][..],
        Some(login) if login.is_object() => {
            super::optional_array(login.get("uris")).map_err(|_| invalid())?
        }
        _ => return Err(invalid()),
    };
    if uris.len() > 128 {
        return Err("Bitwarden 逐网址规则超过128条上限；未导入任何内容".into());
    }
    let mut primary = None;
    let mut rules = Vec::with_capacity(uris.len());
    for uri in uris {
        if !uri.is_object() {
            return Err(invalid());
        }
        let url = super::optional_string(uri.get("uri")).map_err(|_| invalid())?;
        let match_type = match uri.get("match") {
            None | Some(Json::Null) => settings.default_match,
            Some(value) => value.as_u64().filter(|v| *v <= 5).ok_or_else(invalid)? as u8,
        };
        let mode = bitwarden_uri::mode(match_type)?;
        if primary.is_none() && mode != WebsiteMode::Regex {
            primary = bitwarden_uri::launch_url(url);
        }
        rules.push(WebsiteRule {
            url: url.to_owned(),
            mode,
        });
    }
    let policy = ValidatedWebsiteRules::new(WebsiteRuleSet {
        version: 2,
        rules,
        bitwarden: Some(settings.clone()),
    })?;
    Ok((primary.unwrap_or_default(), policy))
}
