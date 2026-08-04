//! KeePassHttp field crypto: AES enc/decrypt + verifier + response HMAC.
//! Extracted from `bridge::mod.rs` (constants live in the module root).
use base64::{engine::general_purpose::STANDARD, Engine as _};

use super::NONCE_LEN;
use crate::crypto::{aes_cbc_decrypt_b64, aes_cbc_encrypt_b64, hmac_sha256};
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
