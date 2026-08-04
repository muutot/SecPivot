//! KeePassHttp-compatible browser bridge: protocol core (pure, testable).
//!
//! Wire semantics follow KeePassHttp (chromeIPass / KeePassHelper):
//! - Every request carries `Nonce` (base64 of the 16-byte IV) and `Verifier`
//!   = AES-256-CBC(key, iv = nonce bytes) of the base64 string of those nonce
//!   bytes (matching `Request::CheckVerifier` in the reference implementation).
//! - Sensitive fields are AES-256-CBC encrypted with the request IV; responses
//!   use a fresh IV (their own `Nonce`) for all encrypted fields.
//! - `Hmac` = HMAC-SHA256(key, nonce_bytes || verifier_ciphertext_bytes).
//!
//! The crypto is a shared secret by protocol design — it is *not* authenticated
//! encryption — so the server must stay loopback-only and every new client key
//! requires explicit user approval (see `handle_request`'s `approve` gate).
//! Secrets are wiped in place before any local key copy is dropped.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::crypto::{aes_cbc_decrypt_b64, aes_cbc_encrypt_b64, hmac_sha256, random_bytes};

/// KeePassHttp default loopback port (hard-coded into chromeIPass).
pub const BRIDGE_PORT: u16 = 19455;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 16;
const PROTOCOL_VERSION: &str = "1.8.4";

// ---------------------------------------------------------------------------
// Crypto primitives (shared helpers live in `crate::crypto`)
// ---------------------------------------------------------------------------

/// AES-256-CBC encrypt `plaintext` with PKCS7 padding, base64-encoded.
/// `key` must be 32 bytes and `iv` 16 bytes (the request/response nonce).
pub fn encrypt_field(key: &[u8], iv: &[u8], plaintext: &str) -> String {
    aes_cbc_encrypt_b64(key, iv, plaintext)
}

/// AES-256-CBC decrypt with PKCS7 padding; base64 on the wire.
pub fn decrypt_field(key: &[u8], iv: &[u8], encoded: &str) -> Result<String, String> {
    aes_cbc_decrypt_b64(key, iv, encoded)
}

/// KeePassHttp verifier: AES-256-CBC of the base64 string of `nonce` bytes,
/// using those same bytes as the IV.
pub fn make_verifier(key: &[u8], nonce: &[u8]) -> String {
    encrypt_field(key, nonce, &STANDARD.encode(nonce))
}

/// Check a request verifier the way chromeIPass/KeePassHelper do: decrypt with
/// the request nonce and compare against `base64(nonce)`.
pub fn check_verifier(key: &[u8], nonce_b64: &str, verifier_b64: &str) -> bool {
    let Ok(nonce) = STANDARD.decode(nonce_b64) else {
        return false;
    };
    if nonce.len() != NONCE_LEN {
        return false;
    }
    decrypt_field(key, &nonce, verifier_b64).is_ok_and(|plain| plain == STANDARD.encode(&nonce))
}

/// Response `Hmac`: HMAC-SHA256 over (nonce bytes || verifier ciphertext),
/// matching the reference implementation's `Response::finalize`.
pub fn response_hmac(key: &[u8], nonce_b64: &str, verifier_b64: &str) -> String {
    let mut data = Vec::with_capacity(64);
    data.extend_from_slice(&STANDARD.decode(nonce_b64).unwrap_or_default());
    data.extend_from_slice(&STANDARD.decode(verifier_b64).unwrap_or_default());
    STANDARD.encode(hmac_sha256(key, &data))
}

// ---------------------------------------------------------------------------
// Wire shapes (exact KeePassHttp field names, camelCase)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct BridgeRequest {
    pub request_type: String,
    pub id: Option<String>,
    pub nonce: String,
    pub verifier: Option<String>,
    /// Associate-only: the 32-byte AES key chosen by the browser extension.
    pub key: Option<String>,
    pub url: Option<String>,
    pub submit_url: Option<String>,
    pub login: Option<String>,
    pub password: Option<String>,
    pub uuid: Option<String>,
    #[allow(dead_code)]
    pub trigger_unlock: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeEntry {
    pub login: String,
    pub password: String,
    pub name: String,
    pub uuid: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeResponse {
    pub request_type: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<BridgeEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    /// `generate-password` only: the fresh plaintext password (never a vault
    /// secret; the browser needs it to fill the form).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    pub nonce: String,
    pub verifier: String,
    pub hash: String,
    pub version: String,
    pub hmac: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl BridgeResponse {
    /// Error response for requests that carry no usable key material
    /// (locked vault, unknown client, failed verifier).
    fn failure(request_type: &str, error: &str) -> Self {
        Self {
            request_type: request_type.to_owned(),
            success: false,
            id: None,
            entries: Vec::new(),
            count: None,
            password: None,
            nonce: String::new(),
            verifier: String::new(),
            hash: String::new(),
            version: PROTOCOL_VERSION.to_owned(),
            hmac: String::new(),
            error: Some(error.to_owned()),
        }
    }

    /// Success envelope: fresh nonce, verifier and hmac under `key`.
    fn success(request_type: &str, key: &[u8], host: &dyn BridgeHost) -> Self {
        let nonce = random_bytes(NONCE_LEN);
        let nonce_b64 = STANDARD.encode(&nonce);
        let verifier_b64 = make_verifier(key, &nonce);
        Self {
            request_type: request_type.to_owned(),
            success: true,
            id: None,
            entries: Vec::new(),
            count: None,
            password: None,
            nonce: nonce_b64.clone(),
            verifier: verifier_b64.clone(),
            hash: host.db_hash(),
            version: PROTOCOL_VERSION.to_owned(),
            hmac: response_hmac(key, &nonce_b64, &verifier_b64),
            error: None,
        }
    }
}

/// Decrypt one optional request field under the request IV (the nonce bytes).
fn decrypt_request_field(
    key: &[u8],
    nonce_b64: &str,
    field: Option<&str>,
) -> Result<Option<String>, String> {
    let Some(encoded) = field else {
        return Ok(None);
    };
    let iv = STANDARD
        .decode(nonce_b64)
        .map_err(|_| "Nonce 格式无效".to_owned())?;
    if iv.len() != NONCE_LEN {
        return Err("Nonce 长度无效".to_owned());
    }
    decrypt_field(key, &iv, encoded).map(Some)
}

// ---------------------------------------------------------------------------
// Host abstraction + dispatch
// ---------------------------------------------------------------------------

/// One credential the bridge hands back to the browser. Plaintext lives only
/// on the stack of the request handler and is encrypted before it leaves.
#[derive(Debug, Clone)]
pub struct BridgeLogin {
    pub uuid: String,
    pub name: String,
    pub login: String,
    pub password: String,
}

/// The vault-session subset the bridge needs. Implemented by `VaultSession`
/// (vault.rs) so dispatch stays independent of vault internals; the cycle-free
/// direction keeps the two modules decoupled.
pub trait BridgeHost {
    fn is_open(&self) -> bool;
    /// Copy of the AES key for `id` (zeroized by the caller after use), or
    /// `None` when the client is unknown or the vault is locked.
    fn client_key(&self, id: &str) -> Option<Vec<u8>>;
    fn register_client(&mut self, id: &str, key: Vec<u8>);
    fn list_clients(&self) -> Vec<String>;
    fn remove_client(&mut self, id: &str) -> bool;
    fn logins_for(&self, url: &str, submit_url: Option<&str>) -> Vec<BridgeLogin>;
    fn db_hash(&self) -> String;
    /// Update the entry referenced by `uuid` (must already exist).
    fn set_login(
        &mut self,
        login: &str,
        password: &str,
        url: &str,
        uuid: Option<&str>,
    ) -> Result<(), String>;
    /// Create a new entry with the given credentials.
    fn create_login(&mut self, login: &str, password: &str, url: &str) -> Result<(), String>;
}

/// Handle one KeePassHttp request against `host`. `approve` is invoked once
/// for a fresh `associate` (the user's explicit consent in the desktop UI);
/// returning `false` rejects the client.
pub fn handle_request(
    request: BridgeRequest,
    host: &mut dyn BridgeHost,
    approve: impl FnOnce(&str) -> bool,
) -> BridgeResponse {
    let request_type = request.request_type.clone();
    if request_type.trim().is_empty() {
        return BridgeResponse::failure(&request_type, "缺少 RequestType");
    }
    if !host.is_open() {
        return BridgeResponse::failure(&request_type, "数据库未打开或已锁定");
    }
    if request_type == "associate" {
        return handle_associate(request, host, approve);
    }

    let id = request.id.clone().unwrap_or_default();
    let Some(mut key) = host.client_key(&id) else {
        return BridgeResponse::failure(&request_type, "未授权的浏览器客户端,请在浏览器中重新关联");
    };
    let valid = check_verifier(
        &key,
        &request.nonce,
        request.verifier.as_deref().unwrap_or_default(),
    );
    if !valid {
        key.zeroize();
        return BridgeResponse::failure(&request_type, "请求校验失败");
    }
    let response = dispatch(request_type.as_str(), &request, &key, id.as_str(), host);
    key.zeroize();
    response
}

/// `associate` adds a new client key after explicit user approval; the key is
/// then bound to `id` inside the (session-held) host state.
fn handle_associate(
    request: BridgeRequest,
    host: &mut dyn BridgeHost,
    approve: impl FnOnce(&str) -> bool,
) -> BridgeResponse {
    let request_type = request.request_type.clone();
    let Some(key_b64) = request.key.as_deref() else {
        return BridgeResponse::failure(&request_type, "关联请求缺少 Key");
    };
    let mut key = match STANDARD.decode(key_b64) {
        Ok(bytes) if bytes.len() == KEY_LEN => bytes,
        _ => return BridgeResponse::failure(&request_type, "关联密钥必须是 256 位"),
    };
    let valid = check_verifier(
        &key,
        &request.nonce,
        request.verifier.as_deref().unwrap_or_default(),
    );
    if !valid {
        key.zeroize();
        return BridgeResponse::failure(&request_type, "关联校验失败");
    }
    let id = request.id.clone().unwrap_or_else(new_client_id);
    if !approve(&id) {
        key.zeroize();
        return BridgeResponse::failure(&request_type, "已拒绝浏览器连接授权");
    }
    host.register_client(&id, key);
    // Echo the bound id under the stored key (fresh response nonce).
    let mut key = host.client_key(&id).unwrap_or_default();
    let nonce = random_bytes(NONCE_LEN);
    let nonce_b64 = STANDARD.encode(&nonce);
    let verifier_b64 = make_verifier(&key, &nonce);
    let response = BridgeResponse {
        request_type,
        success: true,
        id: Some(id),
        entries: Vec::new(),
        count: None,
        password: None,
        nonce: nonce_b64.clone(),
        verifier: verifier_b64.clone(),
        hash: host.db_hash(),
        version: PROTOCOL_VERSION.to_owned(),
        hmac: response_hmac(&key, &nonce_b64, &verifier_b64),
        error: None,
    };
    key.zeroize();
    response
}

/// Random client/approval token (base64 of 12 entropy bytes).
pub fn new_client_id() -> String {
    STANDARD.encode(random_bytes(12))
}

/// Decrypt-then-serve for the authorized request types. Runs only after the
/// verifier passed, so `key` is the shared client secret.
fn dispatch(
    request_type: &str,
    request: &BridgeRequest,
    key: &[u8],
    id: &str,
    host: &mut dyn BridgeHost,
) -> BridgeResponse {
    match request_type {
        "test-associate" => {
            let mut response = BridgeResponse::success(request_type, key, host);
            response.id = Some(id.to_owned());
            response
        }
        "get-logins" | "get-logins-count" => {
            let (url, submit_url) = match decrypt_request_fields(request, key) {
                Ok(fields) => fields,
                Err(e) => return BridgeResponse::failure(request_type, &e),
            };
            let logins = host.logins_for(url.as_deref().unwrap_or_default(), submit_url.as_deref());
            let mut response = BridgeResponse::success(request_type, key, host);
            if !logins.is_empty() {
                let iv = STANDARD
                    .decode(&response.nonce)
                    .expect("fresh response nonce is valid base64");
                response.entries = build_entries(&logins, key, &iv);
            }
            if request_type == "get-logins-count" {
                response.count = Some(logins.len());
            }
            response
        }
        "set-login" => {
            let fields = match decrypt_set_login_fields(request, key) {
                Ok(fields) => fields,
                Err(e) => return BridgeResponse::failure(request_type, &e),
            };
            let result = match fields.uuid.as_deref() {
                Some(_) => host.set_login(
                    fields.login.as_deref().unwrap_or_default(),
                    fields.password.as_deref().unwrap_or_default(),
                    fields.url.as_deref().unwrap_or_default(),
                    fields.uuid.as_deref(),
                ),
                None => host.create_login(
                    fields.login.as_deref().unwrap_or_default(),
                    fields.password.as_deref().unwrap_or_default(),
                    fields.url.as_deref().unwrap_or_default(),
                ),
            };
            match result {
                Ok(()) => {
                    let mut response = BridgeResponse::success(request_type, key, host);
                    response.count = Some(1);
                    response
                }
                Err(e) => BridgeResponse::failure(request_type, &e),
            }
        }
        "generate-password" => {
            let mut response = BridgeResponse::success(request_type, key, host);
            response.password = Some(generate_password());
            response
        }
        other => BridgeResponse::failure(other, &format!("不支持的操作: {other}")),
    }
}

fn build_entries(logins: &[BridgeLogin], key: &[u8], iv: &[u8]) -> Vec<BridgeEntry> {
    logins
        .iter()
        .map(|login| BridgeEntry {
            login: encrypt_field(key, iv, &login.login),
            password: encrypt_field(key, iv, &login.password),
            name: encrypt_field(key, iv, &login.name),
            uuid: encrypt_field(key, iv, &login.uuid),
        })
        .collect()
}

fn decrypt_request_fields(
    request: &BridgeRequest,
    key: &[u8],
) -> Result<(Option<String>, Option<String>), String> {
    let url = decrypt_request_field(key, &request.nonce, request.url.as_deref())?;
    let submit_url = decrypt_request_field(key, &request.nonce, request.submit_url.as_deref())?;
    Ok((url, submit_url))
}

struct SetLoginFields {
    login: Option<String>,
    password: Option<String>,
    url: Option<String>,
    uuid: Option<String>,
}

fn decrypt_set_login_fields(request: &BridgeRequest, key: &[u8]) -> Result<SetLoginFields, String> {
    let login = decrypt_request_field(key, &request.nonce, request.login.as_deref())?;
    let password = decrypt_request_field(key, &request.nonce, request.password.as_deref())?;
    let url = decrypt_request_field(key, &request.nonce, request.url.as_deref())?;
    let uuid = decrypt_request_field(key, &request.nonce, request.uuid.as_deref())?;
    Ok(SetLoginFields {
        login,
        password,
        url,
        uuid,
    })
}

/// Random index in `0..bound` from the OS RNG.
fn rand_index(bound: usize) -> usize {
    let mut buf = [0u8; 4];
    getrandom::getrandom(&mut buf).expect("OS RNG must be available");
    u32::from_le_bytes(buf) as usize % bound
}

/// Fresh 20-char password over upper/lower/digits/symbols, one character from
/// each category guaranteed — mirrors the app's default generator settings
/// (`DEFAULT_DATABASE_SETTINGS.generator`). Shared with the KeePassRPC bridge.
pub(crate) fn generate_password() -> String {
    const LEN: usize = 20;
    const UPPER: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const LOWER: &str = "abcdefghijklmnopqrstuvwxyz";
    const DIGITS: &str = "0123456789";
    const SYMBOLS: &str = "!@#$%^&*()-_=+[]{};:,.<>?";
    let pool: Vec<char> = format!("{UPPER}{LOWER}{DIGITS}{SYMBOLS}").chars().collect();
    let mut out: Vec<char> = (0..LEN).map(|_| pool[rand_index(pool.len())]).collect();
    for category in [UPPER, LOWER, DIGITS, SYMBOLS] {
        if !out.iter().any(|c| category.contains(*c)) {
            let pos = rand_index(out.len());
            let category_len = category.chars().count();
            out[pos] = category
                .chars()
                .nth(rand_index(category_len))
                .expect("category is non-empty");
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    // ---------------------------------------------------------------------
    // Crypto vectors
    // ---------------------------------------------------------------------

    /// NIST SP 800-38A CBC-AES-256 vector: encrypting the first plaintext
    /// block must yield the reference ciphertext, plus a full PKCS7 padding
    /// block (0x10 × 16) appended by the padded API.
    #[test]
    fn aes256_cbc_matches_nist_vector() {
        let key = hex_bytes("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
        let iv = hex_bytes("000102030405060708090a0b0c0d0e0f");
        let plaintext = hex_bytes("6bc1bee22e409f96e93d7e117393172a");
        let expected_ct = hex_bytes("f58c4c04d6e5f1ba779eabfb5f7bfbd6");

        let ciphertext = crate::crypto::aes_cbc_encrypt(&key, &iv, &plaintext);
        assert_eq!(&ciphertext[..16], expected_ct.as_slice());
        // PKCS7 appends one full padding block (0x10 × 16); the padded API
        // strips it again on decrypt, so the round trip is the plaintext.
        assert_eq!(ciphertext.len(), 32);
        let round_trip = crate::crypto::aes_cbc_decrypt(&key, &iv, &ciphertext).unwrap();
        assert_eq!(round_trip, plaintext);
    }

    #[test]
    fn encrypt_decrypt_round_trips_utf8_and_empty() {
        let key = [7u8; 32];
        let iv = [9u8; 16];
        let encoded = encrypt_field(&key, &iv, "用户@示例.com/密码🔐");
        assert_eq!(
            decrypt_field(&key, &iv, &encoded).unwrap(),
            "用户@示例.com/密码🔐"
        );

        let empty = encrypt_field(&key, &iv, "");
        assert_eq!(decrypt_field(&key, &iv, &empty).unwrap(), "");
    }

    #[test]
    fn decrypt_rejects_wrong_key_or_tampered_text() {
        let key = [1u8; 32];
        let iv = [2u8; 16];
        let encoded = encrypt_field(&key, &iv, "secret");
        assert!(decrypt_field(&[0u8; 32], &iv, &encoded).is_err());

        let mut tampered = STANDARD.decode(&encoded).unwrap();
        tampered[0] ^= 0xff;
        let tampered_b64 = STANDARD.encode(&tampered);
        assert!(decrypt_field(&key, &iv, &tampered_b64).is_err());
        assert!(decrypt_field(&key, &iv, "not-base64!!").is_err());
    }

    /// RFC 4231 HMAC-SHA256 test case 2.
    #[test]
    fn hmac_matches_rfc4231_vector() {
        let key = b"Jefe";
        let data = b"what do ya want for nothing?";
        let expected =
            hex_bytes("5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843");
        assert_eq!(hmac_sha256(key, data), expected);
    }

    #[test]
    fn verifier_round_trip_and_checks() {
        let key = [3u8; 32];
        let nonce = random_bytes(16);
        let verifier = make_verifier(&key, &nonce);
        let nonce_b64 = STANDARD.encode(&nonce);

        assert!(check_verifier(&key, &nonce_b64, &verifier));
        assert!(!check_verifier(&[4u8; 32], &nonce_b64, &verifier));
        assert!(!check_verifier(&key, &nonce_b64, "AAAA"));
        assert!(!check_verifier(&key, "AAAA", &verifier));
        assert!(!check_verifier(&key, &STANDARD.encode([0u8; 8]), &verifier));
    }

    #[test]
    fn response_hmac_is_stable_and_changes_with_key() {
        let key = [5u8; 32];
        let other = [6u8; 32];
        let nonce_b64 = STANDARD.encode([7u8; 16]);
        let verifier_b64 = STANDARD.encode([8u8; 32]);
        let hmac = response_hmac(&key, &nonce_b64, &verifier_b64);
        assert_eq!(hmac, response_hmac(&key, &nonce_b64, &verifier_b64));
        assert_ne!(hmac, response_hmac(&other, &nonce_b64, &verifier_b64));
        // 32-byte HMAC-SHA256 digest
        assert_eq!(STANDARD.decode(&hmac).unwrap().len(), 32);
    }

    // ---------------------------------------------------------------------
    // Dispatch with a mock host
    // ---------------------------------------------------------------------

    struct MockHost {
        open: bool,
        clients: HashMap<String, Vec<u8>>,
        logins: Vec<BridgeLogin>,
        created: Vec<(String, String, String)>,
        updated: Vec<(String, String, String, String)>,
    }

    impl MockHost {
        fn open() -> Self {
            let key = [0x11; 32];
            let mut clients = HashMap::new();
            clients.insert("client-1".to_owned(), key.to_vec());
            Self {
                open: true,
                clients,
                logins: vec![BridgeLogin {
                    uuid: "uuid-1".to_owned(),
                    name: "示例站点".to_owned(),
                    login: "user1".to_owned(),
                    password: "pw-1".to_owned(),
                }],
                created: Vec::new(),
                updated: Vec::new(),
            }
        }
    }

    impl BridgeHost for MockHost {
        fn is_open(&self) -> bool {
            self.open
        }
        fn client_key(&self, id: &str) -> Option<Vec<u8>> {
            self.clients.get(id).cloned()
        }
        fn register_client(&mut self, id: &str, key: Vec<u8>) {
            self.clients.insert(id.to_owned(), key);
        }
        fn list_clients(&self) -> Vec<String> {
            self.clients.keys().cloned().collect()
        }
        fn remove_client(&mut self, id: &str) -> bool {
            self.clients.remove(id).is_some()
        }
        fn logins_for(&self, url: &str, submit_url: Option<&str>) -> Vec<BridgeLogin> {
            if url.contains("example.com") || submit_url.is_some_and(|s| s.contains("example.com"))
            {
                self.logins.clone()
            } else {
                Vec::new()
            }
        }
        fn db_hash(&self) -> String {
            "abc123".to_owned()
        }
        fn set_login(
            &mut self,
            login: &str,
            password: &str,
            url: &str,
            uuid: Option<&str>,
        ) -> Result<(), String> {
            self.updated.push((
                uuid.unwrap_or_default().to_owned(),
                login.to_owned(),
                password.to_owned(),
                url.to_owned(),
            ));
            Ok(())
        }
        fn create_login(&mut self, login: &str, password: &str, url: &str) -> Result<(), String> {
            self.created
                .push((login.to_owned(), password.to_owned(), url.to_owned()));
            Ok(())
        }
    }

    fn field(value: &str, key: &[u8], iv: &[u8]) -> String {
        encrypt_field(key, iv, value)
    }

    fn authorized_request(request_type: &str) -> BridgeRequest {
        let key = [0x11; 32];
        let nonce = random_bytes(16);
        let nonce_b64 = STANDARD.encode(&nonce);
        let verifier = make_verifier(&key, &nonce);
        BridgeRequest {
            request_type: request_type.to_owned(),
            id: Some("client-1".to_owned()),
            nonce: nonce_b64,
            verifier: Some(verifier),
            ..Default::default()
        }
    }

    #[test]
    fn locked_vault_answers_error_without_keys() {
        let mut host = MockHost::open();
        host.open = false;
        let response = handle_request(authorized_request("get-logins"), &mut host, |_| true);
        assert!(!response.success);
        assert_eq!(response.error.as_deref(), Some("数据库未打开或已锁定"));
        assert!(response.verifier.is_empty());
    }

    #[test]
    fn unknown_client_is_rejected_before_dispatch() {
        let mut request = authorized_request("get-logins");
        request.id = Some("ghost".to_owned());
        let mut host = MockHost::open();
        let response = handle_request(request, &mut host, |_| true);
        assert!(!response.success);
        assert!(response.error.unwrap().contains("未授权"));
    }

    #[test]
    fn tampered_verifier_is_rejected() {
        let mut request = authorized_request("get-logins");
        request.verifier = Some(STANDARD.encode([0u8; 32]));
        let mut host = MockHost::open();
        let response = handle_request(request, &mut host, |_| true);
        assert!(!response.success);
        assert!(response.error.unwrap().contains("校验失败"));
    }

    #[test]
    fn unsupported_request_type_is_rejected() {
        let mut host = MockHost::open();
        let response = handle_request(authorized_request("delete-logins"), &mut host, |_| true);
        assert!(!response.success);
        assert!(response.error.unwrap().contains("不支持"));
    }

    #[test]
    fn test_associate_round_trip() {
        let mut host = MockHost::open();
        let response = handle_request(authorized_request("test-associate"), &mut host, |_| true);
        assert!(response.success);
        assert_eq!(response.id.as_deref(), Some("client-1"));
        assert!(response.entries.is_empty());
        assert_eq!(response.hash, "abc123");
        // Response envelope decrypts with the client key.
        let key = [0x11; 32];
        let iv = STANDARD.decode(&response.nonce).unwrap();
        assert_eq!(
            decrypt_field(&key, &iv, &response.verifier).unwrap(),
            STANDARD.encode(&iv)
        );
    }

    #[test]
    fn get_logins_returns_encrypted_entries_and_hmac() {
        let key = [0x11; 32];
        let mut request = authorized_request("get-logins");
        request.url = Some(field(
            "https://example.com/login",
            &key,
            &STANDARD.decode(&request.nonce).unwrap(),
        ));
        let mut host = MockHost::open();
        let response = handle_request(request, &mut host, |_| true);
        assert!(response.success);
        assert_eq!(response.entries.len(), 1);

        let iv = STANDARD.decode(&response.nonce).unwrap();
        let entry = &response.entries[0];
        assert_eq!(decrypt_field(&key, &iv, &entry.login).unwrap(), "user1");
        assert_eq!(decrypt_field(&key, &iv, &entry.password).unwrap(), "pw-1");
        assert_eq!(decrypt_field(&key, &iv, &entry.name).unwrap(), "示例站点");
        assert_eq!(decrypt_field(&key, &iv, &entry.uuid).unwrap(), "uuid-1");

        // Verifier and hmac verify against the fresh response nonce.
        let expected_verifier_plain = STANDARD.encode(&iv);
        assert_eq!(
            decrypt_field(&key, &iv, &response.verifier).unwrap(),
            expected_verifier_plain
        );
        assert_eq!(
            response.hmac,
            response_hmac(&key, &response.nonce, &response.verifier)
        );
    }

    #[test]
    fn get_logins_count_reports_count() {
        let key = [0x11; 32];
        let mut request = authorized_request("get-logins-count");
        request.url = Some(field(
            "https://example.com/login",
            &key,
            &STANDARD.decode(&request.nonce).unwrap(),
        ));
        let mut host = MockHost::open();
        let response = handle_request(request, &mut host, |_| true);
        assert!(response.success);
        assert_eq!(response.count, Some(1));
        assert_eq!(response.entries.len(), 1);
    }

    #[test]
    fn get_logins_with_no_match_returns_empty_entries() {
        let key = [0x11; 32];
        let mut request = authorized_request("get-logins");
        request.url = Some(field(
            "https://elsewhere.io/",
            &key,
            &STANDARD.decode(&request.nonce).unwrap(),
        ));
        let mut host = MockHost::open();
        let response = handle_request(request, &mut host, |_| true);
        assert!(response.success);
        assert!(response.entries.is_empty());
        assert_eq!(response.count, None);
    }

    #[test]
    fn set_login_creates_when_no_uuid_and_updates_with_uuid() {
        let key = [0x11; 32];

        let mut request = authorized_request("set-login");
        let iv = STANDARD.decode(&request.nonce).unwrap();
        request.login = Some(field("new-user", &key, &iv));
        request.password = Some(field("new-pw", &key, &iv));
        request.url = Some(field("https://example.com", &key, &iv));
        let mut host = MockHost::open();
        let response = handle_request(request, &mut host, |_| true);
        assert!(response.success);
        assert_eq!(host.created.len(), 1);
        assert_eq!(
            host.created[0],
            (
                "new-user".to_owned(),
                "new-pw".to_owned(),
                "https://example.com".to_owned()
            )
        );

        let mut request = authorized_request("set-login");
        let iv = STANDARD.decode(&request.nonce).unwrap();
        request.uuid = Some(field("uuid-1", &key, &iv));
        request.login = Some(field("old-user", &key, &iv));
        request.password = Some(field("old-pw", &key, &iv));
        request.url = Some(field("https://example.com", &key, &iv));
        let response = handle_request(request, &mut host, |_| true);
        assert!(response.success);
        assert_eq!(host.updated.len(), 1);
        assert_eq!(
            host.updated[0],
            (
                "uuid-1".to_owned(),
                "old-user".to_owned(),
                "old-pw".to_owned(),
                "https://example.com".to_owned()
            )
        );
    }

    #[test]
    fn associate_approves_and_registers_new_client() {
        let key = [0x22; 32];
        let nonce = random_bytes(16);
        let nonce_b64 = STANDARD.encode(&nonce);
        let request = BridgeRequest {
            request_type: "associate".to_owned(),
            id: Some("browser-chrome".to_owned()),
            nonce: nonce_b64.clone(),
            verifier: Some(make_verifier(&key, &nonce)),
            key: Some(STANDARD.encode(key)),
            ..Default::default()
        };
        let mut host = MockHost::open();
        let approved = std::cell::Cell::new(false);
        let response = handle_request(request, &mut host, |id| {
            assert_eq!(id, "browser-chrome");
            approved.set(true);
            true
        });
        assert!(approved.get());
        assert!(response.success);
        assert_eq!(response.id.as_deref(), Some("browser-chrome"));
        assert!(host.clients.contains_key("browser-chrome"));

        // The new client is immediately usable.
        let nonce = random_bytes(16);
        let nonce_b64 = STANDARD.encode(&nonce);
        let request = BridgeRequest {
            request_type: "test-associate".to_owned(),
            id: Some("browser-chrome".to_owned()),
            nonce: nonce_b64.clone(),
            verifier: Some(make_verifier(&key, &nonce)),
            ..Default::default()
        };
        let response = handle_request(request, &mut host, |_| true);
        assert!(response.success);
        assert_eq!(response.id.as_deref(), Some("browser-chrome"));
    }

    #[test]
    fn associate_rejects_bad_key_or_rejection() {
        let mut host = MockHost::open();

        // Key of the wrong length is refused before the approval prompt.
        let nonce = random_bytes(16);
        let nonce_b64 = STANDARD.encode(&nonce);
        let bad_key_request = BridgeRequest {
            request_type: "associate".to_owned(),
            nonce: nonce_b64,
            verifier: None,
            key: Some(STANDARD.encode([1u8; 16])),
            ..Default::default()
        };
        let response = handle_request(bad_key_request, &mut host, |_| true);
        assert!(!response.success);
        assert!(response.error.unwrap().contains("256"));

        // User rejection cancels the association.
        let key = [0x33; 32];
        let nonce = random_bytes(16);
        let nonce_b64 = STANDARD.encode(&nonce);
        let request = BridgeRequest {
            request_type: "associate".to_owned(),
            id: Some("denied-client".to_owned()),
            nonce: nonce_b64,
            verifier: Some(make_verifier(&key, &nonce)),
            key: Some(STANDARD.encode(key)),
            ..Default::default()
        };
        let response = handle_request(request, &mut host, |_| false);
        assert!(!response.success);
        assert!(!host.clients.contains_key("denied-client"));
        assert!(response.error.unwrap().contains("拒绝"));
    }

    #[test]
    fn associate_fails_when_locked() {
        let mut host = MockHost::open();
        host.open = false;
        let nonce = random_bytes(16);
        let nonce_b64 = STANDARD.encode(&nonce);
        let request = BridgeRequest {
            request_type: "associate".to_owned(),
            nonce: nonce_b64,
            verifier: None,
            key: Some(STANDARD.encode([1u8; 32])),
            ..Default::default()
        };
        let response = handle_request(request, &mut host, |_| true);
        assert!(!response.success);
        assert_eq!(response.error.as_deref(), Some("数据库未打开或已锁定"));
    }

    #[test]
    fn generate_password_meets_default_policy_and_is_fresh() {
        let mut host = MockHost::open();
        let response = handle_request(authorized_request("generate-password"), &mut host, |_| true);
        assert!(response.success);
        let password = response
            .password
            .expect("generate-password returns a password");
        assert_eq!(password.len(), 20);
        assert!(password.chars().any(|c| c.is_ascii_uppercase()));
        assert!(password.chars().any(|c| c.is_ascii_lowercase()));
        assert!(password.chars().any(|c| c.is_ascii_digit()));
        assert!(password.chars().any(|c| !c.is_ascii_alphanumeric()));

        let again = handle_request(authorized_request("generate-password"), &mut host, |_| true);
        assert_ne!(password, again.password.expect("fresh password"));
    }

    fn hex_bytes(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
