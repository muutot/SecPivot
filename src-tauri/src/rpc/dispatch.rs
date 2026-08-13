//! JSON-RPC dispatch (KeePassRPC v1 method names) and DTO-to-wire conversion.
//! Extracted from `rpc::mod.rs`.
use serde_json::{json, Value};

use super::dto::{RpcDatabase, RpcError, RpcGroup, RpcGroupRef, RpcHost, RpcLogin, RpcLoginWrite};
use crate::config::PasswordGeneratorSettings;
fn group_summary_dto(g: &RpcGroupRef) -> Value {
    json!({
        "title": g.title,
        "uniqueID": g.uuid,
        "iconImageData": g.icon_image_data,
        "path": g.path,
    })
}

fn entry_summary_dto(e: &RpcLogin) -> Value {
    json!({
        "iconImageData": e.icon_image_data,
        "usernameValue": e.username,
        "usernameName": "KeePass username",
        "title": e.title,
        "uRLs": e.urls,
        "uniqueID": e.uuid,
    })
}

fn group_dto(g: &RpcGroup) -> Value {
    json!({
        "title": g.title,
        "uniqueID": g.uuid,
        "iconImageData": g.icon_image_data,
        "path": g.path,
        "childLightEntries": g.entries.iter().map(entry_summary_dto).collect::<Vec<_>>(),
        "childGroups": g.children.iter().map(group_dto).collect::<Vec<_>>(),
    })
}

fn database_dto(db: &RpcDatabase) -> Value {
    json!({
        "name": db.name,
        "fileName": db.file_name,
        "iconImageData": db.icon_image_data,
        "root": group_dto(&db.root),
        "active": db.active,
    })
}

fn database_summary_dto(db: &RpcDatabase) -> Value {
    let root = &db.root;
    let root_ref = RpcGroupRef {
        uuid: root.uuid.clone(),
        title: root.title.clone(),
        path: String::new(),
        icon_image_data: root.icon_image_data.clone(),
    };
    json!({
        "name": db.name,
        "fileName": db.file_name,
        "iconImageData": db.icon_image_data,
        "root": group_summary_dto(&root_ref),
        "active": db.active,
    })
}

pub(crate) fn entry_dto(e: &RpcLogin, db: &RpcDatabase) -> Value {
    json!({
        "uRLs": e.urls,
        "neverAutoFill": false,
        "alwaysAutoFill": false,
        "neverAutoSubmit": false,
        "alwaysAutoSubmit": false,
        "iconImageData": e.icon_image_data,
        "parent": group_summary_dto(&e.parent_group),
        "matchAccuracy": e.match_accuracy,
        "hTTPRealm": e.http_realm,
        "uniqueID": e.uuid,
        "title": e.title,
        "formFieldList": [
            { "displayName": "KeePass username", "id": "", "name": "KeePass username", "type": "FFTusername", "value": e.username, "page": 0 },
            { "displayName": "KeePass password", "id": "", "name": "KeePass password", "type": "FFTpassword", "value": e.password, "page": 0 },
        ],
        "db": database_summary_dto(db),
    })
}

#[derive(Debug, Clone)]
pub(crate) enum RpcWriteRequest {
    Add {
        login: RpcLoginWrite,
        parent_uuid: String,
    },
    Update {
        login: RpcLoginWrite,
        old_uuid: String,
        url_merge_mode: u8,
    },
}

/// Parse the two mutating v1 methods with the exact same validation used by
/// the generic dispatcher. The socket server uses this to persist writes
/// outside the vault-session lock without creating a second wire contract.
pub(crate) fn parse_write_request(
    method: &str,
    params: Option<&Value>,
) -> Result<Option<RpcWriteRequest>, RpcError> {
    match method {
        "AddLogin" => {
            let params =
                params.ok_or_else(|| RpcError::InvalidMessage("AddLogin 缺少参数".to_owned()))?;
            let login: RpcLoginWrite =
                serde_json::from_value(params.get(0).cloned().unwrap_or(Value::Null))
                    .map_err(|e| RpcError::InvalidMessage(format!("login 参数无效: {e}")))?;
            let parent_uuid = params
                .get(1)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned();
            Ok(Some(RpcWriteRequest::Add { login, parent_uuid }))
        }
        "UpdateLogin" => {
            let params = params
                .ok_or_else(|| RpcError::InvalidMessage("UpdateLogin 缺少参数".to_owned()))?;
            let login: RpcLoginWrite =
                serde_json::from_value(params.get(0).cloned().unwrap_or(Value::Null))
                    .map_err(|e| RpcError::InvalidMessage(format!("login 参数无效: {e}")))?;
            let old_uuid = params
                .get(1)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned();
            if old_uuid.is_empty() {
                return Err(RpcError::InvalidMessage(
                    "oldLoginUUID was not passed to the updateLogin function".to_owned(),
                ));
            }
            if params
                .get(3)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .is_empty()
            {
                return Err(RpcError::InvalidMessage(
                    "dbFileName was not passed to the updateLogin function".to_owned(),
                ));
            }
            let url_merge_mode = params.get(2).and_then(|v| v.as_u64()).unwrap_or(0) as u8;
            Ok(Some(RpcWriteRequest::Update {
                login,
                old_uuid,
                url_merge_mode,
            }))
        }
        _ => Ok(None),
    }
}
// ---------------------------------------------------------------------------
// JSON-RPC dispatch (v1 method names used by Kee 4.0.7)
// ---------------------------------------------------------------------------

/// Handle one decrypted JSON-RPC request body; returns the `result` payload.
pub fn handle_jsonrpc(
    host: &mut dyn RpcHost,
    method: &str,
    params: Option<&Value>,
) -> Result<Value, RpcError> {
    handle_jsonrpc_with_generator(host, method, params, &PasswordGeneratorSettings::default())
}

/// `handle_jsonrpc` with an explicit generator settings snapshot (the loopback
/// server passes the configured rules so `GeneratePassword` matches the app).
pub fn handle_jsonrpc_with_generator(
    host: &mut dyn RpcHost,
    method: &str,
    params: Option<&Value>,
    generator: &PasswordGeneratorSettings,
) -> Result<Value, RpcError> {
    if !host.is_open() {
        return Err(RpcError::Locked);
    }
    match method {
        "GetAllDatabases" => {
            let db = host.database().ok_or(RpcError::Locked)?;
            Ok(json!([database_dto(&db)]))
        }
        "FindLogins" => {
            let db = host.database().ok_or(RpcError::Locked)?;
            let urls: Vec<String> = params
                .and_then(|p| p.get(0))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let uuid = params
                .and_then(|p| p.get(5))
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let free_text = params
                .and_then(|p| p.get(7))
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let username = params
                .and_then(|p| p.get(8))
                .and_then(|v| v.as_str())
                .map(str::to_owned);
            let logins = host.find_logins(
                &urls,
                uuid.as_deref(),
                free_text.as_deref(),
                username.as_deref(),
            );
            let result: Vec<Value> = logins.iter().map(|e| entry_dto(e, &db)).collect();
            Ok(json!(result))
        }
        "GetPasswordProfiles" => Ok(json!(["Default"])),
        "GeneratePassword" => crate::bridge::generate_password_with(generator)
            .map(|password| json!(password))
            .map_err(RpcError::InvalidMessage),
        "AddLogin" => {
            let db = host.database().ok_or(RpcError::Locked)?;
            let Some(RpcWriteRequest::Add { login, parent_uuid }) =
                parse_write_request(method, params)?
            else {
                unreachable!("AddLogin parser returned a different request")
            };
            let entry = host.add_login(&login, &parent_uuid)?;
            Ok(entry_dto(&entry, &db))
        }
        "UpdateLogin" => {
            let db = host.database().ok_or(RpcError::Locked)?;
            let Some(RpcWriteRequest::Update {
                login,
                old_uuid,
                url_merge_mode,
            }) = parse_write_request(method, params)?
            else {
                unreachable!("UpdateLogin parser returned a different request")
            };
            let entry = host.update_login(&login, &old_uuid, url_merge_mode)?;
            Ok(entry_dto(&entry, &db))
        }
        other => Err(RpcError::Unsupported(other.to_owned())),
    }
}
