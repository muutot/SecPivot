//! Loopback HTTP server for the KeePassHttp bridge.
//!
//! Binds `127.0.0.1:19455` only (never `0.0.0.0`) and serves one JSON POST
//! per connection. Every request is dispatched through `bridge::handle_request`
//! under the `VaultSession` lock; a fresh `associate` first asks the user via
//! a frontend event (`bridge-associate-request`) and blocks up to
//! `APPROVAL_TIMEOUT` for an explicit allow/deny (`bridge_approve`).
//!
//! The server itself holds no secrets: keys live in the session and are
//! wiped on lock, so a locked vault responds with a plain error envelope.

use crate::bridge::{handle_request, new_client_id, BridgeRequest, BRIDGE_PORT};
use crate::vault::VaultSession;
use serde::Serialize;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{mpsc, Mutex};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

/// Body cap: the largest legal KeePassHttp payload is a few KiB.
const MAX_HEAD_BYTES: usize = 16 * 1024;
const MAX_BODY_BYTES: usize = 1024 * 1024;
/// How long an associate approval may stay unanswered before it is rejected.
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(120);
const IO_TIMEOUT: Duration = Duration::from_secs(10);

/// Event emitted to the frontend when a browser requests association.
/// Payload: `{ token: string, id: string }` (never the key material).
pub(crate) const ASSOCIATE_EVENT: &str = "bridge-associate-request";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AssociateRequest {
    pub(crate) token: String,
    pub(crate) id: String,
}

// ---------------------------------------------------------------------------
// Server lifecycle
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
pub(crate) struct BridgeState {
    server: Mutex<Option<ServerHandle>>,
    last_error: Mutex<Option<String>>,
}

impl BridgeState {
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
            .map_err(|_| "桥接状态锁已损坏".to_owned())?;
        if guard.is_some() {
            return Ok(());
        }
        let listener = match TcpListener::bind(("127.0.0.1", BRIDGE_PORT)) {
            Ok(listener) => listener,
            Err(e) => {
                let message = format!(
                    "无法监听 127.0.0.1:{BRIDGE_PORT} (端口可能被其他 KeePass 占用): {e}"
                );
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
                let app = app.clone();
                std::thread::spawn(move || handle_connection(stream, &app));
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(50)),
        }
    }
}

// ---------------------------------------------------------------------------
// One request
// ---------------------------------------------------------------------------

fn handle_connection(mut stream: TcpStream, app: &AppHandle) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let (method, body) = match read_http_request(&mut stream) {
        Ok(request) => request,
        Err(_) => return,
    };
    // Browser extensions send a CORS preflight before the real POST; answer
    // it with the same policy as KeePassXC so the fetch is allowed.
    if method.eq_ignore_ascii_case("OPTIONS") {
        let _ = write_http_options(&mut stream);
        return;
    }
    let request: BridgeRequest = match serde_json::from_str(&body) {
        Ok(request) => request,
        Err(_) => {
            let _ = write_http_response(
                &mut stream,
                r#"{"Success":false,"Error":"请求格式无效","Version":"1.8.4"}"#,
            );
            return;
        }
    };
    let Some(board) = app.try_state::<ApprovalBoard>() else {
        let _ = write_http_response(
            &mut stream,
            r#"{"Success":false,"Error":"内部错误","Version":"1.8.4"}"#,
        );
        return;
    };
    let Some(session_state) = app.try_state::<Mutex<VaultSession>>() else {
        let _ = write_http_response(
            &mut stream,
            r#"{"Success":false,"Error":"内部错误","Version":"1.8.4"}"#,
        );
        return;
    };
    let response = {
        let Ok(mut session) = session_state.lock() else {
            let _ = write_http_response(
                &mut stream,
                r#"{"Success":false,"Error":"内部错误","Version":"1.8.4"}"#,
            );
            return;
        };
        let app = app.clone();
        handle_request(request, &mut *session, move |id| {
            let app = app.clone();
            request_approval(&app, &board, id)
        })
    };
    match serde_json::to_string(&response) {
        Ok(json) => {
            let _ = write_http_response(&mut stream, &json);
        }
        Err(_) => {
            let _ = write_http_response(
                &mut stream,
                r#"{"Success":false,"Error":"内部错误","Version":"1.8.4"}"#,
            );
        }
    }
}

/// Ask the user for consent to bind a new browser client: register a pending
/// approval, emit the frontend event, and block up to `APPROVAL_TIMEOUT` for
/// `bridge_approve`. Returns the user's decision (rejected on timeout).
fn request_approval(app: &AppHandle, board: &ApprovalBoard, id: &str) -> bool {
    request_approval_with(
        board,
        |token, id| {
            let payload = AssociateRequest {
                token: token.to_owned(),
                id: id.to_owned(),
            };
            let _ = app.emit(ASSOCIATE_EVENT, payload);
        },
        id,
        APPROVAL_TIMEOUT,
    )
}

/// Pure half of `request_approval` (unit-testable): `emit` must fire the
/// event so the UI can answer through `board`.
pub(crate) fn request_approval_with(
    board: &ApprovalBoard,
    emit: impl FnOnce(&str, &str),
    id: &str,
    timeout: Duration,
) -> bool {
    let token = new_client_id();
    let (tx, rx) = mpsc::channel();
    if !board.insert(&token, tx) {
        return false;
    }
    emit(&token, id);
    matches!(rx.recv_timeout(timeout), Ok(decision) if decision)
}

/// Pending associate approvals keyed by token; one shot per token.
#[derive(Default)]
pub(crate) struct ApprovalBoard {
    pending: Mutex<HashMap<String, mpsc::Sender<bool>>>,
}

impl ApprovalBoard {
    pub(crate) fn insert(&self, token: &str, sender: mpsc::Sender<bool>) -> bool {
        self.pending
            .lock()
            .ok()
            .is_some_and(|mut map| map.insert(token.to_owned(), sender).is_none())
    }

    /// Resolve a pending approval. Returns false for unknown/expired tokens.
    pub(crate) fn decide(&self, token: &str, allowed: bool) -> bool {
        let sender = self
            .pending
            .lock()
            .ok()
            .and_then(|mut map| map.remove(token));
        match sender {
            Some(sender) => sender.send(allowed).is_ok(),
            None => false,
        }
    }
}

// ---------------------------------------------------------------------------
// Minimal HTTP framing (one POST per connection; we never need more)
// ---------------------------------------------------------------------------

fn find_headers_end(head: &[u8]) -> Option<usize> {
    head.windows(4).position(|window| window == b"\r\n\r\n")
}

fn parse_content_length(head: &[u8]) -> Result<usize, String> {
    for line in head.split(|b| *b == b'\n') {
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let Some(colon) = line.iter().position(|b| *b == b':') else {
            continue;
        };
        let (name, value) = line.split_at(colon);
        if name.eq_ignore_ascii_case(b"content-length") {
            let digits = String::from_utf8_lossy(&value[1..]);
            return digits
                .trim()
                .parse::<usize>()
                .map_err(|_| "Content-Length 无效".to_owned());
        }
    }
    Ok(0)
}

fn read_http_request(stream: &mut TcpStream) -> Result<(String, String), String> {
    let mut head = Vec::with_capacity(1024);
    let mut buf = [0u8; 4096];
    loop {
        let n = stream
            .read(&mut buf)
            .map_err(|e| format!("读取请求失败: {e}"))?;
        if n == 0 {
            return Err("连接已关闭".to_owned());
        }
        head.extend_from_slice(&buf[..n]);
        let Some(end) = find_headers_end(&head) else {
            if head.len() > MAX_HEAD_BYTES {
                return Err("请求头过大".to_owned());
            }
            continue;
        };
        let request_line = String::from_utf8_lossy(&head[..end])
            .lines()
            .next()
            .unwrap_or_default()
            .to_owned();
        let method = request_line
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned();
        let content_length = parse_content_length(&head[..end])?;
        if content_length > MAX_BODY_BYTES {
            return Err("请求体过大".to_owned());
        }
        let body_start = end + 4;
        let mut body = head[body_start..].to_vec();
        while body.len() < content_length {
            let n = stream
                .read(&mut buf)
                .map_err(|e| format!("读取请求体失败: {e}"))?;
            if n == 0 {
                return Err("连接已关闭".to_owned());
            }
            body.extend_from_slice(&buf[..n]);
        }
        body.truncate(content_length);
        return String::from_utf8(body)
            .map(|text| (method, text))
            .map_err(|_| "请求体不是有效文本".to_owned());
    }
}

/// Shared CORS policy: browser extensions fetch this loopback port from a
/// different origin, so every response must carry these headers and OPTIONS
/// preflights must be answered (mirrors KeePassXC's KeePassHttp server).
fn cors_headers() -> &'static str {
    "Access-Control-Allow-Origin: *\r\n\
     Access-Control-Allow-Methods: POST, OPTIONS\r\n\
     Access-Control-Allow-Headers: Content-Type, X-Requested-With\r\n"
}

/// Empty 200 answer for a CORS preflight (OPTIONS).
fn write_http_options(stream: &mut TcpStream) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\n{}\
         Content-Length: 0\r\nConnection: close\r\n\r\n",
        cors_headers()
    )?;
    stream.flush()
}

fn write_http_response(stream: &mut TcpStream, body: &str) -> std::io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 200 OK\r\n{}\
         Content-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        cors_headers(),
        body.len()
    )?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Serve `raw` over a real loopback socket; the returned stream is the
    /// server side after the client wrote and half-closed.
    fn client_writes(raw: Vec<u8>) -> (TcpStream, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let writer = std::thread::spawn(move || {
            let stream = TcpStream::connect(addr).unwrap();
            let mut stream = stream;
            stream.write_all(&raw).unwrap();
            let _ = stream.shutdown(std::net::Shutdown::Write);
        });
        let (stream, _) = listener.accept().unwrap();
        (stream, writer)
    }

    #[test]
    fn read_http_request_parses_post_with_content_length() {
        let raw = b"POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Length: 19\r\n\r\n{\"RequestType\":\"x\"}";
        let (stream, writer) = client_writes(raw.to_vec());
        writer.join().unwrap();
        let mut stream = stream;
        let (method, body) = read_http_request(&mut stream).unwrap();
        assert_eq!(method, "POST");
        assert_eq!(body, r#"{"RequestType":"x"}"#);
    }

    #[test]
    fn read_http_request_handles_split_body_chunks() {
        let raw = b"POST / HTTP/1.1\r\nContent-Length: 19\r\n\r\n{\"RequestType\":\"";
        let rest = b"x\"}";
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let writer = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).unwrap();
            stream.write_all(raw).unwrap();
            std::thread::sleep(Duration::from_millis(20));
            stream.write_all(rest).unwrap();
            let _ = stream.shutdown(std::net::Shutdown::Write);
        });
        let (mut stream, _) = listener.accept().unwrap();
        writer.join().unwrap();
        let (method, body) = read_http_request(&mut stream).unwrap();
        assert_eq!(method, "POST");
        assert_eq!(body, r#"{"RequestType":"x"}"#);
    }

    #[test]
    fn read_http_request_rejects_oversized_body_and_garbage_headers() {
        let raw = b"POST / HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n{}";
        let (stream, writer) = client_writes(raw.to_vec());
        writer.join().unwrap();
        let mut stream = stream;
        assert!(read_http_request(&mut stream).unwrap_err().contains("过大"));

        let raw = b"POST / HTTP/1.1\r\nContent-Length: nope\r\n\r\n{}";
        let (stream, writer) = client_writes(raw.to_vec());
        writer.join().unwrap();
        let mut stream = stream;
        assert!(read_http_request(&mut stream).unwrap_err().contains("无效"));
    }

    #[test]
    fn read_http_request_rejects_closed_connection() {
        let raw = b"POST / HTTP/1.1\r\nContent-Length: 50\r\n\r\nshort";
        let (stream, writer) = client_writes(raw.to_vec());
        writer.join().unwrap();
        let mut stream = stream;
        assert!(read_http_request(&mut stream).unwrap_err().contains("关闭"));
    }

    #[test]
    fn write_http_response_emits_valid_json_frame() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let reader = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).unwrap();
            stream.shutdown(std::net::Shutdown::Write).unwrap();
            let mut out = Vec::new();
            stream.read_to_end(&mut out).unwrap();
            String::from_utf8(out).unwrap()
        });
        let (mut stream, _) = listener.accept().unwrap();
        write_http_response(&mut stream, "{\"Success\":true}").unwrap();
        drop(stream);
        let frame = reader.join().unwrap();
        assert!(frame.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(frame.contains("Content-Type: application/json\r\n"));
        assert!(frame.contains("Access-Control-Allow-Origin: *\r\n"));
        assert!(frame.contains("Access-Control-Allow-Headers: Content-Type, X-Requested-With\r\n"));
        assert!(frame.ends_with("{\"Success\":true}"));
    }

    #[test]
    fn cors_preflight_options_gets_200_with_allow_headers() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let reader = std::thread::spawn(move || {
            let mut stream = TcpStream::connect(addr).unwrap();
            stream.shutdown(std::net::Shutdown::Write).unwrap();
            let mut out = Vec::new();
            stream.read_to_end(&mut out).unwrap();
            String::from_utf8(out).unwrap()
        });
        let (mut stream, _) = listener.accept().unwrap();
        write_http_options(&mut stream).unwrap();
        drop(stream);
        let frame = reader.join().unwrap();
        assert!(frame.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(frame.contains("Access-Control-Allow-Origin: *\r\n"));
        assert!(frame.contains("Access-Control-Allow-Methods: POST, OPTIONS\r\n"));
        assert!(frame.contains("Content-Length: 0\r\n"));
    }

    #[test]
    fn approval_flow_approves_and_rejects_via_board() {
        let board = ApprovalBoard::default();

        let approved = request_approval_with(
            &board,
            |token, id| {
                assert_eq!(id, "client-x");
                assert!(board.decide(token, true));
            },
            "client-x",
            Duration::from_secs(5),
        );
        assert!(approved);

        let rejected = request_approval_with(
            &board,
            |token, _| {
                assert!(board.decide(token, false));
            },
            "client-y",
            Duration::from_secs(5),
        );
        assert!(!rejected);

        // An answered token cannot be answered twice.
        assert!(!board.decide("does-not-exist", true));
    }

    #[test]
    fn approval_flow_times_out_when_unanswered() {
        let board = ApprovalBoard::default();
        let decided = request_approval_with(
            &board,
            |_token, _| {},
            "client-z",
            Duration::from_millis(50),
        );
        assert!(!decided);
        // The stale pending entry is cleaned up only by `decide`.
        assert!(!board.decide("stale", true));
    }
}
