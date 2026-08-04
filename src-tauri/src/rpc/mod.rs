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

mod dispatch;
mod dto;
mod keys;
mod srp;
#[cfg(test)]
mod tests;

pub use self::dispatch::*;
pub use self::dto::*;
pub use self::keys::*;
pub use self::srp::*;
