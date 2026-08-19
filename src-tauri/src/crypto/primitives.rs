//! Shared cryptographic primitives for the loopback protocols (KeePassHttp
//! bridge, KeePassRPC) and vault hashing. Both protocols used to re-implement
//! the same AES-256-CBC + PKCS7 / HMAC / hex / base64 stack; this module is the
//! single source so a fix lands once. The AES/CBC half is desktop-only (the
//! bridge/RPC servers do not exist on mobile); hashing/base64 stay shared.

#[cfg(desktop)]
use aes::Aes256;
use base64::{engine::general_purpose::STANDARD, Engine as _};
#[cfg(desktop)]
use block_padding::Pkcs7;
#[cfg(desktop)]
use cbc::{Decryptor, Encryptor};
#[cfg(desktop)]
use cipher::{generic_array::GenericArray, BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Digest, Sha256};

/// AES-256 key length in bytes.
#[cfg(desktop)]
pub const KEY_LEN: usize = 32;
/// AES-CBC IV / nonce length in bytes.
#[cfg(desktop)]
pub const NONCE_LEN: usize = 16;

#[cfg(desktop)]
type Aes256CbcEnc = Encryptor<Aes256>;
#[cfg(desktop)]
type Aes256CbcDec = Decryptor<Aes256>;

/// CSPRNG bytes from the OS.
pub fn random_bytes(len: usize) -> Vec<u8> {
    let mut buf = vec![0u8; len];
    getrandom::getrandom(&mut buf).expect("OS RNG must be available");
    buf
}

/// AES-256-CBC encrypt with PKCS7 padding (raw bytes). `key` must be 32 bytes,
/// `iv` 16 bytes.
#[cfg(desktop)]
pub fn aes_cbc_encrypt(key: &[u8], iv: &[u8], plaintext: &[u8]) -> Vec<u8> {
    Aes256CbcEnc::new(GenericArray::from_slice(key), GenericArray::from_slice(iv))
        .encrypt_padded_vec_mut::<Pkcs7>(plaintext)
}

/// AES-256-CBC decrypt with PKCS7 padding (raw bytes).
#[cfg(desktop)]
pub fn aes_cbc_decrypt(key: &[u8], iv: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, String> {
    Aes256CbcDec::new(GenericArray::from_slice(key), GenericArray::from_slice(iv))
        .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
        .map_err(|_| "解密失败".to_owned())
}

/// AES-256-CBC encrypt a string, base64-encoded for the wire.
#[cfg(desktop)]
pub fn aes_cbc_encrypt_b64(key: &[u8], iv: &[u8], plaintext: &str) -> String {
    STANDARD.encode(aes_cbc_encrypt(key, iv, plaintext.as_bytes()))
}

/// Decrypt a base64 AES-256-CBC value back into a string.
#[cfg(desktop)]
pub fn aes_cbc_decrypt_b64(key: &[u8], iv: &[u8], encoded: &str) -> Result<String, String> {
    let ciphertext = STANDARD
        .decode(encoded)
        .map_err(|_| "加密字段格式无效".to_owned())?;
    let plaintext = aes_cbc_decrypt(key, iv, &ciphertext)?;
    String::from_utf8(plaintext).map_err(|_| "解密内容不是有效文本".to_owned())
}

/// HMAC-SHA256 (accepts any key length).
pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// SHA-1 digest of raw bytes.
pub fn sha1_bytes(input: &[u8]) -> [u8; 20] {
    let mut hasher = Sha1::new();
    hasher.update(input);
    hasher.finalize().into()
}

/// SHA-256 digest of raw bytes.
pub fn sha256_bytes(input: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(input);
    hasher.finalize().into()
}

/// SHA-256 of a string, lowercase hex.
pub fn sha256_hex(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex(&hasher.finalize())
}

/// Lowercase hex encoding of raw bytes.
pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// Standard base64 encode.
pub fn b64_encode(data: &[u8]) -> String {
    STANDARD.encode(data)
}

/// Standard base64 decode.
pub fn b64_decode(data: &str) -> Result<Vec<u8>, String> {
    STANDARD.decode(data).map_err(|_| "base64 无效".to_owned())
}

/// Random bytes, lowercase hex.
pub fn random_hex(len: usize) -> String {
    hex(&random_bytes(len))
}

/// Constant-time-ish equality for string digests/MACs.
pub fn mac_eq(a: &str, b: &str) -> bool {
    let a = a.as_bytes();
    let b = b.as_bytes();
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aes_cbc_round_trips_through_b64() {
        let key = [0x2a; KEY_LEN];
        let iv = [0x11; NONCE_LEN];
        let encoded = aes_cbc_encrypt_b64(&key, &iv, "hello 世界");
        assert_eq!(
            aes_cbc_decrypt_b64(&key, &iv, &encoded).unwrap(),
            "hello 世界"
        );
        // A wrong key must not decrypt (padding oracle surfaces an error).
        let other_key = [0x2b; KEY_LEN];
        assert!(aes_cbc_decrypt_b64(&other_key, &iv, &encoded).is_err());
        // Tampering with the ciphertext must fail decryption.
        let mut tampered = b64_decode(&encoded).unwrap();
        tampered[0] ^= 0xff;
        let tampered_b64 = b64_encode(&tampered);
        assert!(aes_cbc_decrypt_b64(&key, &iv, &tampered_b64).is_err());
    }

    #[test]
    fn hmac_sha256_matches_known_vector() {
        // RFC 4231 test case 1 (SHA-256 key/data variants).
        let key = [0x0b; 20];
        let data = b"Hi There";
        let digest = hmac_sha256(&key, data);
        let expected = [
            0xb0u8, 0x34, 0x4c, 0x61, 0xd8, 0xdb, 0x38, 0x53, 0x5c, 0xa8, 0xaf, 0xce, 0xaf, 0x0b,
            0xf1, 0x2b, 0x88, 0x1d, 0xc2, 0x00, 0xc9, 0x83, 0x3d, 0xa7, 0x26, 0xe9, 0x37, 0x6c,
            0x2e, 0x32, 0xcf, 0xf7,
        ];
        assert_eq!(digest, expected);
    }

    #[test]
    fn hex_and_base64_are_stable() {
        assert_eq!(hex(&[0x00, 0xab, 0xff]), "00abff");
        assert_eq!(b64_encode(b"hello"), "aGVsbG8=");
        assert_eq!(b64_decode("aGVsbG8=").unwrap(), b"hello");
        assert!(b64_decode("###not-base64###").is_err());
    }

    #[test]
    fn sha1_sha256_vectors() {
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            hex(&sha1_bytes(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    #[test]
    fn mac_eq_is_constant_length_safe_and_correct() {
        assert!(mac_eq("abc", "abc"));
        assert!(!mac_eq("abc", "abd"));
        assert!(!mac_eq("abc", "abcd"));
    }
}
