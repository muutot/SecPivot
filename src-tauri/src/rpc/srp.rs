//! SRP-6a server-side handshake (KeePassRPC variant).
//! Extracted from `rpc::mod.rs`.
use num_bigint::BigUint;
use serde_json::{json, Value};

use super::dto::RpcError;
use super::{group, mod_pow, SECURITY_LEVEL, SRP_G, SRP_K_HEX};
use crate::crypto::{random_bytes, random_hex, sha256_hex};
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
                eprintln!("[rpc] verify_proof: M mismatch");
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
