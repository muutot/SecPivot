//! Pure KeePassRPC protocol state machines (no sockets): setup + JSON-RPC.
//! Extracted from `rpc::server.rs`.
use serde_json::{json, Value};
use std::time::Instant;
use zeroize::Zeroize;

use super::{Conn, SIDE_CHANNEL_BYTES, SIDE_CHANNEL_TTL};
use crate::config::PasswordGeneratorSettings;
use crate::rpc::{
    decrypt_frame, encrypt_frame, handle_jsonrpc_with_generator, hex, key_auth_cr, key_auth_sr,
    random_hex, secret_bytes, Envelope, ErrorMessage, KeyMessage, RpcError, SrpMessage, SrpServer,
    FEATURES, SECURITY_LEVEL,
};
/// Pure setup-protocol state machine; returns the envelope to send (or an
/// error envelope). Tested directly with a stub host.
pub(crate) fn dispatch_setup(
    conn: &mut Conn,
    env: &Envelope,
    host: &mut dyn crate::rpc::RpcHost,
    side_channel: &mut dyn FnMut(&str, u64),
) -> Option<Envelope> {
    let error = |code: &str| {
        let mut reply = Envelope::setup();
        reply.error = Some(ErrorMessage {
            code: code.to_owned(),
            message_params: vec![],
        });
        reply
    };

    if let Some(srp) = &env.srp {
        match srp.stage.as_deref() {
            Some("identifyToServer") => {
                let i = srp.i.clone().filter(|s| !s.is_empty())?;
                let a = srp.a.clone().filter(|s| !s.is_empty())?;
                if !host.is_open() {
                    eprintln!("[rpc] identifyToServer refused: vault locked");
                    return Some(error("AUTH_FAILED"));
                }
                eprintln!("[rpc] identifyToServer accepted");
                let mut password = random_hex(SIDE_CHANNEL_BYTES);
                let expires = SIDE_CHANNEL_TTL.as_secs();
                side_channel(&password, expires);
                let (server, payload) = SrpServer::begin(&password);
                password.zeroize();
                let srp_msg: SrpMessage = serde_json::from_value(payload).unwrap_or_default();
                conn.username = Some(i);
                conn.srp_a = Some(a);
                conn.srp = Some(server);
                conn.srp_expiry = Some(Instant::now() + SIDE_CHANNEL_TTL);
                let mut reply = Envelope::setup();
                reply.srp = Some(srp_msg);
                reply.features = Some(features());
                Some(reply)
            }
            Some("proofToServer") => {
                let m = srp.m.clone()?;
                let mut server = conn.srp.take()?;
                let expiry = conn.srp_expiry.take()?;
                let a = conn.srp_a.take()?;
                eprintln!(
                    "[rpc] proofToServer received (a_len={}, m_len={}, a[..6]={})",
                    a.len(),
                    m.len(),
                    &a[..6.min(a.len())]
                );
                if Instant::now() > expiry {
                    eprintln!("[rpc] proof expired");
                    return Some(error("AUTH_EXPIRED"));
                }
                let m2 = match server.verify_proof(&a, &m) {
                    Ok(m2) => m2,
                    Err(RpcError::AuthFailed) => return Some(error("AUTH_FAILED")),
                    Err(_) => return None,
                };
                let secret = server
                    .secret_key()
                    .ok()
                    .and_then(|k| secret_bytes(&k).ok())?;
                let username = conn.username.clone().unwrap_or_default();
                host.register_rpc_key(&username, secret.clone());
                conn.session_key = Some(secret);
                let mut reply = Envelope::setup();
                reply.srp = Some(SrpMessage {
                    stage: Some("proofToClient".to_owned()),
                    m2: Some(m2),
                    // Kee checks `srp.securityLevel` on every setup message;
                    // missing here it rejects the server as "security level
                    // too low" (AUTH_SERVER_SECURITY_LEVEL_TOO_LOW).
                    security_level: Some(SECURITY_LEVEL),
                    ..Default::default()
                });
                Some(reply)
            }
            _ => None,
        }
    } else if let Some(key) = &env.key {
        if key.sc.is_none() && key.cc.is_some() && key.cr.is_some() {
            // Stage 2: challenge-response; `cr` must hash to the expected value.
            let mut secret = conn.key_secret.take()?;
            let sc = conn.key_sc.take()?;
            let cc = key.cc.clone().unwrap_or_default();
            let cr = key.cr.clone().unwrap_or_default();
            let secret_hex = hex(&secret);
            if cr != key_auth_cr(&secret_hex, &sc, &cc) {
                secret.zeroize();
                return Some(error("AUTH_FAILED"));
            }
            let sr = key_auth_sr(&secret_hex, &sc, &cc);
            conn.session_key = Some(secret);
            let mut reply = Envelope::setup();
            reply.key = Some(KeyMessage {
                sr: Some(sr),
                security_level: Some(SECURITY_LEVEL),
                ..Default::default()
            });
            Some(reply)
        } else {
            // Stage 1: fresh challenge from the session-held key.
            let username = key.username.clone().filter(|s| !s.is_empty())?;
            let Some(secret) = host.rpc_key(&username) else {
                return Some(error("AUTH_RESTART"));
            };
            if secret.len() != 32 {
                return Some(error("AUTH_RESTART"));
            }
            let sc = random_hex(32);
            conn.key_secret = Some(secret);
            conn.key_sc = Some(sc.clone());
            let mut reply = Envelope::setup();
            reply.key = Some(KeyMessage {
                sc: Some(sc),
                security_level: Some(SECURITY_LEVEL),
                ..Default::default()
            });
            reply.features = Some(features());
            Some(reply)
        }
    } else {
        None
    }
}
/// Pure JSON-RPC dispatch: decrypt → `handle_jsonrpc` → encrypt. Returns the
/// encrypted response envelope plus the method name (or `None` to close on
/// frame/auth failures).
pub(crate) fn dispatch_jsonrpc(
    conn: &mut Conn,
    env: &Envelope,
    host: &mut dyn crate::rpc::RpcHost,
    generator: &PasswordGeneratorSettings,
) -> Option<(Envelope, String)> {
    let frame = env.jsonrpc.as_ref()?;
    let secret = conn.session_key.as_ref()?;
    let plaintext = match decrypt_frame(secret, frame) {
        Ok(plaintext) => plaintext,
        Err(_) => return None, // tampered frame or key wiped on lock
    };
    let request: Value = serde_json::from_str(&plaintext).ok()?;
    let method = request.get("method").and_then(|m| m.as_str())?.to_owned();
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let params = request.get("params");
    if method == "FindLogins" {
        let urls: Vec<&str> = params
            .and_then(|p| p.get(0))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();
        eprintln!("[rpc] FindLogins urls={urls:?}");
    }
    let response = match handle_jsonrpc_with_generator(host, &method, params, generator) {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(RpcError::Locked) => jsonrpc_error(&id, -32000, "Vault is locked"),
        Err(RpcError::Unsupported(m)) => {
            jsonrpc_error(&id, -32601, &format!("Unsupported method: {m}"))
        }
        Err(RpcError::InvalidMessage(m)) => jsonrpc_error(&id, -32600, &m),
        Err(RpcError::AuthFailed) => jsonrpc_error(&id, -32603, "Frame authentication failed"),
        Err(RpcError::EntryNotFound) => jsonrpc_error(
            &id,
            -32001,
            "oldLoginUUID could not be resolved to an existing entry.",
        ),
        Err(RpcError::InRecycleBin) => jsonrpc_error(&id, -32002, "Entry is in the Recycle Bin."),
    };
    Some((
        Envelope::jsonrpc(encrypt_frame(secret, &response.to_string())),
        method,
    ))
}
// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

fn features() -> Vec<String> {
    FEATURES.iter().map(|f| f.to_string()).collect()
}

fn jsonrpc_error(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
