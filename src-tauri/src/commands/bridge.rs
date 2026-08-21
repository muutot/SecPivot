//! KeePassHttp / KeePassRPC status + browser-association commands
//! (extracted from commands.rs).

use crate::bridge;
use crate::bridge::BridgeHost;
use crate::rpc;
use crate::vault::VaultSession;
use std::sync::Mutex;
// ---------------------------------------------------------------------------

/// Whether the loopback server is currently listening.
#[derive(serde::Serialize)]
pub(crate) struct BridgeStatus {
    running: bool,
    port: u16,
    error: Option<String>,
}
#[tauri::command]
pub(crate) fn bridge_status(
    state: tauri::State<'_, crate::bridge::server::BridgeState>,
) -> Result<BridgeStatus, String> {
    Ok(BridgeStatus {
        running: state.running(),
        port: bridge::BRIDGE_PORT,
        error: state.last_error(),
    })
}

/// Whether the KeePassRPC loopback server is currently listening.
#[derive(serde::Serialize)]
pub(crate) struct RpcStatus {
    running: bool,
    port: u16,
    error: Option<String>,
}

#[tauri::command]
pub(crate) fn rpc_status(
    state: tauri::State<'_, crate::rpc::server::RpcState>,
) -> Result<RpcStatus, String> {
    Ok(RpcStatus {
        running: state.running(),
        port: rpc::RPC_PORT,
        error: state.last_error(),
    })
}

/// Live KeePassRPC browser connections (id/identity/peer only — never keys).
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RpcSessionInfo {
    id: u64,
    username: Option<String>,
    peer: String,
    connected_at_ms: u64,
    authenticated: bool,
}

#[tauri::command]
pub(crate) fn rpc_sessions(
    state: tauri::State<'_, crate::rpc::server::RpcState>,
) -> Vec<RpcSessionInfo> {
    state
        .list_sessions()
        .into_iter()
        .map(|s| RpcSessionInfo {
            id: s.id,
            username: s.username,
            peer: s.peer,
            connected_at_ms: s.connected_at_ms,
            authenticated: s.authenticated,
        })
        .collect()
}

/// Force-close one live KeePassRPC browser connection.
#[tauri::command]
pub(crate) fn rpc_close_session(
    state: tauri::State<'_, crate::rpc::server::RpcState>,
    id: u64,
) -> Result<(), String> {
    if state.close_session(id) {
        Ok(())
    } else {
        Err("会话不存在或已断开".to_owned())
    }
}

/// Authorized browser clients of the open session (id only — never keys).
#[tauri::command]
pub(crate) fn bridge_clients(
    session: tauri::State<'_, Mutex<VaultSession>>,
) -> Result<Vec<String>, String> {
    Ok(session
        .lock()
        .map_err(|_| "数据库锁已损坏".to_owned())?
        .list_clients())
}

/// Deauthorize one browser client; returns the remaining list.
#[tauri::command]
pub(crate) fn bridge_remove_client(
    session: tauri::State<'_, Mutex<VaultSession>>,
    id: String,
) -> Result<Vec<String>, String> {
    let mut session = session.lock().map_err(|_| "数据库锁已损坏".to_owned())?;
    if !session.remove_client(&id) {
        return Err("未找到该客户端".to_owned());
    }
    Ok(session.list_clients())
}

/// Answer a pending browser-association approval from the settings UI.
#[tauri::command]
pub(crate) fn bridge_approve(
    board: tauri::State<'_, crate::bridge::server::ApprovalBoard>,
    token: String,
    allowed: bool,
) -> Result<(), String> {
    if board.decide(&token, allowed) {
        Ok(())
    } else {
        Err("审批已过期或不存在".to_owned())
    }
}
