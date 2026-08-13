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

use crate::config::PasswordGeneratorSettings;
use crate::rpc::{entry_dto, parse_write_request, Envelope, RpcError, SrpServer, RPC_PORT};
use crate::vault::{persist_rpc_write, VaultSession, VaultSessions, REMOTE_CHANGED_MARKER};
use serde::Serialize;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tungstenite::{accept_hdr, Message as WsMessage, WebSocket};
use zeroize::Zeroize;

mod handshake;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use self::handshake::dispatch_jsonrpc;
pub(crate) use self::handshake::dispatch_setup;
use self::handshake::{
    decode_jsonrpc, dispatch_decoded_jsonrpc, encode_jsonrpc_result, DecodedJsonRpc,
};

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
/// Emitted after a browser-originated write (AddLogin/UpdateLogin) so the
/// desktop UI refreshes its entry list immediately.
pub(crate) const VAULT_CHANGED_EVENT: &str = "rpc-vault-changed";

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
    generator: Mutex<PasswordGeneratorSettings>,
}

impl RpcState {
    pub(crate) fn running(&self) -> bool {
        self.server.lock().ok().is_some_and(|guard| guard.is_some())
    }

    pub(crate) fn last_error(&self) -> Option<String> {
        self.last_error.lock().ok().and_then(|e| e.clone())
    }

    pub(crate) fn set_generator(&self, settings: PasswordGeneratorSettings) {
        if let Ok(mut guard) = self.generator.lock() {
            *guard = settings;
        }
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
pub(crate) struct Conn {
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
    match env.protocol.as_str() {
        "setup" => reply_setup(ws, conn, env, app),
        "jsonrpc" => reply_jsonrpc(ws, conn, env, app),
        // ping / pong / teardown / reconnect — KeePassRPC client keepalives.
        _ => true,
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
        // Same poison-guard as the bridge server: the guard lives outside the
        // unwind so a handler panic can't poison the session mutex.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            dispatch_setup(conn, &env, &mut *session, &mut |password, expires| {
                let _ = app.emit(
                    SIDE_CHANNEL_EVENT,
                    SideChannelRequest {
                        password: password.to_owned(),
                        expires_in_secs: expires,
                    },
                );
            })
        }));
        match outcome {
            Ok(reply) => reply,
            Err(_) => {
                eprintln!("[rpc] dispatch_setup panicked");
                None
            }
        }
    }) else {
        eprintln!(
            "[rpc] dispatch_setup returned None (srp={:?} key={:?} error={:?})",
            env.srp
                .as_ref()
                .map(|s| s.stage.clone().unwrap_or_default()),
            env.key.as_ref().map(|k| format!(
                "username_present={} sc={} cc={} cr={} sr={}",
                k.username.is_some(),
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
fn reply_jsonrpc(
    ws: &mut WebSocket<TcpStream>,
    conn: &mut Conn,
    env: Envelope,
    app: &AppHandle,
) -> bool {
    let Some(request) = decode_jsonrpc(conn, &env) else {
        return false;
    };
    if matches!(request.method.as_str(), "AddLogin" | "UpdateLogin") {
        return reply_jsonrpc_write(ws, conn, request, app);
    }
    let Some(session_state) = app.try_state::<Mutex<VaultSession>>() else {
        return false;
    };
    let Ok(mut session) = session_state.lock() else {
        return false;
    };
    let generator = app
        .state::<RpcState>()
        .generator
        .lock()
        .ok()
        .map(|guard| guard.clone())
        .unwrap_or_default();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        dispatch_decoded_jsonrpc(conn, request, &mut *session, &generator)
    }));
    match outcome {
        Ok(Some((reply, method))) => {
            if matches!(method.as_str(), "AddLogin" | "UpdateLogin") {
                // Writes mutate the vault in place; tell the desktop UI to
                // refresh so the new/edited entry shows up without a reopen.
                let _ = app.emit(VAULT_CHANGED_EVENT, ());
            }
            send_envelope(ws, &reply)
        }
        Ok(None) => false,
        Err(_) => {
            eprintln!("[rpc] dispatch_jsonrpc panicked");
            false
        }
    }
}

fn reply_jsonrpc_write(
    ws: &mut WebSocket<TcpStream>,
    conn: &Conn,
    request: DecodedJsonRpc,
    app: &AppHandle,
) -> bool {
    let Some(session_state) = app.try_state::<Mutex<VaultSession>>() else {
        return false;
    };
    let Some(vaults) = app.try_state::<VaultSessions>() else {
        return false;
    };
    let Some(session_id) = vaults.active_id() else {
        return encode_jsonrpc_result(conn, &request.id, Err(RpcError::Locked))
            .is_some_and(|reply| send_envelope(ws, &reply));
    };
    let _persistence = vaults.acquire_persistence();
    let job = {
        let Ok(mut active) = session_state.lock() else {
            return false;
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            vaults.with_session_mut_result(
                &mut active,
                &session_id,
                RpcError::InvalidMessage,
                |target| {
                    if !target.is_open() {
                        return Err(RpcError::Locked);
                    }
                    let parsed = parse_write_request(&request.method, request.params.as_ref())?
                        .ok_or_else(|| RpcError::Unsupported(request.method.clone()))?;
                    target.prepare_rpc_write(parsed)
                },
            )
        }));
        match outcome {
            Err(_) => {
                eprintln!("[rpc] prepare write panicked");
                return false;
            }
            Ok(Ok(job)) => job,
            Ok(Err(error)) => {
                return encode_jsonrpc_result(conn, &request.id, Err(error))
                    .is_some_and(|reply| send_envelope(ws, &reply));
            }
        }
    };
    let persisted =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| persist_rpc_write(job))) {
            Ok(result) => result,
            Err(_) => {
                eprintln!("[rpc] persist write panicked");
                return false;
            }
        };
    let result = match persisted {
        Ok(persisted) => {
            let (login, database) = persisted.persisted_response();
            let response = entry_dto(login, database);
            let Ok(mut active) = session_state.lock() else {
                return encode_jsonrpc_result(conn, &request.id, Ok(response))
                    .is_some_and(|reply| send_envelope(ws, &reply));
            };
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                vaults.with_session_mut_result(
                    &mut active,
                    &session_id,
                    RpcError::InvalidMessage,
                    |target| target.complete_rpc_write(persisted),
                )
            }));
            match outcome {
                Ok(Ok(_)) => Ok(response),
                Ok(Err(_)) => {
                    // Persistence is already durable. The originating tab may
                    // have closed while KDF/storage work ran; report success
                    // to Kee even when there is no live session left to adopt.
                    Ok(response)
                }
                Err(_) => {
                    eprintln!("[rpc] complete write panicked after persistence");
                    Ok(response)
                }
            }
        }
        Err(error) => {
            if !error.starts_with(REMOTE_CHANGED_MARKER) {
                if let Ok(mut active) = session_state.lock() {
                    let _ = vaults.with_session_mut(&mut active, Some(&session_id), |target| {
                        target.note_save_failure();
                        Ok(())
                    });
                }
            }
            Err(RpcError::InvalidMessage(format!("保存失败: {error}")))
        }
    };
    let success = result.is_ok();
    drop(_persistence);
    let sent = encode_jsonrpc_result(conn, &request.id, result)
        .is_some_and(|reply| send_envelope(ws, &reply));
    if success {
        let _ = app.emit(VAULT_CHANGED_EVENT, ());
    }
    sent
}
fn send_envelope(ws: &mut WebSocket<TcpStream>, env: &Envelope) -> bool {
    match serde_json::to_string(env) {
        Ok(json) => ws.send(WsMessage::Text(json.into())).is_ok(),
        Err(_) => false,
    }
}
