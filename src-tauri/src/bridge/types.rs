//! KeePassHttp wire shapes (exact camelCase field names) + host abstraction.
//! Extracted from `bridge::mod.rs`.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};

use super::crypto::{decrypt_field, make_verifier, response_hmac};
use super::{NONCE_LEN, PROTOCOL_VERSION};
use crate::crypto::random_bytes;
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
    pub(crate) fn failure(request_type: &str, error: &str) -> Self {
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
    pub(crate) fn success(request_type: &str, key: &[u8], host: &dyn BridgeHost) -> Self {
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
pub(crate) fn decrypt_request_field(
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
