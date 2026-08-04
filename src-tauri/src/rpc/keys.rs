//! Session-key handling: challenge/response auth + JSON-RPC frame crypto.
//! Extracted from `rpc::mod.rs`.
use super::dto::{JsonRpcFrame, RpcError};
use crate::crypto::{
    aes_cbc_decrypt, aes_cbc_encrypt, b64_decode, b64_encode, mac_eq, random_bytes, sha1_bytes,
    sha256_hex, KEY_LEN, NONCE_LEN,
};
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
