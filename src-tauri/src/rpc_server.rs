//! KeePassRPC loopback WebSocket server (Kee 4.x bridge).
//!
//! Binds `127.0.0.1:12546` only and speaks the KeePassRPC 1.8.x wire
//! protocol over WebSocket, matching what Kee 4.0.7 sends:
//!
//! - **SRP-6a setup** (`identifyToServer` → `identifyToClient` →
//!   `proofToServer` → `proofToClient`). The side-channel password is
//!   generated here, shown once via `rpc-side-channel-request`, and consumed
//!   by `SrpServer` immediately (zeroized; never logged or persisted).
//! - **Stored-key setup** (`key:{username}` → `key:{sc}` → `key:{cc,cr}` →
//!   `key:{sr}`), available only while the session key still lives in
//!   `VaultSession` (wiped on lock, so a locked vault forces a fresh SRP).
//! - **Encrypted JSON-RPC** frames, AES-256-CBC with the protocol's naive
//!   SHA-1 MAC, dispatched through `rpc::handle_jsonrpc` under the session
//!   lock (never held across network I/O).
//!
//! The server itself holds no secrets beyond the per-connection handshake
//! state, which is zeroized on drop; long-lived keys live in the session.

use crate::rpc::{
    decrypt_frame, encrypt_frame, handle_jsonrpc, hex, key_auth_cr, key_auth_sr, random_hex,
    secret_bytes, Envelope, ErrorMessage, KeyMessage, RpcError, SrpMessage, SrpServer, FEATURES,
    RPC_PORT, SECURITY_LEVEL,
};
use crate::vault::VaultSession;
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::{accept_hdr, Message as WsMessage, WebSocket};
use zeroize::Zeroize;

/// How long a connection may stall in a handshake step before it is dropped.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
/// Idle cap for an authenticated data connection (Kee reconnects on demand).
const DATA_TIMEOUT: Duration = Duration::from_secs(300);
/// Side-channel password lifetime (mirrors the KeePassRPC 120 s default).
const SIDE_CHANNEL_TTL: Duration = Duration::from_secs(120);
/// Side-channel password entropy: 8 bytes → 16 lowercase hex chars.
const SIDE_CHANNEL_BYTES: usize = 8;
/// Payload cap (well above any real KeePassRPC message).
const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Event emitted to the frontend so the side-channel password can be shown.
/// Payload: `{ password, expiresInSecs }` — never persisted, never logged.
pub(crate) const SIDE_CHANNEL_EVENT: &str = "rpc-side-channel-request";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SideChannelRequest {
    pub(crate) password: String,
    pub(crate) expires_in_secs: u64,
}

// ---------------------------------------------------------------------------
// Server lifecycle (mirrors bridge_server::BridgeState)
// ---------------------------------------------------------------------------

/// Owns the accept-loop thread; `stop()` signals it and joins.
pub(crate) struct ServerHandle {
    stop: mpsc::Sender<()>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl ServerHandle {
    fn stop(mut self) {
        let _ = self.stop.send(());
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Managed state: the running server, if any. Start is idempotent.
#[derive(Default)]
pub(crate) struct RpcState {
    server: Mutex<Option<ServerHandle>>,
    last_error: Mutex<Option<String>>,
}

impl RpcState {
    pub(crate) fn running(&self) -> bool {
        self.server.lock().ok().is_some_and(|guard| guard.is_some())
    }

    pub(crate) fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|e| e.clone())
    }

    pub(crate) fn start(&self, app: &AppHandle) -> Result<(), String> {
        let mut guard = self
            .server
            .lock()
            .map_err(|_| "RPC 状态锁已损坏".to_owned())?;
        if guard.is_some() {
            return Ok(());
        }
        let listener = match TcpListener::bind(("127.0.0.1", RPC_PORT)) {
            Ok(listener) => listener,
            Err(e) => {
                let message =
                    format!("无法监听 127.0.0.1:{RPC_PORT} (端口可能已被 KeePass 客户端占用): {e}");
                if let Ok(mut slot) = self.last_error.lock() {
                    *slot = Some(message.clone());
                }
                return Err(message);
            }
        };
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("设置监听模式失败: {e}"))?;
        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let app = app.clone();
        let join = std::thread::spawn(move || accept_loop(&listener, &app, &stop_rx));
        *guard = Some(ServerHandle {
            stop: stop_tx,
            join: Some(join),
        });
        if let Ok(mut slot) = self.last_error.lock() {
            *slot = None;
        }
        Ok(())
    }

    pub(crate) fn stop(&self) {
        if let Ok(mut guard) = self.server.lock() {
            if let Some(handle) = guard.take() {
                handle.stop();
            }
        }
    }
}

/// Accept loopback connections until told to stop; each connection is served
/// on its own thread so a slow client never blocks the listener.
fn accept_loop(listener: &TcpListener, app: &AppHandle, stop: &mpsc::Receiver<()>) {
    while stop.try_recv().is_err() {
        match listener.accept() {
            Ok((stream, _addr)) => {
                // Accepted streams inherit the listener's non-blocking flag on
                // Windows; tungstenite needs a blocking stream, so restore it
                // (a non-blocking read yields WSAEWOULDBLOCK → os error 10035).
                let _ = stream.set_nonblocking(false);
                let app = app.clone();
                std::thread::spawn(move || handle_connection(stream, &app));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

// ---------------------------------------------------------------------------
// One connection
// ---------------------------------------------------------------------------

/// Per-connection handshake/authenticated state. Secrets are zeroized on
/// drop; the long-lived session key lives in `VaultSession` (wiped on lock).
#[derive(Default)]
struct Conn {
    /// SRP username (`I`) — registered as the stored-key identity at proof.
    username: Option<String>,
    /// Client's ephemeral `A` (needed again at `proofToServer`).
    srp_a: Option<String>,
    srp: Option<SrpServer>,
    srp_expiry: Option<Instant>,
    /// Key-auth challenge state (sc served, cc/cr expected next).
    key_secret: Option<Vec<u8>>,
    key_sc: Option<String>,
    /// Authenticated session key (mirror of the session-held copy).
    session_key: Option<Vec<u8>>,
}

impl Drop for Conn {
    fn drop(&mut self) {
        if let Some(key) = &mut self.key_secret {
            key.zeroize();
        }
        if let Some(key) = &mut self.session_key {
            key.zeroize();
        }
    }
}

/// Accept a WebSocket upgrade on `/`. Any other request gets a 404 with CORS
/// headers — Kee 2.x probes the port with a plain `GET /pingAvailabilityTest`
/// and only proceeds to the WebSocket handshake when that probe returns 404.
#[allow(clippy::result_large_err)] // ErrorResponse is tungstenite's type, not ours
fn accept_callback(req: &Request, res: Response) -> Result<Response, ErrorResponse> {
    if req.uri().path() == "/" {
        return Ok(res);
    }
    Err(Response::builder()
        .status(tungstenite::http::StatusCode::NOT_FOUND)
        .body(None)
        .expect("valid response"))
}

/// Kee 2.x's port-availability probe answer (plain HTTP, no WebSocket).
const PROBE_404_RESPONSE: &[u8] = b"HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Headers: Content-Type, X-Requested-With\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";

/// Read the request head (up to `\r\n\r\n`) via `peek` and judge whether the
/// client is asking for a WebSocket upgrade; nothing is consumed from the
/// stream, so the head is still visible to tungstenite afterwards.
fn is_websocket_upgrade(stream: &mut TcpStream) -> bool {
    let mut head = Vec::new();
    let mut buf = [0u8; 2048];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") && head.len() < 64 * 1024 {
        match stream.peek(&mut buf) {
            Ok(0) => return false,
            Ok(n) => head.extend_from_slice(&buf[..n]),
            Err(_) => return false,
        }
    }
    String::from_utf8_lossy(&head)
        .to_ascii_lowercase()
        .contains("upgrade: websocket")
}

/// Serve one inbound connection: plain-HTTP port probes get Kee's expected
/// 404, WebSocket upgrades go through tungstenite.
fn serve_connection(mut stream: TcpStream) -> Option<WebSocket<TcpStream>> {
    if !is_websocket_upgrade(&mut stream) {
        eprintln!("[rpc] plain HTTP probe answered 404");
        let _ = stream.write_all(PROBE_404_RESPONSE);
        let _ = stream.shutdown(std::net::Shutdown::Write);
        // Drain the request so the close is a graceful FIN — Windows sends
        // an RST when a socket with unread data is dropped.
        let mut buf = [0u8; 1024];
        loop {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
        }
        return None;
    }
    // The request head was only peeked, so tungstenite still sees it.
    match accept_hdr(stream, accept_callback) {
        Ok(ws) => Some(ws),
        Err(e) => {
            eprintln!("[rpc] websocket accept failed: {e}");
            None
        }
    }
}

fn handle_connection(stream: TcpStream, app: &AppHandle) {
    let _ = stream.set_read_timeout(Some(HANDSHAKE_TIMEOUT));
    eprintln!("[rpc] connection accepted");
    let Some(mut ws) = serve_connection(stream) else {
        return;
    };
    eprintln!("[rpc] websocket upgraded");
    // Handshake is done; allow longer idle on the data path.
    let _ = ws.get_mut().set_read_timeout(Some(DATA_TIMEOUT));
    let mut conn = Conn::default();
    loop {
        let msg = match ws.read() {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("[rpc] ws read error: {e}");
                break;
            }
        };
        match msg {
            WsMessage::Text(text) => {
                if text.len() > MAX_FRAME_BYTES {
                    break;
                }
                if !step(&mut ws, &mut conn, &text, app) {
                    break;
                }
            }
            WsMessage::Ping(_) => {
                let _ = ws.send(WsMessage::Pong(Vec::new().into()));
            }
            WsMessage::Close(_) => break,
            // Binary frames are not part of the KeePassRPC protocol.
            WsMessage::Binary(_) | WsMessage::Frame(_) | WsMessage::Pong(_) => {}
        }
    }
}

/// Dispatch one text envelope; returns `false` when the connection should
/// close (error sent, protocol violation, or a completed handshake).
fn step(ws: &mut WebSocket<TcpStream>, conn: &mut Conn, text: &str, app: &AppHandle) -> bool {
    let env: Envelope = match serde_json::from_str(text) {
        Ok(env) => env,
        Err(e) => {
            eprintln!("[rpc] unparsable envelope: {e}");
            return false;
        }
    };
    eprintln!("[rpc] envelope protocol={}", env.protocol);
    eprintln!(
        "[rpc]   raw: {}",
        text.chars().take(400).collect::<String>()
    );
    match env.protocol.as_str() {
        "setup" => reply_setup(ws, conn, env, app),
        "jsonrpc" => reply_jsonrpc(ws, conn, env, app),
        // ping / pong / teardown / reconnect — KeePassRPC client keepalives.
        _ => true,
    }
}

/// Pure setup-protocol state machine; returns the envelope to send (or an
/// error envelope). Tested directly with a stub host.
fn dispatch_setup(
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

fn reply_setup(
    ws: &mut WebSocket<TcpStream>,
    conn: &mut Conn,
    env: Envelope,
    app: &AppHandle,
) -> bool {
    if env.error.is_some() {
        // Client-side teardown error — close without answering.
        return false;
    }
    let Some(session_state) = app.try_state::<Mutex<VaultSession>>() else {
        eprintln!("[rpc] VaultSession state missing");
        return false;
    };
    let Some(reply) = ({
        let Ok(mut session) = session_state.lock() else {
            eprintln!("[rpc] VaultSession lock poisoned");
            return false;
        };
        dispatch_setup(conn, &env, &mut *session, &mut |password, expires| {
            let _ = app.emit(
                SIDE_CHANNEL_EVENT,
                SideChannelRequest {
                    password: password.to_owned(),
                    expires_in_secs: expires,
                },
            );
        })
    }) else {
        eprintln!(
            "[rpc] dispatch_setup returned None (srp={:?} key={:?} error={:?})",
            env.srp
                .as_ref()
                .map(|s| s.stage.clone().unwrap_or_default()),
            env.key.as_ref().map(|k| format!(
                "username={:?} sc={} cc={} cr={} sr={}",
                k.username,
                k.sc.is_some(),
                k.cc.is_some(),
                k.cr.is_some(),
                k.sr.is_some()
            )),
            env.error.as_ref().map(|e| e.code.clone())
        );
        return false;
    };
    let error_sent = reply.error.is_some();
    let sent = send_envelope(ws, &reply);
    eprintln!("[rpc] setup reply sent={sent} error={error_sent}");
    if !sent || error_sent {
        let _ = ws.close(None);
        false
    } else {
        true
    }
}

/// Pure JSON-RPC dispatch: decrypt → `handle_jsonrpc` → encrypt. Returns the
/// encrypted response envelope (or `None` to close on frame/auth failures).
fn dispatch_jsonrpc(
    conn: &mut Conn,
    env: &Envelope,
    host: &mut dyn crate::rpc::RpcHost,
) -> Option<Envelope> {
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
    let response = match handle_jsonrpc(host, &method, params) {
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
    Some(Envelope::jsonrpc(encrypt_frame(
        secret,
        &response.to_string(),
    )))
}

fn reply_jsonrpc(
    ws: &mut WebSocket<TcpStream>,
    conn: &mut Conn,
    env: Envelope,
    app: &AppHandle,
) -> bool {
    let Some(session_state) = app.try_state::<Mutex<VaultSession>>() else {
        return false;
    };
    let Ok(mut session) = session_state.lock() else {
        return false;
    };
    match dispatch_jsonrpc(conn, &env, &mut *session) {
        Some(reply) => send_envelope(ws, &reply),
        None => false,
    }
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

fn send_envelope(ws: &mut WebSocket<TcpStream>, env: &Envelope) -> bool {
    match serde_json::to_string(env) {
        Ok(json) => ws.send(WsMessage::Text(json.into())).is_ok(),
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rpc::{hex, RpcHost};
    use num_bigint::{BigInt, BigUint, Sign};
    use std::collections::HashMap;
    use std::net::TcpStream;
    use std::time::{Duration, Instant};

    /// Minimal in-memory RpcHost (no crypto secrets beyond the fake key).
    struct FakeHost {
        open: bool,
        keys: HashMap<String, Vec<u8>>,
    }

    impl FakeHost {
        fn open() -> Self {
            Self {
                open: true,
                keys: HashMap::new(),
            }
        }
        fn locked() -> Self {
            Self {
                open: false,
                keys: HashMap::new(),
            }
        }
        fn database() -> crate::rpc::RpcDatabase {
            crate::rpc::RpcDatabase {
                name: "My Vault".to_owned(),
                file_name: "vault.kdbx".to_owned(),
                icon_image_data: String::new(),
                root: crate::rpc::RpcGroup {
                    uuid: "g-root".to_owned(),
                    title: "Root".to_owned(),
                    path: String::new(),
                    icon_image_data: String::new(),
                    entries: Vec::new(),
                    children: Vec::new(),
                },
                active: true,
            }
        }
    }

    impl RpcHost for FakeHost {
        fn is_open(&self) -> bool {
            self.open
        }
        fn rpc_key(&self, username: &str) -> Option<Vec<u8>> {
            self.keys.get(username).cloned()
        }
        fn register_rpc_key(&mut self, username: &str, key: Vec<u8>) {
            self.keys.insert(username.to_owned(), key);
        }
        fn database(&self) -> Option<crate::rpc::RpcDatabase> {
            self.open.then(Self::database)
        }
        fn find_logins(
            &self,
            _urls: &[String],
            _uuid: Option<&str>,
            _free_text: Option<&str>,
            _username: Option<&str>,
        ) -> Vec<crate::rpc::RpcLogin> {
            Vec::new()
        }
        fn add_login(
            &mut self,
            _login: &crate::rpc::RpcLoginWrite,
            _parent_uuid: &str,
        ) -> Result<crate::rpc::RpcLogin, RpcError> {
            Err(RpcError::Unsupported("AddLogin".to_owned()))
        }
        fn update_login(
            &mut self,
            _login: &crate::rpc::RpcLoginWrite,
            _old_uuid: &str,
            _url_merge_mode: u8,
        ) -> Result<crate::rpc::RpcLogin, RpcError> {
            Err(RpcError::Unsupported("UpdateLogin".to_owned()))
        }
    }

    /// Client side of the SRP handshake: mirrors `SRPc` in kprpcClient.js so
    /// the server-side path is exercised with the extension's own math.
    /// `client_ephemeral` derives `(A = g^a mod N, a)` — the client knows A
    /// before identifying; `client_proof` computes `M`/`M2` from the server's
    /// salt and B, reusing the same scalar `a`.
    fn client_ephemeral() -> (String, BigUint) {
        let group_hex = crate::rpc::group_n_hex();
        let n = BigUint::parse_bytes(group_hex.as_bytes(), 16).unwrap();
        let g = BigUint::from(2u32);
        let a = BigUint::from_bytes_be(&random_hex(32).into_bytes());
        let a_hex = g.modpow(&a, &n).to_str_radix(16).to_uppercase();
        (a_hex, a)
    }

    fn client_proof(
        salt: &str,
        b_hex: &str,
        password: &str,
        a_hex: &str,
        a: &BigUint,
    ) -> (String, String) {
        let group_hex = crate::rpc::group_n_hex();
        let n = BigUint::parse_bytes(group_hex.as_bytes(), 16).unwrap();
        let g = BigUint::from(2u32);
        let k = BigUint::parse_bytes("b7867f1299da8cc24ab93e08986ebc4d6a478ad0".as_bytes(), 16)
            .unwrap();
        let b = BigUint::parse_bytes(b_hex.as_bytes(), 16).unwrap();
        let u = BigUint::parse_bytes(sha256(&format!("{a_hex}{b_hex}")).as_bytes(), 16).unwrap();
        let x = BigUint::parse_bytes(sha256(&format!("{salt}{password}")).as_bytes(), 16).unwrap();
        let kgx = (&k * g.modpow(&x, &n)) % &n;
        let aux = a + &u * &x;
        let s = ((&b + &n - &kgx) % &n).modpow(&aux, &n);
        let s_upper = s.to_str_radix(16).to_uppercase();
        let m = sha256(&format!("{a_hex}{b_hex}{s_upper}"));
        let m2 = sha256(&format!("{a_hex}{m}{s_upper}"));
        (m, m2)
    }

    /// JS-style modular exponentiation exactly as Kee 4.0.6's `SRP.ts` does it:
    /// `%` on a negative base yields a negative remainder (ECMAScript BigInt
    /// semantics), unlike `num-bigint`'s non-negative `%`.
    fn js_modpow(base: &BigInt, exponent: &BigInt, modulus: &BigInt) -> BigInt {
        let mut result = BigInt::from(1u32);
        let mut base = base.clone();
        let mut exponent = exponent.clone();
        while exponent > BigInt::ZERO {
            if (&exponent & BigInt::from(1u32)) != BigInt::ZERO {
                result = (&result * &base) % modulus;
            }
            exponent >>= 1;
            if exponent > BigInt::ZERO {
                base = (&base * &base) % modulus;
            }
        }
        result
    }

    /// Byte-for-byte replication of `SRPc.calculations` from Kee 4.0.6
    /// (`src/background/SRP.ts`), including its negative-remainder quirk.
    /// Returns the client's M and whether the client-side S came out negative
    /// (JS `BigInt.toString(16)` would print "-…", corrupting the M preimage).
    fn kee_406_client_proof(
        salt: &str,
        b_hex: &str,
        password: &str,
        a_hex: &str,
        a: &BigUint,
    ) -> (String, bool) {
        let group_hex = crate::rpc::group_n_hex();
        let n = BigUint::parse_bytes(group_hex.as_bytes(), 16).unwrap();
        let n_big = BigInt::parse_bytes(group_hex.as_bytes(), 16).unwrap();
        let g = BigUint::from(2u32);
        let k = BigUint::parse_bytes("b7867f1299da8cc24ab93e08986ebc4d6a478ad0".as_bytes(), 16)
            .unwrap();
        let u = BigUint::parse_bytes(sha256(&format!("{a_hex}{b_hex}")).as_bytes(), 16).unwrap();
        let x = BigUint::parse_bytes(sha256(&format!("{salt}{password}")).as_bytes(), 16).unwrap();
        let kgx = (&k * g.modpow(&x, &n)) % &n;
        let aux = a + &u * &x;
        let base = BigInt::parse_bytes(b_hex.as_bytes(), 16).unwrap()
            - BigInt::from_biguint(Sign::Plus, kgx);
        let s_js = js_modpow(&base, &BigInt::from_biguint(Sign::Plus, aux), &n_big);
        let negative = s_js.sign() == Sign::Minus;
        // JS `BigInt.toString(16)` keeps a "-" prefix; the client uppercases
        // the whole string when building the M preimage.
        let s_str = s_js.to_str_radix(16);
        let m = sha256(&format!("{a_hex}{b_hex}{}", s_str.to_uppercase()));
        (m, negative)
    }

    /// Statistical probe: run the exact Kee 4.0.6 client SRP math against the
    /// server 200 times and report how often the server would reject M.
    #[test]
    fn kee_406_client_srp_compatibility_statistics() {
        let trials = 200u32;
        let mut rejected = 0u32;
        let mut negative_s = 0u32;
        let mut rejected_with_positive_s = 0u32;
        for _ in 0..trials {
            let (a_hex, a) = client_ephemeral();
            let (mut server, payload) = crate::rpc::SrpServer::begin("password-shared");
            let salt = payload["s"].as_str().unwrap().to_owned();
            let b_hex = payload["B"].as_str().unwrap().to_owned();
            let (m, neg) = kee_406_client_proof(&salt, &b_hex, "password-shared", &a_hex, &a);
            if neg {
                negative_s += 1;
            }
            if server.verify_proof(&a_hex, &m).is_err() {
                rejected += 1;
                if !neg {
                    rejected_with_positive_s += 1;
                }
            }
        }
        eprintln!(
            "Kee 4.0.6 SRP compat: {rejected}/{trials} rejected, \
             {negative_s}/{trials} negative S, \
             {rejected_with_positive_s} rejected with positive S"
        );
        assert_eq!(
            rejected, 0,
            "every Kee 4.0.6 handshake must succeed via the negative-S fallback"
        );
    }

    fn sha256(input: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(input);
        hex(&hasher.finalize())
    }

    fn setup_env(
        srp: Option<SrpMessage>,
        key: Option<KeyMessage>,
        error: Option<ErrorMessage>,
    ) -> Envelope {
        let mut env = Envelope::setup();
        env.srp = srp;
        env.key = key;
        env.error = error;
        env
    }

    fn identify_to_server(username: &str, a: &str) -> Envelope {
        setup_env(
            Some(SrpMessage {
                stage: Some("identifyToServer".to_owned()),
                i: Some(username.to_owned()),
                a: Some(a.to_owned()),
                security_level: Some(2),
                ..Default::default()
            }),
            None,
            None,
        )
    }

    fn proof_to_server(m: &str) -> Envelope {
        setup_env(
            Some(SrpMessage {
                stage: Some("proofToServer".to_owned()),
                m: Some(m.to_owned()),
                security_level: Some(2),
                ..Default::default()
            }),
            None,
            None,
        )
    }

    fn key_request(username: &str) -> Envelope {
        setup_env(
            None,
            Some(KeyMessage {
                username: Some(username.to_owned()),
                security_level: Some(2),
                ..Default::default()
            }),
            None,
        )
    }

    fn key_challenge_response(cc: &str, cr: &str) -> Envelope {
        setup_env(
            None,
            Some(KeyMessage {
                cc: Some(cc.to_owned()),
                cr: Some(cr.to_owned()),
                security_level: Some(2),
                ..Default::default()
            }),
            None,
        )
    }

    #[test]
    fn full_srp_handshake_registers_key_and_round_trips_jsonrpc() {
        let mut host = FakeHost::open();
        let mut conn = Conn::default();
        let mut shown: Option<(String, u64)> = None;

        let mut show = |pw: &str, expires: u64| shown = Some((pw.to_owned(), expires));
        // The client knows its own ephemeral A before identifying.
        let (a_hex, a) = client_ephemeral();
        let reply = dispatch_setup(
            &mut conn,
            &identify_to_server("alice@kprpc", &a_hex),
            &mut host,
            &mut show,
        )
        .expect("identifyToClient reply");
        assert_eq!(
            reply.srp.as_ref().unwrap().stage.as_deref(),
            Some("identifyToClient")
        );
        assert!(reply.features.is_some());
        let srp_msg = reply.srp.as_ref().unwrap();
        let s = srp_msg.s.as_ref().unwrap().clone();
        let b = srp_msg.b.as_ref().unwrap().clone();
        let (password, expires) = shown.expect("side-channel emitted");
        assert_eq!(expires, SIDE_CHANNEL_TTL.as_secs());
        assert_eq!(password.len(), SIDE_CHANNEL_BYTES * 2);

        let (m, m2) = client_proof(&s, &b, &password, &a_hex, &a);
        let reply = dispatch_setup(&mut conn, &proof_to_server(&m), &mut host, &mut |_, _| {})
            .expect("proofToClient reply");
        assert_eq!(
            reply.srp.as_ref().unwrap().stage.as_deref(),
            Some("proofToClient"),
            "Kee dispatches on srp.stage == \"proofToClient\"; without it the client silently aborts"
        );
        assert_eq!(reply.srp.as_ref().unwrap().m2.as_ref().unwrap(), &m2);

        assert!(host.rpc_key("alice@kprpc").is_some());
        assert!(conn.session_key.is_some());

        // JSON-RPC round trip under the derived session key.
        let request = json!({"jsonrpc":"2.0","id":41,"method":"GetAllDatabases","params":[]});
        let env = Envelope::jsonrpc(encrypt_frame(
            conn.session_key.as_ref().unwrap(),
            &request.to_string(),
        ));
        let reply = dispatch_jsonrpc(&mut conn, &env, &mut host).expect("jsonrpc reply");
        let frame = reply.jsonrpc.as_ref().unwrap();
        let plaintext = decrypt_frame(conn.session_key.as_ref().unwrap(), frame).expect("decrypt");
        let value: Value = serde_json::from_str(&plaintext).unwrap();
        assert_eq!(value["id"], 41);
        assert!(value["result"].is_array());
    }

    #[test]
    fn wrong_password_fails_and_expiry_answers_auth_expired() {
        let mut host = FakeHost::open();
        let mut shown = None;
        let (a_hex, a) = client_ephemeral();
        let mut conn = Conn::default();
        let reply = dispatch_setup(
            &mut conn,
            &identify_to_server("bob@kprpc", &a_hex),
            &mut host,
            &mut |pw, _| shown = Some(pw.to_owned()),
        )
        .unwrap();
        let srp = reply.srp.unwrap();
        let s = srp.s.unwrap();
        let b = srp.b.unwrap();
        let (m_correct, _) = client_proof(&s, &b, shown.unwrap().as_str(), &a_hex, &a);
        let (m_wrong, _) = client_proof(&s, &b, "not-the-password", &a_hex, &a);

        let mut conn = Conn::default();
        dispatch_setup(
            &mut conn,
            &identify_to_server("bob@kprpc", &a_hex),
            &mut host,
            &mut |_, _| {},
        )
        .unwrap();
        let reply = dispatch_setup(
            &mut conn,
            &proof_to_server(&m_wrong),
            &mut host,
            &mut |_, _| {},
        )
        .expect("error reply");
        assert_eq!(reply.error.as_ref().unwrap().code, "AUTH_FAILED");
        assert!(host.rpc_key("bob@kprpc").is_none());

        // Same handshake, expired → AUTH_EXPIRED.
        let mut conn = Conn::default();
        dispatch_setup(
            &mut conn,
            &identify_to_server("bob@kprpc", &a_hex),
            &mut host,
            &mut |_, _| {},
        )
        .unwrap();
        conn.srp_expiry = Some(Instant::now() - Duration::from_secs(1));
        let reply = dispatch_setup(
            &mut conn,
            &proof_to_server(&m_correct),
            &mut host,
            &mut |_, _| {},
        )
        .expect("error reply");
        assert_eq!(reply.error.as_ref().unwrap().code, "AUTH_EXPIRED");
    }

    #[test]
    fn stored_key_challenge_round_trip_and_restart_on_missing_key() {
        let mut host = FakeHost::open();
        host.register_rpc_key("alice@kprpc", vec![9u8; 32]);

        // Missing key → AUTH_RESTART (client forgets and re-SRPs).
        let mut conn = Conn::default();
        let reply = dispatch_setup(
            &mut conn,
            &key_request("nobody@kprpc"),
            &mut host,
            &mut |_, _| {},
        )
        .expect("error reply");
        assert_eq!(reply.error.as_ref().unwrap().code, "AUTH_RESTART");

        // Existing key → sc challenge → correct cr → sr.
        let mut conn = Conn::default();
        let reply = dispatch_setup(
            &mut conn,
            &key_request("alice@kprpc"),
            &mut host,
            &mut |_, _| {},
        )
        .expect("sc challenge");
        let sc = reply.key.as_ref().unwrap().sc.clone().unwrap();
        assert_eq!(
            reply.key.as_ref().unwrap().security_level,
            Some(SECURITY_LEVEL)
        );
        assert!(reply.features.is_some());
        let cc = random_hex(16);
        let secret_hex = hex(&[9u8; 32]);
        let cr = key_auth_cr(&secret_hex, &sc, &cc);

        let reply = dispatch_setup(
            &mut conn,
            &key_challenge_response(&cc, &cr),
            &mut host,
            &mut |_, _| {},
        )
        .expect("sr reply");
        let sr = reply.key.as_ref().unwrap().sr.clone().unwrap();
        assert_eq!(sr, key_auth_sr(&secret_hex, &sc, &cc));
        assert!(conn.session_key.is_some());

        // Wrong cr is rejected and no session key is installed.
        let mut conn = Conn::default();
        dispatch_setup(
            &mut conn,
            &key_request("alice@kprpc"),
            &mut host,
            &mut |_, _| {},
        )
        .unwrap();
        let reply = dispatch_setup(
            &mut conn,
            &key_challenge_response(&random_hex(16), &random_hex(32)),
            &mut host,
            &mut |_, _| {},
        )
        .expect("error reply");
        assert_eq!(reply.error.as_ref().unwrap().code, "AUTH_FAILED");
        assert!(conn.session_key.is_none());
    }

    #[test]
    fn locked_vault_refuses_srp_and_keys() {
        let mut host = FakeHost::locked();
        let mut conn = Conn::default();
        let reply = dispatch_setup(
            &mut conn,
            &identify_to_server("alice@kprpc", "A1B2"),
            &mut host,
            &mut |_, _| panic!("no side-channel password while locked"),
        )
        .expect("error reply");
        assert_eq!(reply.error.as_ref().unwrap().code, "AUTH_FAILED");

        let reply = dispatch_setup(
            &mut conn,
            &key_request("alice@kprpc"),
            &mut host,
            &mut |_, _| {},
        )
        .expect("error reply");
        assert_eq!(reply.error.as_ref().unwrap().code, "AUTH_RESTART");
    }

    #[test]
    fn tampered_or_keyless_jsonrpc_frames_close() {
        let mut host = FakeHost::open();
        host.register_rpc_key("alice@kprpc", vec![3u8; 32]);
        let request = json!({"jsonrpc":"2.0","id":1,"method":"GetAllDatabases"});
        let env = Envelope::jsonrpc(encrypt_frame(&[9u8; 32], &request.to_string()));

        // Key wiped (as on lock) → frame MAC fails → no reply.
        let mut conn = Conn::default();
        conn.session_key = Some(vec![3u8; 32]);
        assert!(dispatch_jsonrpc(&mut conn, &env, &mut host).is_none());

        // No session key at all → no reply.
        let mut conn = Conn::default();
        assert!(dispatch_jsonrpc(&mut conn, &env, &mut host).is_none());
    }

    /// Transport-level check: prove the tungstenite accept path works on
    /// loopback with the exact handshake and framing Kee sends (raw sockets,
    /// masked client frames, unmasked server frames).
    mod ws_transport {
        use super::*;
        use std::io::{Read, Write};

        fn handshake_request(key: &str) -> String {
            format!(
                "GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: {key}\r\nSec-WebSocket-Version: 13\r\nOrigin: moz-extension://test\r\n\r\n"
            )
        }

        fn masked_text(payload: &[u8]) -> Vec<u8> {
            let mask = [1u8, 2, 3, 4];
            let mut frame = vec![0x81, 0x80 | payload.len() as u8];
            frame.extend_from_slice(&mask);
            for (i, byte) in payload.iter().enumerate() {
                frame.push(byte ^ mask[i % 4]);
            }
            frame
        }

        fn read_frame(stream: &mut TcpStream) -> (u8, Vec<u8>) {
            let mut head = [0u8; 2];
            stream.read_exact(&mut head).unwrap();
            let opcode = head[0] & 0x0f;
            assert_eq!(head[1] & 0x80, 0, "server frames must not be masked");
            let len = head[1] & 0x7f;
            let len = if len < 126 {
                len as usize
            } else {
                let mut ext = [0u8; 8];
                let n = if len == 126 { 2 } else { 8 };
                stream.read_exact(&mut ext[..n]).unwrap();
                let mut out = 0usize;
                for byte in &ext[..n] {
                    out = (out << 8) | *byte as usize;
                }
                out
            };
            let mut payload = vec![0u8; len];
            if len > 0 {
                stream.read_exact(&mut payload).unwrap();
            }
            (opcode, payload)
        }

        #[test]
        fn ws_handshake_accepts_kee_request_and_answers_pong() {
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let addr = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let mut ws = serve_connection(stream).expect("ws upgrade via serve_connection");
                let msg = ws.read().expect("text frame");
                assert_eq!(
                    msg,
                    WsMessage::Text(r#"{"protocol":"ping"}"#.to_owned().into())
                );
                let _ = ws.send(WsMessage::Pong(Vec::new().into()));
                let _ = ws.read(); // client close
            });

            let mut stream = TcpStream::connect(addr).unwrap();
            stream
                .write_all(handshake_request("dGhlIHNhbXBsZSBub25jZQ==").as_bytes())
                .unwrap();
            let mut response = Vec::new();
            let mut one = [0u8; 1];
            while !response.ends_with(b"\r\n\r\n") && response.len() < 8192 {
                if stream.read(&mut one).unwrap() == 0 {
                    break;
                }
                response.push(one[0]);
            }
            assert!(
                String::from_utf8_lossy(&response).starts_with("HTTP/1.1 101"),
                "handshake rejected: {}",
                String::from_utf8_lossy(&response)
            );

            stream
                .write_all(&masked_text(br#"{"protocol":"ping"}"#))
                .unwrap();
            let (opcode, _) = read_frame(&mut stream);
            assert_eq!(opcode, 0xA, "expected Pong");

            // Masked close frame, code 1000.
            stream
                .write_all(&[0x88, 0x82, 0x11, 0x22, 0x33, 0x44, 0x03 ^ 0x11, 0xe8 ^ 0x22])
                .unwrap();
            let _ = stream.read(&mut [0u8; 8]).ok();
            server.join().unwrap();
        }

        #[test]
        fn serve_connection_round_trips_client_message() {
            // Regression: the peek-based upgrade path must still deliver
            // client frames to tungstenite (a dead connection here showed up
            // as RSTs to real clients right after the WS upgrade).
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let addr = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let mut ws = serve_connection(stream).expect("ws upgrade via serve_connection");
                let msg = ws.read().expect("client text frame");
                eprintln!("server read: {msg:?}");
                let _ = ws.send(WsMessage::Text("pong".to_owned().into()));
                let _ = ws.read(); // client close
            });

            let (mut client, _) = tungstenite::connect(format!("ws://{addr}/")).unwrap();
            client
                .send(WsMessage::Text("hello".to_owned().into()))
                .unwrap();
            let reply = client.read().expect("server reply");
            assert_eq!(reply, WsMessage::Text("pong".to_owned().into()));
            server.join().unwrap();
        }

        #[test]
        fn nonblocking_listener_streams_are_restored_to_blocking() {
            // accept_loop() keeps the listener non-blocking; on Windows
            // accepted streams inherit that flag and tungstenite then fails
            // reads with WSAEWOULDBLOCK (os error 10035). Mirror the loop:
            // restore blocking before handing the stream to tungstenite.
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            listener.set_nonblocking(true).unwrap();
            let addr = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || loop {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false).expect("restore blocking");
                        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                        let mut ws = serve_connection(stream).expect("ws upgrade");
                        let msg = ws.read().expect("text frame on blocking stream");
                        let _ = ws.send(WsMessage::Text(format!("echo:{msg}").into()));
                        break;
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(10)),
                }
            });

            let (mut client, _) = tungstenite::connect(format!("ws://{addr}/")).unwrap();
            // Delay the frame so the server's first read runs while the
            // socket has no data — the exact window where a non-blocking
            // stream fails with WSAEWOULDBLOCK (os error 10035).
            std::thread::sleep(Duration::from_millis(300));
            client
                .send(WsMessage::Text("hello".to_owned().into()))
                .unwrap();
            let reply = client.read().expect("server echo");
            assert_eq!(reply, WsMessage::Text("echo:hello".to_owned().into()));
            server.join().unwrap();
        }

        #[test]
        fn availability_probe_gets_404_with_cors() {
            // Kee 2.x polls `GET /pingAvailabilityTest` over plain HTTP and
            // only attempts the WebSocket handshake when it gets a 404; a
            // different status makes it assume another service owns the port.
            let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
            let addr = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (stream, _) = listener.accept().unwrap();
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                // serve_connection must answer the probe and decline the
                // connection without touching tungstenite.
                assert!(serve_connection(stream).is_none());
            });

            let mut stream = TcpStream::connect(addr).unwrap();
            stream
                .write_all(
                    b"GET /pingAvailabilityTest HTTP/1.1\r\nHost: 127.0.0.1\r\nOrigin: moz-extension://test\r\nConnection: keep-alive\r\n\r\n",
                )
                .unwrap();
            let mut response = Vec::new();
            let mut one = [0u8; 1];
            while !response.ends_with(b"\r\n\r\n") && response.len() < 8192 {
                if stream.read(&mut one).unwrap() == 0 {
                    break;
                }
                response.push(one[0]);
            }
            // Close our end so the server's drain loop ends before join().
            let _ = stream.shutdown(std::net::Shutdown::Both);
            let text = String::from_utf8_lossy(&response);
            assert!(
                text.starts_with("HTTP/1.1 404"),
                "probe must answer 404, got: {text}"
            );
            assert!(
                text.to_lowercase()
                    .contains("access-control-allow-origin: *"),
                "probe response needs CORS headers, got: {text}"
            );
            server.join().unwrap();
        }

        #[test]
        fn serialized_replies_carry_keepassrpc_field_names() {
            // The extension reads `srp.securityLevel` / `key.securityLevel`
            // (camelCase) and `srp.B` / `srp.I` / `srp.A` / `srp.M` / `srp.M2`
            // (uppercase). The JSON on the wire must match exactly, or Kee
            // rejects the server as "security level too low" / aborts setup.
            let mut host = FakeHost::open();
            let (a_hex, _) = client_ephemeral();
            let mut conn = Conn::default();
            let reply = dispatch_setup(
                &mut conn,
                &identify_to_server("serialize@kprpc", &a_hex),
                &mut host,
                &mut |_, _| {},
            )
            .expect("identifyToClient reply");
            let json = serde_json::to_string(&reply).expect("reply serializes");
            eprintln!("identifyToClient wire: {json}");
            assert!(
                json.contains("\"securityLevel\":3"),
                "missing camelCase securityLevel: {json}"
            );
            assert!(
                json.contains("\"stage\":\"identifyToClient\""),
                "missing stage: {json}"
            );
            assert!(json.contains("\"B\""), "missing uppercase B: {json}");

            // proofToClient must also carry securityLevel: Kee 4.x checks it on
            // every srp message and rejects the server (AUTH_SERVER_SECURITY_
            // LEVEL_TOO_LOW) when it is missing or null.
            let mut conn = Conn::default();
            let (a_hex, a) = client_ephemeral();
            let mut shown: Option<String> = None;
            let reply = dispatch_setup(
                &mut conn,
                &identify_to_server("serialize@kprpc", &a_hex),
                &mut host,
                &mut |pw, _| shown = Some(pw.to_owned()),
            )
            .expect("identifyToClient reply");
            let s = reply.srp.as_ref().unwrap().s.clone().unwrap();
            let b = reply.srp.as_ref().unwrap().b.clone().unwrap();
            let (m, _) = client_proof(&s, &b, shown.as_ref().unwrap(), &a_hex, &a);
            let reply = dispatch_setup(&mut conn, &proof_to_server(&m), &mut host, &mut |_, _| {})
                .expect("proofToClient reply");
            let json = serde_json::to_string(&reply).expect("reply serializes");
            eprintln!("proofToClient wire: {json}");
            assert!(
                json.contains("\"securityLevel\":3"),
                "proofToClient must carry securityLevel: {json}"
            );
            assert!(json.contains("\"M2\""), "missing uppercase M2: {json}");

            let mut host = FakeHost::open();
            host.register_rpc_key("serialize@kprpc", vec![9u8; 32]);
            let mut conn = Conn::default();
            let reply = dispatch_setup(
                &mut conn,
                &key_request("serialize@kprpc"),
                &mut host,
                &mut |_, _| {},
            )
            .expect("sc challenge");
            let json = serde_json::to_string(&reply).expect("reply serializes");
            eprintln!("key.sc wire: {json}");
            assert!(
                json.contains("\"securityLevel\":3"),
                "missing camelCase securityLevel: {json}"
            );
            assert!(json.contains("\"sc\""), "missing sc: {json}");
        }
    }
}
