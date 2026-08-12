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

pub(crate) mod server;

/// KeePassHttp default loopback port (hard-coded into chromeIPass).
pub const BRIDGE_PORT: u16 = 19455;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 16;
const PROTOCOL_VERSION: &str = "1.8.4";

mod crypto;
mod dispatch;
#[cfg(test)]
mod tests;
mod types;

pub use self::crypto::*;
pub use self::dispatch::*;
pub use self::types::*;
