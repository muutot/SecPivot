//! KeePassRPC server tests: WS transport + pure handshake state machines.
//! Extracted from `rpc::server.rs`.
use super::*;
use crate::rpc::{
    decrypt_frame, encrypt_frame, hex, key_auth_cr, key_auth_sr, random_hex, Envelope,
    ErrorMessage, KeyMessage, RpcError, RpcHost, SrpMessage, SECURITY_LEVEL,
};
use num_bigint::{BigInt, BigUint, Sign};
use serde_json::{json, Value};
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
    let k =
        BigUint::parse_bytes("b7867f1299da8cc24ab93e08986ebc4d6a478ad0".as_bytes(), 16).unwrap();
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
    let k =
        BigUint::parse_bytes("b7867f1299da8cc24ab93e08986ebc4d6a478ad0".as_bytes(), 16).unwrap();
    let u = BigUint::parse_bytes(sha256(&format!("{a_hex}{b_hex}")).as_bytes(), 16).unwrap();
    let x = BigUint::parse_bytes(sha256(&format!("{salt}{password}")).as_bytes(), 16).unwrap();
    let kgx = (&k * g.modpow(&x, &n)) % &n;
    let aux = a + &u * &x;
    let base =
        BigInt::parse_bytes(b_hex.as_bytes(), 16).unwrap() - BigInt::from_biguint(Sign::Plus, kgx);
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
    crate::crypto::sha256_hex(input)
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
    let (reply, method) = dispatch_jsonrpc(&mut conn, &env, &mut host).expect("jsonrpc reply");
    assert_eq!(method, "GetAllDatabases");
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
