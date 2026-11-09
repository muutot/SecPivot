//! Pure KeePassRPC protocol state machines (no sockets): setup + JSON-RPC.
//! Extracted from `rpc::server.rs`.
use serde_json::{json, Value};
use std::time::Instant;
use zeroize::{Zeroize, Zeroizing};

use super::{Conn, SIDE_CHANNEL_BYTES, SIDE_CHANNEL_TTL};
use crate::config::PasswordGeneratorSettings;
use crate::crypto::mac_eq;
use crate::rpc::{
    decrypt_frame, encrypt_frame, handle_jsonrpc_with_generator, hex, key_auth_cr, key_auth_sr,
    random_hex, secret_bytes, Envelope, ErrorMessage, KeyMessage, RpcError, SrpMessage, SrpServer,
    FEATURES, SECURITY_LEVEL,
};

pub(crate) struct DecodedJsonRpc {
    pub(crate) method: String,
    pub(crate) id: Value,
    pub(crate) params: Option<Value>,
}

fn setup_failure(code: &str) -> Envelope {
    let mut reply = Envelope::setup();
    reply.error = Some(ErrorMessage {
        code: code.to_owned(),
        message_params: vec![],
    });
    reply
}

/// The socket's copy proves frame integrity, not current authorization. Check
/// the host key under the same vault lock as dispatch, including lazy expiry.
fn current_key_matches(host: &mut dyn crate::rpc::RpcHost, username: &str, cached: &[u8]) -> bool {
    if username.is_empty() || cached.len() != 32 {
        return false;
    }
    let Some(current) = host.rpc_key(username).map(Zeroizing::new) else {
        return false;
    };
    current.len() == 32 && mac_eq(&Zeroizing::new(hex(&current)), &Zeroizing::new(hex(cached)))
}

pub(crate) fn connection_authorized(conn: &Conn, host: &mut dyn crate::rpc::RpcHost) -> bool {
    let (Some(username), Some(key)) = (&conn.username, &conn.session_key) else {
        return false;
    };
    current_key_matches(host, username, key)
}

/// Socket setup additionally binds a pending SRP proof to its original stable
/// vault-session id. A tab switch or a lock/reopen must not authorize a new host.
pub(crate) fn dispatch_setup_for_session(
    conn: &mut Conn,
    env: &Envelope,
    host: &mut dyn crate::rpc::RpcHost,
    session_id: Option<&str>,
    side_channel: &mut dyn FnMut(&str, u64),
) -> Option<Envelope> {
    let stage = env.srp.as_ref().and_then(|srp| srp.stage.as_deref());
    if stage == Some("proofToServer")
        && (session_id.is_none() || conn.srp_session.as_deref() != session_id)
    {
        conn.srp = None;
        conn.srp_a = None;
        conn.srp_expiry = None;
        conn.srp_session = None;
        return Some(setup_failure("AUTH_FAILED"));
    }
    let reply = dispatch_setup(conn, env, host, side_channel);
    if stage == Some("identifyToServer") && conn.srp.is_some() {
        conn.srp_session = session_id.map(str::to_owned);
    } else if stage == Some("proofToServer") {
        conn.srp_session = None;
    }
    reply
}
/// Pure setup-protocol state machine; returns the envelope to send (or an
/// error envelope). Tested directly with a stub host.
pub(crate) fn dispatch_setup(
    conn: &mut Conn,
    env: &Envelope,
    host: &mut dyn crate::rpc::RpcHost,
    side_channel: &mut dyn FnMut(&str, u64),
) -> Option<Envelope> {
    let error = setup_failure;

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
                let mut password = match random_hex(SIDE_CHANNEL_BYTES) {
                    Ok(password) => password,
                    Err(e) => {
                        eprintln!("[rpc] side-channel password unavailable: {e}");
                        return Some(error("AUTH_FAILED"));
                    }
                };
                let expires = SIDE_CHANNEL_TTL.as_secs();
                side_channel(&password, expires);
                let (server, payload) = match SrpServer::begin(&password) {
                    Ok(pair) => pair,
                    Err(e) => {
                        eprintln!("[rpc] SRP handshake unavailable: {e}");
                        return Some(error("AUTH_FAILED"));
                    }
                };
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
                if !host.is_open() {
                    return Some(error("AUTH_FAILED"));
                }
                eprintln!(
                    "[rpc] proofToServer received (a_len={}, m_len={})",
                    a.len(),
                    m.len()
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
            let secret = Zeroizing::new(conn.key_secret.take()?);
            let sc = conn.key_sc.take()?;
            let username = conn.username.as_deref()?;
            if !current_key_matches(host, username, &secret) {
                return Some(error("AUTH_RESTART"));
            }
            let cc = key.cc.clone().unwrap_or_default();
            let cr = key.cr.clone().unwrap_or_default();
            let secret_hex = Zeroizing::new(hex(&secret));
            if !mac_eq(&cr, &key_auth_cr(&secret_hex, &sc, &cc)) {
                return Some(error("AUTH_FAILED"));
            }
            let sr = key_auth_sr(&secret_hex, &sc, &cc);
            conn.session_key = Some(secret.to_vec());
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
            let Some(secret) = host.rpc_key(&username).map(Zeroizing::new) else {
                return Some(error("AUTH_RESTART"));
            };
            if secret.len() != 32 {
                return Some(error("AUTH_RESTART"));
            }
            let sc = match random_hex(32) {
                Ok(sc) => sc,
                Err(e) => {
                    eprintln!("[rpc] key secret unavailable: {e}");
                    return Some(error("AUTH_RESTART"));
                }
            };
            conn.key_secret = Some(secret.to_vec());
            conn.key_sc = Some(sc.clone());
            conn.username = Some(username);
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
#[cfg(test)]
pub(crate) fn dispatch_jsonrpc(
    conn: &mut Conn,
    env: &Envelope,
    host: &mut dyn crate::rpc::RpcHost,
    generator: &PasswordGeneratorSettings,
) -> Option<(Envelope, String)> {
    let request = decode_jsonrpc(conn, env)?;
    dispatch_decoded_jsonrpc(conn, request, host, generator)
}

pub(crate) fn decode_jsonrpc(conn: &Conn, env: &Envelope) -> Option<DecodedJsonRpc> {
    let frame = env.jsonrpc.as_ref()?;
    let secret = conn.session_key.as_ref()?;
    let plaintext = match decrypt_frame(secret, frame) {
        Ok(plaintext) => plaintext,
        Err(_) => return None, // tampered frame or key wiped on lock
    };
    let request: Value = serde_json::from_str(&plaintext).ok()?;
    let method = request.get("method").and_then(|m| m.as_str())?.to_owned();
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let params = request.get("params").cloned();
    Some(DecodedJsonRpc { method, id, params })
}

pub(crate) fn dispatch_decoded_jsonrpc(
    conn: &Conn,
    request: DecodedJsonRpc,
    host: &mut dyn crate::rpc::RpcHost,
    generator: &PasswordGeneratorSettings,
) -> Option<(Envelope, String)> {
    if !connection_authorized(conn, host) {
        return None;
    }
    let method = request.method.clone();
    let result =
        handle_jsonrpc_with_generator(host, &request.method, request.params.as_ref(), generator);
    encode_jsonrpc_result(conn, &request.id, result).map(|reply| (reply, method))
}

pub(crate) fn encode_jsonrpc_result(
    conn: &Conn,
    id: &Value,
    result: Result<Value, RpcError>,
) -> Option<Envelope> {
    let secret = conn.session_key.as_ref()?;
    let response = match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(RpcError::Locked) => jsonrpc_error(id, -32000, "Vault is locked"),
        Err(RpcError::Unsupported(m)) => {
            jsonrpc_error(id, -32601, &format!("Unsupported method: {m}"))
        }
        Err(RpcError::InvalidMessage(m)) => jsonrpc_error(id, -32600, &m),
        Err(RpcError::AuthFailed) => jsonrpc_error(id, -32603, "Frame authentication failed"),
        Err(RpcError::EntryNotFound) => jsonrpc_error(
            id,
            -32001,
            "oldLoginUUID could not be resolved to an existing entry.",
        ),
        Err(RpcError::InRecycleBin) => jsonrpc_error(id, -32002, "Entry is in the Recycle Bin."),
    };
    let frame = match encrypt_frame(secret, &response.to_string()) {
        Ok(frame) => frame,
        Err(e) => {
            eprintln!("[rpc] response frame unavailable: {e:?}");
            return None;
        }
    };
    Some(Envelope::jsonrpc(frame))
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
