//! Explicit-session access to the managed per-website matching policy.
use super::with_vault_session;
use crate::vault::{HistoryRuleSource, VaultSession, VaultSessions, VaultState, WebsiteRuleSet};
use serde::{Deserialize, Deserializer};
use std::sync::Mutex;

/// A required nullable value. Going through Value's deserialize_any rejects a
/// missing field; Option's deserialize_option would silently treat it as None.
pub(crate) struct WebsiteRulesValue(Option<WebsiteRuleSet>);

impl<'de> Deserialize<'de> for WebsiteRulesValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = serde_json::Value::deserialize(deserializer)?;
        serde_json::from_value(value)
            .map(Self)
            .map_err(|_| serde::de::Error::custom("网址规则格式无效；未应用任何规则"))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebsiteRulesWrite {
    uuid: String,
    rules: WebsiteRulesValue,
}

#[tauri::command]
pub(crate) fn get_entry_website_rules(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: String,
    uuid: String,
) -> Result<Option<WebsiteRuleSet>, String> {
    get_for_session(vaults.inner(), session.inner(), &session_id, &uuid)
}

fn get_for_session(
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    session_id: &str,
    uuid: &str,
) -> Result<Option<WebsiteRuleSet>, String> {
    with_vault_session(vaults, session, Some(session_id), |target| {
        target.get_entry_website_rules(uuid)
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct HistoryWebsiteRulesRead {
    uuid: String,
    index: usize,
    selection_token: String,
    source: HistoryRuleSource,
}

#[tauri::command]
pub(crate) fn get_history_website_rules(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: String,
    request: HistoryWebsiteRulesRead,
) -> Result<Option<WebsiteRuleSet>, String> {
    get_history_for_session(vaults.inner(), session.inner(), &session_id, request)
}

fn get_history_for_session(
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    session_id: &str,
    request: HistoryWebsiteRulesRead,
) -> Result<Option<WebsiteRuleSet>, String> {
    with_vault_session(vaults, session, Some(session_id), |target| {
        target.get_history_website_rules(
            &request.uuid,
            request.index,
            &request.selection_token,
            request.source,
        )
    })
}

/// Null explicitly restores legacy matching; an empty set matches nothing.
#[tauri::command]
pub(crate) fn set_entry_website_rules(
    vaults: tauri::State<'_, VaultSessions>,
    session: tauri::State<'_, Mutex<VaultSession>>,
    session_id: String,
    request: WebsiteRulesWrite,
) -> Result<VaultState, String> {
    set_for_session(vaults.inner(), session.inner(), &session_id, request)
}

fn set_for_session(
    vaults: &VaultSessions,
    session: &Mutex<VaultSession>,
    session_id: &str,
    request: WebsiteRulesWrite,
) -> Result<VaultState, String> {
    with_vault_session(vaults, session, Some(session_id), |target| {
        target.set_entry_website_rules(&request.uuid, request.rules.0)
    })
}

#[cfg(test)]
#[path = "website_rules_tests.rs"]
mod tests;
