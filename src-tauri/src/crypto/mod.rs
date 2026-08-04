//! Pure computation security primitives (no OS/tauri dependency).
//!
//! `primitives` holds the shared AES-256-CBC + PKCS7 / HMAC-SHA256 / SHA-1 /
//! SHA-256 / hex / base64 / CSPRNG stack (single source for bridge, rpc and
//! vault) and is re-exported here so consumers keep using `crate::crypto::*`.
//! `otp` holds the one-time-password primitives (RFC 6238 TOTP, RFC 4226 HOTP,
//! Steam Guard) and seed parsing.

pub mod otp;

mod primitives;

pub use self::primitives::*;
