//! KeePassRPC-compatible browser bridge: protocol core (pure, testable).
//!
//! Wire semantics follow KeePassRPC (Kee extension 4.0.7, v1 protocol):
//! - SRP-6a with KeePassRPC's fixed 512-bit group (`N`/`g`/`k` constants shared
//!   with the C# plugin and the JS client), `H()` = SHA-256 over hex-string
//!   concatenations with strict uppercase/lowercase conventions.
//! - Re-auth via the challenge/response key protocol:
//!   `cr` = SHA-256("1" || secret || sc || cc), `sr` = SHA-256("0" || ...).
//! - JSON-RPC frames: AES-256-CBC (fresh IV per message) + a keyed SHA-1
//!   "hmac" over (SHA-1(key) || ciphertext || iv), all base64.
//!
//! The crypto is a shared secret by protocol design �?it is *not*
//! authenticated encryption �?so the server must stay loopback-only and every
//! first-time client runs a user-typed side-channel SRP password. Session keys
//! live in the vault session and are wiped on lock (see security-model.md).

pub(crate) mod server;

use num_bigint::BigUint;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::crypto::{
    aes_cbc_decrypt, aes_cbc_encrypt, b64_decode, b64_encode, mac_eq, random_bytes, sha1_bytes,
    sha256_hex, KEY_LEN, NONCE_LEN,
};
pub(crate) use crate::crypto::{hex, random_hex};

/// KeePassRPC default loopback port (hard-coded into the Kee extension).
pub const RPC_PORT: u16 = 12546;
/// Server-reported version (KeePassRPC 1.8.4 packed as major<<16|minor<<8|patch).
pub(crate) const PROTOCOL_VERSION: u32 = 0x010804;
/// Security level the server offers; the client accepts >= its configured
/// server minimum (default 2).
pub(crate) const SECURITY_LEVEL: u32 = 3;
/// Feature flags the server offers; must include the client's required set.
/// `KPRPC_FEATURE_ENTRY_URL_REPLACEMENT` makes Kee send `urlMergeMode = 5`
/// (replace all URLs) on `UpdateLogin` instead of the default `2`.
pub const FEATURES: [&str; 5] = [
    "KPRPC_FEATURE_VERSION_1_6",
    "KPRPC_GENERAL_CLIENTS",
    "KPRPC_SECURITY_FIX_20200729",
    "KPRPC_FEATURE_WARN_USER_WHEN_FEATURE_MISSING",
    "KPRPC_FEATURE_ENTRY_URL_REPLACEMENT",
];

/// SRP-6a group shared by KeePassRPC implementations (fixed 512-bit prime).
const SRP_N_HEX: &str = "d4c7f8a2b32c11b8fba9581ec4ba4f1b04215642ef7355e37c0fc0443ef756ea2c6b8eeb755a1c723027663caa265ef785b8ff6a9b35227a52d86633dbdfca43";
const SRP_G: u32 = 2;
/// k = SHA-1(N || g) as hex �?hard-coded in both the C# plugin and JS client.
const SRP_K_HEX: &str = "b7867f1299da8cc24ab93e08986ebc4d6a478ad0";

// ---------------------------------------------------------------------------
// Hash / crypto primitives
// ---------------------------------------------------------------------------
// sha256_hex / sha1_bytes / random_bytes / hex / random_hex live in
// `crate::crypto`; `hex` and `random_hex` are re-exported above for the
// server layer and tests.

/// The SRP group prime N as lowercase hex �?used by client-side test math
/// (the extension's own `SRPc` numbers live in `num-bigint` on both sides).
#[cfg(test)]
pub(crate) fn group_n_hex() -> String {
    SRP_N_HEX.to_owned()
}

fn mod_pow(base: &BigUint, exponent: &BigUint, modulus: &BigUint) -> BigUint {
    base.modpow(exponent, modulus)
}

fn group() -> BigUint {
    BigUint::parse_bytes(SRP_N_HEX.as_bytes(), 16).expect("SRP_N_HEX is valid hex")
}

// ---------------------------------------------------------------------------
// Wire shapes (exact KeePassRPC field names)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Envelope {
    pub protocol: String,
    pub srp: Option<SrpMessage>,
    pub key: Option<KeyMessage>,
    pub jsonrpc: Option<JsonRpcFrame>,
    pub error: Option<ErrorMessage>,
    pub version: u32,
    pub features: Option<Vec<String>>,
    pub client_type_id: Option<String>,
    pub client_display_name: Option<String>,
    pub client_display_description: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SrpMessage {
    pub stage: Option<String>,
    #[serde(rename = "I")]
    pub i: Option<String>,
    #[serde(rename = "A")]
    pub a: Option<String>,
    pub s: Option<String>,
    #[serde(rename = "B")]
    pub b: Option<String>,
    #[serde(rename = "M")]
    pub m: Option<String>,
    #[serde(rename = "M2")]
    pub m2: Option<String>,
    pub security_level: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KeyMessage {
    pub username: Option<String>,
    pub sc: Option<String>,
    pub cc: Option<String>,
    pub cr: Option<String>,
    pub sr: Option<String>,
    pub security_level: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ErrorMessage {
    pub code: String,
    pub message_params: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", default)]
pub struct JsonRpcFrame {
    pub message: String,
    pub iv: String,
    pub hmac: String,
}

impl Envelope {
    /// Server-side setup envelope (`protocol: "setup"`).
    pub fn setup() -> Self {
        Self {
            protocol: "setup".to_owned(),
            version: PROTOCOL_VERSION,
            ..Default::default()
        }
    }

    /// Server-side jsonrpc envelope carrying an encrypted frame.
    pub fn jsonrpc(frame: JsonRpcFrame) -> Self {
        Self {
            protocol: "jsonrpc".to_owned(),
            jsonrpc: Some(frame),
            version: PROTOCOL_VERSION,
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// SRP-6a (server side, KeePassRPC variant)
// ---------------------------------------------------------------------------

/// Server-side SRP state. The side-channel password is consumed to build the
/// verifier immediately and zeroized �?later steps need only `v`.
pub struct SrpServer {
    b: BigUint,
    /// B exactly as sent (uppercase hex).
    b_hex: String,
    v: BigUint,
    /// S as uppercase hex, kept for M2 and K (both depend on S hex).
    s_hex_upper: Option<String>,
}

impl SrpServer {
    /// Start a handshake: derive `v = g^x` from the side-channel password and
    /// emit `identifyToClient`'s `{s, B, securityLevel}` payload.
    pub fn begin(password: &str) -> (Self, Value) {
        let salt = random_hex(16);
        let x = BigUint::parse_bytes(sha256_hex(&format!("{salt}{password}")).as_bytes(), 16)
            .expect("sha256 hex parses");
        let g = BigUint::from(SRP_G);
        let n = group();
        let v = mod_pow(&g, &x, &n);
        let b = BigUint::from_bytes_be(&random_bytes(32)) % &n;
        let k = BigUint::parse_bytes(SRP_K_HEX.as_bytes(), 16).expect("k parses");
        let b_val = (&k * &v + mod_pow(&g, &b, &n)) % &n;
        let b_val_hex = b_val.to_str_radix(16).to_uppercase();
        let payload = json!({
            "stage": "identifyToClient",
            "s": salt,
            "B": b_val_hex,
            "securityLevel": SECURITY_LEVEL,
        });
        (
            Self {
                b,
                b_hex: b_val_hex,
                v,
                s_hex_upper: None,
            },
            payload,
        )
    }

    /// Verify the client proof `M` and produce `M2` on success.
    ///
    /// `a` must be the A string exactly as the client sent it (uppercase hex).
    ///
    /// Kee's `SRPc` client computes `S = modPow(B - kgx, a + ux, N)` with
    /// ECMAScript BigInt `%`, which keeps the sign of the dividend: whenever
    /// `B - kgx < 0` and the exponent is odd, the client's S comes out as a
    /// negative number `-(r)` and its hex encoding `"-<hex>"` (not the positive
    /// residue `N - r`) is hashed into M and M2. The server's positive S is
    /// exactly `N - r` in that case, so a failing M is retried against the
    /// negative-S candidate before giving up.
    pub fn verify_proof(&mut self, a: &str, m: &str) -> Result<String, RpcError> {
        let n = group();
        let a_val = BigUint::parse_bytes(a.as_bytes(), 16)
            .ok_or(RpcError::InvalidMessage("A 不是合法十六进制".to_owned()))?;
        let u = BigUint::parse_bytes(sha256_hex(&format!("{a}{}", self.b_hex)).as_bytes(), 16)
            .expect("sha256 hex parses");
        let s = mod_pow(&self.v, &u, &n);
        let s = (a_val * s) % &n;
        let s = mod_pow(&s, &self.b, &n);
        let s_hex = s.to_str_radix(16).to_uppercase();
        if s_hex.chars().all(|c| c == '0') {
            return Err(RpcError::AuthFailed);
        }
        let m_expected = sha256_hex(&format!("{a}{}{s_hex}", self.b_hex));
        let client_s_hex = if m.to_lowercase() == m_expected {
            s_hex
        } else {
            let neg_hex = (&n - &s).to_str_radix(16).to_uppercase();
            let m_expected_neg = sha256_hex(&format!("{a}{}-{}", self.b_hex, neg_hex));
            if m.to_lowercase() == m_expected_neg {
                eprintln!(
                    "[rpc] verify_proof: accepted negative-S M (Kee SRPc BigInt remainder quirk)"
                );
                format!("-{neg_hex}")
            } else {
                eprintln!(
                    "[rpc] verify_proof: M mismatch (client m[..12]={}.. expected[..12]={}..)",
                    &m.to_lowercase()[..12.min(m.len())],
                    &m_expected[..12]
                );
                return Err(RpcError::AuthFailed);
            }
        };
        let m2 = sha256_hex(&format!("{a}{m}{client_s_hex}"));
        self.s_hex_upper = Some(client_s_hex);
        Ok(m2)
    }

    /// The 64-char lowercase-hex session key (K = H(S uppercase hex)); valid
    /// only after `verify_proof` succeeded.
    pub fn secret_key(&self) -> Result<String, RpcError> {
        match &self.s_hex_upper {
            Some(s_hex) => Ok(sha256_hex(s_hex)),
            None => Err(RpcError::InvalidMessage("SRP 尚未完成".to_owned())),
        }
    }
}

// ---------------------------------------------------------------------------
// Key-auth challenge/response (stage 1b)
// ---------------------------------------------------------------------------

pub fn key_auth_cr(secret_hex: &str, sc: &str, cc: &str) -> String {
    sha256_hex(&format!("1{secret_hex}{sc}{cc}"))
}

pub fn key_auth_sr(secret_hex: &str, sc: &str, cc: &str) -> String {
    sha256_hex(&format!("0{secret_hex}{sc}{cc}"))
}

// ---------------------------------------------------------------------------
// JSON-RPC frames
// ---------------------------------------------------------------------------

/// Encrypt one JSON-RPC payload under the session key (fresh IV).
pub fn encrypt_frame(secret: &[u8], plaintext: &str) -> JsonRpcFrame {
    let iv = random_bytes(NONCE_LEN);
    let ciphertext = aes_cbc_encrypt(secret, &iv, plaintext.as_bytes());
    JsonRpcFrame {
        message: b64_encode(&ciphertext),
        iv: b64_encode(&iv),
        hmac: frame_mac(secret, &ciphertext, &iv),
    }
}

/// Decrypt a frame after verifying its keyed SHA-1 "hmac".
pub fn decrypt_frame(secret: &[u8], frame: &JsonRpcFrame) -> Result<String, RpcError> {
    let iv =
        b64_decode(&frame.iv).map_err(|_| RpcError::InvalidMessage("IV 格式无效".to_owned()))?;
    let ciphertext = b64_decode(&frame.message)
        .map_err(|_| RpcError::InvalidMessage("密文格式无效".to_owned()))?;
    let expected = frame_mac(secret, &ciphertext, &iv);
    if !mac_eq(&expected, &frame.hmac) {
        return Err(RpcError::AuthFailed);
    }
    let plaintext = aes_cbc_decrypt(secret, &iv, &ciphertext)
        .map_err(|_| RpcError::InvalidMessage("解密失败".to_owned()))?;
    String::from_utf8(plaintext).map_err(|_| RpcError::InvalidMessage("明文不是 UTF-8".to_owned()))
}

/// base64(SHA-1(SHA-1(key) || ciphertext || iv)) — the protocol's naive MAC.
fn frame_mac(secret: &[u8], ciphertext: &[u8], iv: &[u8]) -> String {
    let key_hash = sha1_bytes(secret);
    let mut data = Vec::with_capacity(key_hash.len() + ciphertext.len() + iv.len());
    data.extend_from_slice(&key_hash);
    data.extend_from_slice(ciphertext);
    data.extend_from_slice(iv);
    b64_encode(&sha1_bytes(&data))
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum RpcError {
    /// Malformed envelope/frame �?answer `INVALID_MESSAGE`.
    InvalidMessage(String),
    /// Authentication failed �?answer `AUTH_FAILED`.
    AuthFailed,
    /// Method not implemented yet �?answer a JSON-RPC error.
    Unsupported(String),
    /// Vault is not open �?answer a JSON-RPC error.
    Locked,
    /// `oldLoginUUID` did not resolve to an entry (KeePassRPC's exception).
    EntryNotFound,
    /// Target entry lives in the recycle bin �?KeyVault policy rejects the
    /// write (Kee's read paths never surface recycled entries, so this is
    /// unreachable through the extension and purely defense-in-depth).
    InRecycleBin,
}

// ---------------------------------------------------------------------------
// Host abstraction + DTOs (camelCase wire shapes)
// ---------------------------------------------------------------------------

/// One credential the bridge hands to the browser; plaintext exists only
/// inside an encrypted JSON-RPC frame.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcLogin {
    pub uuid: String,
    pub title: String,
    pub username: String,
    pub password: String,
    pub urls: Vec<String>,
    pub http_realm: String,
    pub icon_image_data: String,
    pub parent_group: RpcGroupRef,
    /// KeePassRPC MatchAccuracy: 1 = Best (exact URL), 3 = HostnameAndPort.
    pub match_accuracy: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RpcGroupRef {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub icon_image_data: String,
}

#[derive(Debug, Clone)]
pub struct RpcGroup {
    pub uuid: String,
    pub title: String,
    pub path: String,
    pub icon_image_data: String,
    pub entries: Vec<RpcLogin>,
    pub children: Vec<RpcGroup>,
}

#[derive(Debug, Clone)]
pub struct RpcDatabase {
    pub name: String,
    pub file_name: String,
    pub icon_image_data: String,
    pub root: RpcGroup,
    pub active: bool,
}

/// The vault-session subset the RPC bridge needs. Implemented by `VaultSession`
/// (vault.rs) so dispatch stays independent of vault internals.
pub trait RpcHost {
    fn is_open(&self) -> bool;
    /// Raw 32-byte session key for a client username (wiped on lock).
    fn rpc_key(&self, username: &str) -> Option<Vec<u8>>;
    fn register_rpc_key(&mut self, username: &str, key: Vec<u8>);
    fn database(&self) -> Option<RpcDatabase>;
    /// Find logins by URL host match (recycle bin skipped), uuid, or
    /// free-text/username search �?one or more criteria may be active.
    fn find_logins(
        &self,
        urls: &[String],
        uuid: Option<&str>,
        free_text: Option<&str>,
        username: Option<&str>,
    ) -> Vec<RpcLogin>;
    /// Create an entry under `parent_uuid` (empty or unresolvable �?root
    /// group) and return it. Implements KeePassRPC `AddLogin`.
    fn add_login(&mut self, login: &RpcLoginWrite, parent_uuid: &str)
        -> Result<RpcLogin, RpcError>;
    /// Update the entry identified by `old_uuid` with `login`, merging URLs
    /// per `url_merge_mode` (KeePassRPC `UpdateLogin` + `MergeEntries`).
    fn update_login(
        &mut self,
        login: &RpcLoginWrite,
        old_uuid: &str,
        url_merge_mode: u8,
    ) -> Result<RpcLogin, RpcError>;
}

/// Decode the 64-char lowercase-hex session key into raw bytes.
pub fn secret_bytes(secret_hex: &str) -> Result<Vec<u8>, RpcError> {
    let bytes = hex_decode(secret_hex)?;
    if bytes.len() != KEY_LEN {
        return Err(RpcError::InvalidMessage("密钥长度无效".to_owned()));
    }
    Ok(bytes)
}

fn hex_decode(s: &str) -> Result<Vec<u8>, RpcError> {
    if !s.len().is_multiple_of(2) {
        return Err(RpcError::InvalidMessage("密钥格式无效".to_owned()));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        let byte = u8::from_str_radix(&s[i..i + 2], 16)
            .map_err(|_| RpcError::InvalidMessage("密钥格式无效".to_owned()))?;
        out.push(byte);
    }
    Ok(out)
}

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

fn entry_dto(e: &RpcLogin, db: &RpcDatabase) -> Value {
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

// ---------------------------------------------------------------------------
// Write-path shapes (Kee 4.0.7 `Entry.toKPRPCEntryDTO` + `Field` DTOs)
// ---------------------------------------------------------------------------

/// One `formFieldList` item sent by Kee. Types mirror the extension's
/// `FormFieldTypeDTO`: `FFTusername` / `FFTpassword` / `FFTtext` /
/// `FFTradio` / `FFTcheckbox` / `FFTselect`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RpcFieldWrite {
    pub id: String,
    pub name: String,
    pub display_name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    pub value: String,
    pub page: i64,
}

/// Entry data sent by Kee's `AddLogin`/`UpdateLogin` (v1 `Entry` DTO). The
/// username/password live inside `formFieldList`; extra fields are mapped by
/// name, mirroring the plugin's `setPwEntryFromEntry`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RpcLoginWrite {
    pub title: String,
    #[serde(rename = "uRLs")]
    pub urls: Vec<String>,
    #[serde(rename = "hTTPRealm")]
    pub http_realm: String,
    pub icon_image_data: String,
    pub form_field_list: Vec<RpcFieldWrite>,
}

/// KeePassRPC username convention: every `FFTusername` field maps to `UserName`
/// (the plugin writes all of them, last one wins).
pub fn write_username(login: &RpcLoginWrite) -> String {
    login
        .form_field_list
        .iter()
        .rev()
        .find(|f| f.field_type == "FFTusername")
        .map(|f| f.value.clone())
        .unwrap_or_default()
}

/// KeePassRPC password convention: the first `FFTpassword` field maps to
/// `Password`; later ones fall through to custom fields.
pub fn write_password(login: &RpcLoginWrite) -> String {
    login
        .form_field_list
        .iter()
        .find(|f| f.field_type == "FFTpassword")
        .map(|f| f.value.clone())
        .unwrap_or_default()
}

/// Extra form fields (everything but the consumed username/password fields),
/// named `displayName` (fallback `name`), mirroring `setPwEntryFromEntry`.
pub fn write_custom_fields(login: &RpcLoginWrite) -> Vec<(String, String)> {
    let mut seen_password = false;
    let mut out = Vec::new();
    for field in &login.form_field_list {
        match field.field_type.as_str() {
            "FFTusername" => {}
            "FFTpassword" if !seen_password => {
                seen_password = true;
            }
            _ => {
                let name = if field.display_name.is_empty() {
                    field.name.clone()
                } else {
                    field.display_name.clone()
                };
                if !name.is_empty() {
                    out.push((name, field.value.clone()));
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// URL merging (KeePassRPC `MergeEntries` / `MergeInNewURLs` semantics)
// ---------------------------------------------------------------------------

/// C# `MergeInNewURLs(destURLs, sourceURLs)`: source URLs are walked backwards
/// and inserted at the front when missing; the source's primary URL is promoted
/// to the front when already present (so it stays the primary match).
fn merge_in_new_urls(dest: &mut Vec<String>, source: &[String]) {
    for i in (0..source.len()).rev() {
        let url = &source[i];
        if let Some(pos) = dest.iter().position(|d| d == url) {
            if i == 0 {
                dest.remove(pos);
                dest.insert(0, url.clone());
            }
        } else {
            dest.insert(0, url.clone());
        }
    }
}

/// Apply KeePassRPC `MergeEntries` URL merging to a destination URL list
/// (`[primary, alt...]`) with a source list (`[primary, alt...]`). Modes mirror
/// the plugin's `urlMergeMode`:
/// 1 = merge source URLs in (old URLs kept, new ones first, still matchable);
/// 2 = delete the old primary URL, then merge;
/// 3 = keep old URLs, append only source URLs not already present;
/// 4 = leave URLs unchanged;
/// 5 = replace the whole list with the source URLs.
/// Unknown modes behave like 4 (the plugin's switch has no default).
pub fn merge_urls(dest: &[String], source: &[String], mode: u8) -> Vec<String> {
    let mut dest = dest.to_vec();
    match mode {
        1 => merge_in_new_urls(&mut dest, source),
        2 => {
            if !dest.is_empty() {
                dest.remove(0);
            }
            merge_in_new_urls(&mut dest, source);
        }
        3 => {
            for url in source {
                if !dest.contains(url) {
                    dest.push(url.clone());
                }
            }
        }
        4 => {}
        5 => dest = source.to_vec(),
        _ => {}
    }
    dest
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
        "GeneratePassword" => Ok(json!(crate::bridge::generate_password())),
        "AddLogin" => {
            let db = host.database().ok_or(RpcError::Locked)?;
            let params =
                params.ok_or_else(|| RpcError::InvalidMessage("AddLogin 缺少参数".to_owned()))?;
            let login: RpcLoginWrite =
                serde_json::from_value(params.get(0).cloned().unwrap_or(Value::Null))
                    .map_err(|e| RpcError::InvalidMessage(format!("login 参数无效: {e}")))?;
            let parent_uuid = params.get(1).and_then(|v| v.as_str()).unwrap_or_default();
            let entry = host.add_login(&login, parent_uuid)?;
            Ok(entry_dto(&entry, &db))
        }
        "UpdateLogin" => {
            let db = host.database().ok_or(RpcError::Locked)?;
            let params = params
                .ok_or_else(|| RpcError::InvalidMessage("UpdateLogin 缺少参数".to_owned()))?;
            let login: RpcLoginWrite =
                serde_json::from_value(params.get(0).cloned().unwrap_or(Value::Null))
                    .map_err(|e| RpcError::InvalidMessage(format!("login 参数无效: {e}")))?;
            let old_uuid = params.get(1).and_then(|v| v.as_str()).unwrap_or_default();
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
            let entry = host.update_login(&login, old_uuid, url_merge_mode)?;
            Ok(entry_dto(&entry, &db))
        }
        other => Err(RpcError::Unsupported(other.to_owned())),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::url_host;
    use std::collections::HashMap;

    fn big(n_hex: &str) -> BigUint {
        BigUint::parse_bytes(n_hex.as_bytes(), 16).unwrap()
    }

    /// Mirrors kprpcClient.js `SRPc.calculations` + `key()` exactly, so the
    /// server-side math is cross-checked against the extension's algorithm.
    /// `a` is the client's private ephemeral; `A = g^a mod N` is derived here,
    /// exactly like `SRPc` does (`this.A = modPow(this.g, this.a, this.N)`).
    fn js_client_handshake(
        salt_hex: &str,
        b_hex: &str,
        password: &str,
        a: &BigUint,
    ) -> (String, String, String) {
        let n = group();
        let g = BigUint::from(SRP_G);
        let k = big(SRP_K_HEX);
        let a_public = mod_pow(&g, a, &n);
        let a_hex = a_public.to_str_radix(16).to_uppercase();
        let b = big(b_hex);
        let u = big(&sha256_hex(&format!("{a_hex}{b_hex}")));
        let x = big(&sha256_hex(&format!("{salt_hex}{password}")));
        let kgx = (&k * mod_pow(&g, &x, &n)) % &n;
        let aux = a + &u * &x;
        let s = mod_pow(&((&b + &n - &kgx) % &n), &aux, &n);
        let s_upper = s.to_str_radix(16).to_uppercase();
        let m = sha256_hex(&format!("{a_hex}{b_hex}{s_upper}"));
        let m2 = sha256_hex(&format!("{a_hex}{m}{s_upper}"));
        let k_hex = sha256_hex(&s_upper);
        (m, m2, k_hex)
    }

    #[test]
    fn srp_round_trip_with_js_style_client() {
        let password = "yk4q-9Kz2!";
        let (mut server, payload) = SrpServer::begin(password);
        let s = payload["s"].as_str().unwrap().to_owned();
        let b = payload["B"].as_str().unwrap().to_owned();
        assert_eq!(payload["stage"], "identifyToClient");
        assert_eq!(payload["securityLevel"], 3);
        assert!(b
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit()));

        let a = BigUint::from_bytes_be(&random_bytes(32)) % &group();
        let a_public = mod_pow(&BigUint::from(SRP_G), &a, &group());
        let a_hex = a_public.to_str_radix(16).to_uppercase();
        let (m, m2_expected, k_expected) = js_client_handshake(&s, &b, password, &a);

        let m2 = server.verify_proof(&a_hex, &m).expect("proof must verify");
        assert_eq!(m2, m2_expected);
        assert_eq!(server.secret_key().unwrap(), k_expected);
        assert_eq!(secret_bytes(&k_expected).unwrap().len(), KEY_LEN);
    }

    #[test]
    fn srp_rejects_wrong_password_or_tampered_proof() {
        let (mut server, payload) = SrpServer::begin("correct-pw");
        let s = payload["s"].as_str().unwrap().to_owned();
        let b = payload["B"].as_str().unwrap().to_owned();
        let a = BigUint::from_bytes_be(&random_bytes(32)) % &group();
        let a_public = mod_pow(&BigUint::from(SRP_G), &a, &group());
        let a_hex = a_public.to_str_radix(16).to_uppercase();

        let (m, _, _) = js_client_handshake(&s, &b, "wrong-pw", &a);
        assert_eq!(server.verify_proof(&a_hex, &m), Err(RpcError::AuthFailed));

        let (mut server2, payload2) = SrpServer::begin("correct-pw");
        let s2 = payload2["s"].as_str().unwrap().to_owned();
        let b2 = payload2["B"].as_str().unwrap().to_owned();
        let (m2, _, _) = js_client_handshake(&s2, &b2, "correct-pw", &a);
        let tampered = format!("{}{}", if &m2[0..1] == "a" { "b" } else { "a" }, &m2[1..]);
        assert_eq!(
            server2.verify_proof(&a_hex, &tampered),
            Err(RpcError::AuthFailed)
        );
    }

    #[test]
    fn secret_key_requires_finished_handshake() {
        let (server, _) = SrpServer::begin("pw");
        assert_eq!(
            server.secret_key(),
            Err(RpcError::InvalidMessage("SRP 尚未完成".to_owned()))
        );
    }

    #[test]
    fn key_auth_challenge_response_matches_expected_hash() {
        let secret = random_hex(32);
        let sc = random_hex(32);
        let cc = random_hex(32);
        let cr = key_auth_cr(&secret, &sc, &cc);
        let sr = key_auth_sr(&secret, &sc, &cc);
        assert_eq!(cr, sha256_hex(&format!("1{secret}{sc}{cc}")));
        assert_eq!(sr, sha256_hex(&format!("0{secret}{sc}{cc}")));
        assert_ne!(cr, sr);
        let other = key_auth_cr(&random_hex(32), &sc, &cc);
        assert_ne!(cr, other);
    }

    #[test]
    fn frame_round_trip_and_tamper_rejection() {
        let secret = secret_bytes(&random_hex(32)).unwrap();
        let frame = encrypt_frame(&secret, r#"{"jsonrpc":"2.0","id":7}"#);
        let plaintext = decrypt_frame(&secret, &frame).unwrap();
        assert_eq!(plaintext, r#"{"jsonrpc":"2.0","id":7}"#);

        let mut tampered = frame.clone();
        tampered.message.push('=');
        assert_eq!(
            decrypt_frame(&secret, &tampered),
            Err(RpcError::InvalidMessage("密文格式无效".to_owned()))
        );

        let mut flipped = frame.clone();
        let mid = flipped.message.len() / 2;
        let ch = flipped.message.as_bytes()[mid];
        flipped
            .message
            .replace_range(mid..mid + 1, if ch == b'A' { "B" } else { "A" });
        assert_eq!(decrypt_frame(&secret, &flipped), Err(RpcError::AuthFailed));

        let other_secret = secret_bytes(&random_hex(32)).unwrap();
        assert_eq!(
            decrypt_frame(&other_secret, &frame),
            Err(RpcError::AuthFailed)
        );
    }

    struct MockHost {
        open: bool,
        keys: HashMap<String, Vec<u8>>,
        db: RpcDatabase,
        next_id: u32,
    }

    impl RpcHost for MockHost {
        fn is_open(&self) -> bool {
            self.open
        }
        fn rpc_key(&self, username: &str) -> Option<Vec<u8>> {
            self.keys.get(username).cloned()
        }
        fn register_rpc_key(&mut self, username: &str, key: Vec<u8>) {
            self.keys.insert(username.to_owned(), key);
        }
        fn database(&self) -> Option<RpcDatabase> {
            self.open.then(|| self.db.clone())
        }
        fn find_logins(
            &self,
            urls: &[String],
            uuid: Option<&str>,
            free_text: Option<&str>,
            username: Option<&str>,
        ) -> Vec<RpcLogin> {
            if !self.open {
                return Vec::new();
            }
            self.db
                .root
                .entries
                .iter()
                .filter(|e| {
                    let by_url = urls.iter().any(|u| {
                        let u_host = url_host(u);
                        e.urls.iter().any(|eu| u == eu || url_host(eu) == u_host)
                    });
                    let by_uuid = uuid.is_some_and(|id| id == e.uuid);
                    let by_text =
                        free_text.is_some_and(|t| e.title.contains(t) || e.username.contains(t));
                    let by_username =
                        username.is_some_and(|u| !u.is_empty() && e.username.contains(u));
                    by_url || by_uuid || by_text || by_username
                })
                .cloned()
                .collect()
        }
        fn add_login(
            &mut self,
            login: &RpcLoginWrite,
            parent_uuid: &str,
        ) -> Result<RpcLogin, RpcError> {
            if !self.open {
                return Err(RpcError::Locked);
            }
            let parent = if parent_uuid == "g-1" {
                RpcGroupRef {
                    uuid: "g-1".to_owned(),
                    title: "Internet".to_owned(),
                    path: "/Internet".to_owned(),
                    icon_image_data: String::new(),
                }
            } else {
                RpcGroupRef {
                    uuid: self.db.root.uuid.clone(),
                    title: self.db.root.title.clone(),
                    path: String::new(),
                    icon_image_data: String::new(),
                }
            };
            let created = RpcLogin {
                uuid: format!("e-{}", self.next_id),
                title: login.title.clone(),
                username: write_username(login),
                password: write_password(login),
                urls: login.urls.clone(),
                http_realm: login.http_realm.clone(),
                icon_image_data: login.icon_image_data.clone(),
                parent_group: parent,
                match_accuracy: 1,
            };
            self.next_id += 1;
            self.db.root.entries.push(created.clone());
            Ok(created)
        }
        fn update_login(
            &mut self,
            login: &RpcLoginWrite,
            old_uuid: &str,
            url_merge_mode: u8,
        ) -> Result<RpcLogin, RpcError> {
            if !self.open {
                return Err(RpcError::Locked);
            }
            let pos = self
                .db
                .root
                .entries
                .iter()
                .position(|e| e.uuid == old_uuid)
                .ok_or(RpcError::EntryNotFound)?;
            let old = &self.db.root.entries[pos];
            let updated = RpcLogin {
                uuid: old.uuid.clone(),
                title: login.title.clone(),
                username: write_username(login),
                password: write_password(login),
                urls: merge_urls(&old.urls, &login.urls, url_merge_mode),
                http_realm: login.http_realm.clone(),
                icon_image_data: login.icon_image_data.clone(),
                parent_group: old.parent_group.clone(),
                match_accuracy: 1,
            };
            self.db.root.entries[pos] = updated.clone();
            Ok(updated)
        }
    }

    fn mock_host() -> MockHost {
        let parent = RpcGroupRef {
            uuid: "g-1".to_owned(),
            title: "Internet".to_owned(),
            path: "/Internet".to_owned(),
            icon_image_data: String::new(),
        };
        let login = RpcLogin {
            uuid: "e-1".to_owned(),
            title: "Example".to_owned(),
            username: "alice".to_owned(),
            password: "s3cret".to_owned(),
            urls: vec!["https://example.com/login".to_owned()],
            http_realm: String::new(),
            icon_image_data: String::new(),
            parent_group: parent.clone(),
            match_accuracy: 3,
        };
        let root = RpcGroup {
            uuid: "g-root".to_owned(),
            title: "Root".to_owned(),
            path: String::new(),
            icon_image_data: String::new(),
            entries: vec![login],
            children: Vec::new(),
        };
        let db = RpcDatabase {
            name: "My Vault".to_owned(),
            file_name: "vault.kdbx".to_owned(),
            icon_image_data: String::new(),
            root,
            active: true,
        };
        MockHost {
            open: true,
            keys: HashMap::new(),
            db,
            next_id: 2,
        }
    }

    #[test]
    fn get_all_databases_returns_dto_tree() {
        let mut host = mock_host();
        let result = handle_jsonrpc(&mut host, "GetAllDatabases", None).unwrap();
        let dbs = result.as_array().unwrap();
        assert_eq!(dbs.len(), 1);
        let dto = &dbs[0];
        assert_eq!(dto["fileName"], "vault.kdbx");
        assert_eq!(dto["active"], true);
        assert_eq!(dto["root"]["title"], "Root");
        assert_eq!(
            dto["root"]["childLightEntries"][0]["usernameValue"],
            "alice"
        );
        assert_eq!(dto["root"]["childLightEntries"][0]["uniqueID"], "e-1");
        assert_eq!(
            dto["root"]["childLightEntries"][0]["uRLs"][0],
            "https://example.com/login"
        );
    }

    #[test]
    fn find_logins_matches_url_uuid_and_text() {
        let mut host = mock_host();

        let params = json!([
            ["https://example.com/dashboard"],
            null,
            null,
            "LSTnoForms",
            false,
            null,
            "",
            null,
            null
        ]);
        let result = handle_jsonrpc(&mut host, "FindLogins", Some(&params)).unwrap();
        let entries = result.as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["uniqueID"], "e-1");
        assert_eq!(entries[0]["formFieldList"][0]["type"], "FFTusername");
        assert_eq!(entries[0]["formFieldList"][0]["value"], "alice");
        assert_eq!(entries[0]["formFieldList"][1]["value"], "s3cret");
        assert_eq!(entries[0]["db"]["fileName"], "vault.kdbx");
        assert_eq!(entries[0]["parent"]["path"], "/Internet");

        let params = json!([
            ["https://other.example/x"],
            null,
            null,
            "LSTnoForms",
            false,
            "e-1",
            "",
            null,
            null
        ]);
        let result = handle_jsonrpc(&mut host, "FindLogins", Some(&params)).unwrap();
        assert_eq!(result.as_array().unwrap().len(), 1);

        let params = json!([[], null, null, "LSTnoForms", false, null, "", "Examp", null]);
        let result = handle_jsonrpc(&mut host, "FindLogins", Some(&params)).unwrap();
        assert_eq!(result.as_array().unwrap().len(), 1);

        let params = json!([[], null, null, "LSTnoForms", false, null, "", null, "bob"]);
        let result = handle_jsonrpc(&mut host, "FindLogins", Some(&params)).unwrap();
        assert_eq!(result.as_array().unwrap().len(), 0);
    }

    #[test]
    fn password_profiles_and_generation() {
        let mut host = mock_host();
        let result = handle_jsonrpc(&mut host, "GetPasswordProfiles", None).unwrap();
        assert_eq!(result, json!(["Default"]));
        let result =
            handle_jsonrpc(&mut host, "GeneratePassword", Some(&json!(["Default", ""]))).unwrap();
        let pw = result.as_str().unwrap();
        assert_eq!(pw.len(), 20);
        assert!(pw.chars().any(|c| c.is_ascii_uppercase()));
        assert!(pw.chars().any(|c| c.is_ascii_lowercase()));
        assert!(pw.chars().any(|c| c.is_ascii_digit()));
        assert!(pw.chars().any(|c| !c.is_ascii_alphanumeric()));
    }

    #[test]
    fn locked_host_answers_error_and_unsupported_method_errors() {
        let mut host = mock_host();
        host.open = false;
        assert_eq!(
            handle_jsonrpc(&mut host, "GetAllDatabases", None),
            Err(RpcError::Locked)
        );
        assert_eq!(
            handle_jsonrpc(&mut host, "AddLogin", None),
            Err(RpcError::Locked)
        );
        assert_eq!(
            handle_jsonrpc(&mut host, "UpdateLogin", None),
            Err(RpcError::Locked)
        );
        host.open = true;
        assert_eq!(
            handle_jsonrpc(&mut host, "AddGroup", None),
            Err(RpcError::Unsupported("AddGroup".to_owned()))
        );
    }

    fn login_write(title: &str, username: &str, password: &str, urls: &[&str]) -> Value {
        json!({
            "title": title,
            "uRLs": urls,
            "hTTPRealm": "",
            "iconImageData": "",
            "formFieldList": [
                { "displayName": "KeePass username", "id": "u", "name": "user", "type": "FFTusername", "value": username, "page": 0 },
                { "displayName": "KeePass password", "id": "p", "name": "pass", "type": "FFTpassword", "value": password, "page": 0 },
                { "displayName": "Custom note", "id": "n", "name": "note", "type": "FFTtext", "value": "hello", "page": 0 },
            ],
        })
    }

    #[test]
    fn url_merge_modes_match_keepassrpc_semantics() {
        let old = vec![
            "https://old.example.com".to_owned(),
            "https://alt.example.com".to_owned(),
        ];
        let src = vec![
            "https://new.example.com".to_owned(),
            "https://alt.example.com".to_owned(),
        ];
        // 1: source walked backwards, missing URLs inserted at front; the
        // source primary is promoted when already present.
        assert_eq!(
            merge_urls(&old, &src, 1),
            vec![
                "https://new.example.com",
                "https://old.example.com",
                "https://alt.example.com"
            ]
        );
        // 2: old primary removed first, then merged.
        assert_eq!(
            merge_urls(&old, &src, 2),
            vec!["https://new.example.com", "https://alt.example.com"]
        );
        // 3: keep old, append only new ones.
        assert_eq!(
            merge_urls(&old, &src, 3),
            vec![
                "https://old.example.com",
                "https://alt.example.com",
                "https://new.example.com"
            ]
        );
        // 4: unchanged.
        assert_eq!(merge_urls(&old, &src, 4), old);
        // 5: whole-list replace.
        assert_eq!(merge_urls(&old, &src, 5), src);
        // Unknown modes behave like 4 (plugin switch has no default).
        assert_eq!(merge_urls(&old, &src, 0), old);

        // Source primary promotion: already present but not first.
        let promoted = merge_urls(
            &[
                "https://alt.example.com".to_owned(),
                "https://new.example.com".to_owned(),
            ],
            &["https://new.example.com".to_owned()],
            1,
        );
        assert_eq!(
            promoted,
            vec!["https://new.example.com", "https://alt.example.com"]
        );

        // Mode 2 with an empty source leaves no URL (old primary deleted).
        assert_eq!(
            merge_urls(&old, &Vec::<String>::new(), 2),
            vec!["https://alt.example.com"]
        );
    }

    #[test]
    fn add_login_creates_entry_and_returns_dto() {
        let mut host = mock_host();
        let params = json!([
            login_write(
                "New Site",
                "bob",
                "pw-1",
                &["https://new.example.com/login"]
            ),
            "g-1",
            "vault.kdbx",
        ]);
        let result = handle_jsonrpc(&mut host, "AddLogin", Some(&params)).unwrap();
        assert_eq!(result["uniqueID"], "e-2");
        assert_eq!(result["title"], "New Site");
        assert_eq!(result["uRLs"][0], "https://new.example.com/login");
        assert_eq!(result["formFieldList"][0]["type"], "FFTusername");
        assert_eq!(result["formFieldList"][0]["value"], "bob");
        assert_eq!(result["formFieldList"][1]["value"], "pw-1");
        assert_eq!(result["parent"]["path"], "/Internet");
        assert_eq!(result["db"]["fileName"], "vault.kdbx");

        // The new entry is visible to subsequent reads.
        let params = json!([
            [],
            null,
            null,
            "LSTnoForms",
            false,
            null,
            "",
            "New Site",
            null
        ]);
        let result = handle_jsonrpc(&mut host, "FindLogins", Some(&params)).unwrap();
        let entries = result.as_array().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0]["uniqueID"], "e-2");
        assert_eq!(entries[0]["formFieldList"][0]["value"], "bob");
    }

    #[test]
    fn add_login_without_parent_uses_root_group() {
        let mut host = mock_host();
        let params = json!([
            login_write("Rooted", "u", "p", &["https://root.example.com"]),
            null,
            "vault.kdbx",
        ]);
        let result = handle_jsonrpc(&mut host, "AddLogin", Some(&params)).unwrap();
        assert_eq!(result["parent"]["uniqueID"], "g-root");
        assert_eq!(result["parent"]["path"], "");

        // Unknown parent uuid also falls back to root.
        let params = json!([
            login_write("Rooted 2", "u", "p", &["https://root2.example.com"]),
            "does-not-exist",
            "vault.kdbx",
        ]);
        let result = handle_jsonrpc(&mut host, "AddLogin", Some(&params)).unwrap();
        assert_eq!(result["parent"]["uniqueID"], "g-root");
    }

    #[test]
    fn add_login_with_missing_login_errors() {
        let mut host = mock_host();
        let params = json!([null, "g-1", "vault.kdbx"]);
        assert!(matches!(
            handle_jsonrpc(&mut host, "AddLogin", Some(&params)),
            Err(RpcError::InvalidMessage(_))
        ));
    }

    #[test]
    fn update_login_replaces_or_merges_urls() {
        let mut host = mock_host();
        // Mode 5 (Kee sends this when KPRPC_FEATURE_ENTRY_URL_REPLACEMENT is
        // offered): whole-list replace.
        let params = json!([
            login_write(
                "Example",
                "alice",
                "s3cret",
                &["https://only-new.example.com"]
            ),
            "e-1",
            5,
            "vault.kdbx",
        ]);
        let result = handle_jsonrpc(&mut host, "UpdateLogin", Some(&params)).unwrap();
        assert_eq!(result["uniqueID"], "e-1");
        assert_eq!(result["uRLs"], json!(["https://only-new.example.com"]));
        assert_eq!(result["formFieldList"][0]["value"], "alice");

        // Mode 1: old URL kept, new one promoted to primary.
        let params = json!([
            login_write(
                "Example",
                "alice",
                "s3cret",
                &["https://second.example.com"]
            ),
            "e-1",
            1,
            "vault.kdbx",
        ]);
        let result = handle_jsonrpc(&mut host, "UpdateLogin", Some(&params)).unwrap();
        assert_eq!(
            result["uRLs"],
            json!(["https://second.example.com", "https://only-new.example.com",])
        );
    }

    #[test]
    fn update_login_validates_params_and_unknown_uuid() {
        let mut host = mock_host();
        // Empty oldLoginUUID �?error (plugin ArgumentException mirror).
        let params = json!([login_write("X", "u", "p", &[]), "", 5, "vault.kdbx"]);
        assert!(matches!(
            handle_jsonrpc(&mut host, "UpdateLogin", Some(&params)),
            Err(RpcError::InvalidMessage(_))
        ));
        // Empty dbFileName �?error (plugin ArgumentException mirror).
        let params = json!([login_write("X", "u", "p", &[]), "e-1", 5, ""]);
        assert!(matches!(
            handle_jsonrpc(&mut host, "UpdateLogin", Some(&params)),
            Err(RpcError::InvalidMessage(_))
        ));
        // Unknown uuid �?EntryNotFound.
        let params = json!([
            login_write("X", "u", "p", &["https://x.example.com"]),
            "e-999",
            5,
            "vault.kdbx",
        ]);
        assert_eq!(
            handle_jsonrpc(&mut host, "UpdateLogin", Some(&params)),
            Err(RpcError::EntryNotFound)
        );
    }

    #[test]
    fn envelope_wire_field_names_match_protocol() {
        let envelope = Envelope {
            protocol: "setup".to_owned(),
            srp: Some(SrpMessage {
                stage: Some("proofToClient".to_owned()),
                m2: Some("m2value".to_owned()),
                ..Default::default()
            }),
            key: None,
            jsonrpc: None,
            error: None,
            version: 0x010804,
            features: Some(FEATURES.iter().map(|f| f.to_string()).collect()),
            client_type_id: None,
            client_display_name: None,
            client_display_description: None,
        };
        let v = serde_json::to_value(&envelope).unwrap();
        assert_eq!(v["srp"]["stage"], "proofToClient");
        assert_eq!(v["srp"]["M2"], "m2value");
        assert_eq!(v["version"], 0x010804);
        assert_eq!(v["features"][1], "KPRPC_GENERAL_CLIENTS");
        assert!(v.get("key").unwrap().is_null());

        let msg: Envelope = serde_json::from_value(v).unwrap();
        assert_eq!(msg.srp.unwrap().m2.unwrap(), "m2value");
    }
}
