//! KeeOtp-compatible one-time password primitives: RFC 6238 TOTP, RFC 4226
//! HOTP, and the Steam Guard TOTP variant. Pure and deterministic so the code
//! at a given counter/time is unit-testable against published vectors. No
//! secrets are logged or persisted here; the seed travels only inside the
//! vault session and the returned code is what the UI copies.
//!
//! Field contract (custom fields read by the vault session):
//!
//! - `otp` or `TimeOtp` — TOTP seed: an `otpauth://totp/...` URI (honoring
//!   `secret`, `algorithm`, `digits`, `period`) or a bare Base32 key (RFC 6238
//!   defaults: SHA-1, 6 digits, 30 s). Backward compatible with KeePassXC /
//!   KeeWeb and with SecPivot's previous `otp` field.
//! - `HmacOtp` — HOTP seed: `<Base32>[:<digits>][:<counter>]` (6 digits, 0
//!   counter by default). The counter advances on each code request and is
//!   written back to the same field server-side.
//! - `SteamOtp` or `steam` — Steam Guard seed: a bare Base32 key; the code is
//!   SHA-1 TOTP over a 30 s period, truncated to a 5-character Steam alphabet.

use hmac::{Hmac, Mac};
use sha1::Sha1;
use sha2::{Sha256, Sha512};

const BASE32_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
/// Steam Guard uses this 26-character alphabet (digits, then un-ambiguous
/// letters) instead of the base-10 code shown by ordinary TOTP.
const STEAM_ALPHABET: &str = "23456789BCDFGHJKMNPQRTVWXY";

const DEFAULT_DIGITS: u32 = 6;
const DEFAULT_PERIOD: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgo {
    Sha1,
    Sha256,
    Sha512,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtpKind {
    Totp,
    Hotp,
    Steam,
}

/// A parsed, ready-to-compute OTP configuration. `secret` holds the raw key
/// bytes; everything else drives the derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpSpec {
    pub kind: OtpKind,
    pub secret: Vec<u8>,
    pub algorithm: HashAlgo,
    pub digits: u32,
    pub period: u64,
    /// HOTP moving factor (0 for time-based kinds). The vault keeps the
    /// current value in the entry and advances it on each use.
    pub counter: u64,
}

/// A computed code plus the pieces the UI needs for a countdown. For HOTP
/// `valid_for`/`period` are 0 (no wall-clock schedule) and `counter` is the
/// value that produced this code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OtpCode {
    pub code: String,
    pub kind: OtpKind,
    pub valid_for: u64,
    pub period: u64,
    pub counter: Option<u64>,
}

/// HMAC over `data` with `key` using the selected hash.
fn hmac(key: &[u8], data: &[u8], algo: HashAlgo) -> Vec<u8> {
    match algo {
        HashAlgo::Sha1 => {
            let mut mac = Hmac::<Sha1>::new_from_slice(key).expect("HMAC accepts any key length");
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }
        HashAlgo::Sha256 => {
            let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts any key length");
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }
        HashAlgo::Sha512 => {
            let mut mac = Hmac::<Sha512>::new_from_slice(key).expect("HMAC accepts any key length");
            mac.update(data);
            mac.finalize().into_bytes().to_vec()
        }
    }
}

/// The 31-bit dynamic-truncation value from RFC 4226 §5.3.
fn dynamic_truncate(hash: &[u8]) -> u32 {
    let offset = (hash[hash.len() - 1] & 0x0f) as usize;
    let slice = &hash[offset..offset + 4];
    (u32::from(slice[0]) & 0x7f) << 24
        | u32::from(slice[1]) << 16
        | u32::from(slice[2]) << 8
        | u32::from(slice[3])
}

/// The big-endian 8-byte counter block that both TOTP and HOTP sign.
fn counter_block(counter: u64) -> [u8; 8] {
    counter.to_be_bytes()
}

fn pad_code(binary: u32, digits: u32) -> String {
    let modulo = 10u64.checked_pow(digits).unwrap_or(u64::MAX);
    format!(
        "{:0width$}",
        (binary as u64) % modulo,
        width = digits as usize
    )
}

/// Maximum decimal code length: the 31-bit dynamic-truncation value (RFC 4226
/// §5.3) cannot reach a 10-digit modulus, and zero digits would emit an empty
/// code. Seeds declaring anything outside `1..=MAX_CODE_DIGITS` are rejected
/// instead of silently producing a wrong or empty code.
const MAX_CODE_DIGITS: u32 = 9;

fn validate_digits(kind: OtpKind, digits: u32) -> Result<(), String> {
    if digits == 0 {
        return Err(match kind {
            OtpKind::Totp => "TOTP 位数不能为 0".to_owned(),
            OtpKind::Hotp => "HOTP 位数不能为 0".to_owned(),
            OtpKind::Steam => "Steam 位数不能为 0".to_owned(),
        });
    }
    if digits > MAX_CODE_DIGITS {
        return Err(format!("OTP 位数不能超过 {MAX_CODE_DIGITS}"));
    }
    Ok(())
}

/// Map a truncated 31-bit value onto the Steam alphabet (5 rolls, mod 26).
fn steam_string(binary: u32) -> String {
    let mut value = binary;
    let mut out = String::with_capacity(5);
    for _ in 0..5 {
        let idx = (value % STEAM_ALPHABET.len() as u32) as usize;
        out.push(STEAM_ALPHABET.as_bytes()[idx] as char);
        value /= STEAM_ALPHABET.len() as u32;
    }
    out
}

/// Decode a Base32 (RFC 4648) secret. Whitespace and hyphens are tolerated.
fn decode_base32(input: &str) -> Result<Vec<u8>, String> {
    let clean: String = input
        .to_ascii_uppercase()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '=')
        .collect();
    let mut bits = 0u32;
    let mut acc = 0u32;
    let mut out = Vec::with_capacity(clean.len() * 5 / 8);
    for ch in clean.chars() {
        let Some(index) = BASE32_ALPHABET.find(ch) else {
            return Err(format!("Base32 字符无效: {ch}"));
        };
        acc = (acc << 5) | index as u32;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    if out.is_empty() {
        return Err("OTP 密钥为空".to_owned());
    }
    Ok(out)
}

/// Strip a trailing `=` padding and whitespace from a bare Base32 secret.
fn clean_secret(input: &str) -> String {
    input
        .trim()
        .replace([' ', '-', '='], "")
        .to_ascii_uppercase()
}

/// Detect the algorithm tokens KeePass and otpauth use into a `HashAlgo`.
fn hash_from_alg(value: &str) -> HashAlgo {
    match value.trim().to_ascii_uppercase().as_str() {
        "SHA256" => HashAlgo::Sha256,
        "SHA512" => HashAlgo::Sha512,
        _ => HashAlgo::Sha1,
    }
}

fn parse_otpauth_uri(uri: &str) -> Result<OtpSpec, String> {
    let url = uri
        .parse::<url::Url>()
        .map_err(|_| "TOTP URI 无效".to_owned())?;
    if url.scheme() != "otpauth" || url.host_str() != Some("totp") {
        return Err("仅支持 otpauth://totp/ URI".to_owned());
    }
    let secret = url
        .query_pairs()
        .find(|(k, _)| k == "secret")
        .map(|(_, v)| v.clone())
        .ok_or_else(|| "TOTP URI 缺少 secret".to_owned())?;
    let digits = url
        .query_pairs()
        .find(|(k, _)| k == "digits")
        .map(|(_, v)| v.parse::<u32>().unwrap_or(DEFAULT_DIGITS))
        .unwrap_or(DEFAULT_DIGITS);
    let period = url
        .query_pairs()
        .find(|(k, _)| k == "period")
        .map(|(_, v)| v.parse::<u64>().unwrap_or(DEFAULT_PERIOD))
        .unwrap_or(DEFAULT_PERIOD);
    let algorithm = url
        .query_pairs()
        .find(|(k, _)| k == "algorithm")
        .map(|(_, v)| hash_from_alg(&v))
        .unwrap_or(HashAlgo::Sha1);
    Ok(OtpSpec {
        kind: OtpKind::Totp,
        secret: decode_base32(&secret)?,
        algorithm,
        digits,
        period,
        counter: 0,
    })
}

/// Parse a TOTP seed field (`otp`/`TimeOtp`): an otpauth URI or a bare key.
pub fn parse_totp_seed(value: &str) -> Result<OtpSpec, String> {
    let trimmed = value.trim();
    if trimmed.to_ascii_lowercase().starts_with("otpauth://") {
        return parse_otpauth_uri(trimmed);
    }
    Ok(OtpSpec {
        kind: OtpKind::Totp,
        secret: decode_base32(&clean_secret(trimmed))?,
        algorithm: HashAlgo::Sha1,
        digits: DEFAULT_DIGITS,
        period: DEFAULT_PERIOD,
        counter: 0,
    })
}

/// Parse an `HmacOtp` field: `<base32>[:<digits>][:<counter>]`.
pub fn parse_hotp_seed(value: &str) -> Result<OtpSpec, String> {
    let parts: Vec<&str> = value.split(':').collect();
    let key = parts.first().copied().unwrap_or("");
    let digits = parts
        .get(1)
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(DEFAULT_DIGITS);
    let counter = parts
        .get(2)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    Ok(OtpSpec {
        kind: OtpKind::Hotp,
        secret: decode_base32(&clean_secret(key))?,
        algorithm: HashAlgo::Sha1,
        digits,
        period: 0,
        counter,
    })
}

/// Serialize an `HmacOtp` field (Base32 key retained verbatim, digits kept,
/// counter rewritten) so a counter advance writes the whole seed back.
pub fn render_hotp_seed(spec: &OtpSpec) -> String {
    let key = base32_encode(&spec.secret);
    if spec.digits != DEFAULT_DIGITS {
        format!("{key}:{}:{}", spec.digits, spec.counter)
    } else {
        format!("{key}::{}", spec.counter)
    }
}

pub fn base32_encode(bytes: &[u8]) -> String {
    let mut bits = 0u32;
    let mut acc = 0u32;
    let mut out = String::with_capacity((bytes.len() * 8).div_ceil(5));
    for byte in bytes {
        acc = (acc << 8) | u32::from(*byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(BASE32_ALPHABET.as_bytes()[((acc >> bits) & 0x1f) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(BASE32_ALPHABET.as_bytes()[((acc << (5 - bits)) & 0x1f) as usize] as char);
    }
    out
}

/// Parse a Steam guard seed field (`SteamOtp` / `steam`).
pub fn parse_steam_seed(value: &str) -> Result<OtpSpec, String> {
    Ok(OtpSpec {
        kind: OtpKind::Steam,
        secret: decode_base32(&clean_secret(value))?,
        algorithm: HashAlgo::Sha1,
        digits: 5,
        period: DEFAULT_PERIOD,
        counter: 0,
    })
}

/// Compute the code for a spec at a wall-clock moment (`unix_time` seconds
/// since the epoch). HOTP ignores the clock and only uses `spec.counter`.
pub fn compute(spec: &OtpSpec, unix_time: u64) -> Result<OtpCode, String> {
    match spec.kind {
        OtpKind::Totp | OtpKind::Steam => {
            validate_digits(spec.kind, spec.digits)?;
            if spec.period == 0 {
                return Err("OTP 周期不能为 0".to_owned());
            }
            let counter = unix_time / spec.period;
            let hash = hmac(&spec.secret, &counter_block(counter), spec.algorithm);
            let binary = dynamic_truncate(&hash);
            let code = if spec.kind == OtpKind::Steam {
                steam_string(binary)
            } else {
                pad_code(binary, spec.digits)
            };
            Ok(OtpCode {
                code,
                kind: spec.kind,
                valid_for: spec.period - (unix_time % spec.period),
                period: spec.period,
                counter: None,
            })
        }
        OtpKind::Hotp => {
            validate_digits(spec.kind, spec.digits)?;
            let hash = hmac(&spec.secret, &counter_block(spec.counter), spec.algorithm);
            let binary = dynamic_truncate(&hash);
            Ok(OtpCode {
                code: pad_code(binary, spec.digits),
                kind: OtpKind::Hotp,
                valid_for: 0,
                period: 0,
                counter: Some(spec.counter),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 6238 Appendix B — ASCII secrets and step 0..=9 codes.
    /// Each row is (secret_bytes, algorithm, times, expected 8-digit codes).
    type AppendixRow = (
        &'static [u8],
        &'static str,
        &'static [u64],
        &'static [&'static str],
    );
    const APPENDIX_B: [AppendixRow; 3] = [
        (
            b"12345678901234567890",
            "SHA1",
            &[
                59,
                1111111109,
                1111111111,
                1234567890,
                2000000000,
                20000000000,
            ],
            &[
                "94287082", "07081804", "14050471", "89005924", "69279037", "65353130",
            ],
        ),
        (
            b"12345678901234567890123456789012",
            "SHA256",
            &[
                59,
                1111111109,
                1111111111,
                1234567890,
                2000000000,
                20000000000,
            ],
            &[
                "46119246", "68084774", "67062674", "91819424", "90698825", "77737706",
            ],
        ),
        (
            b"1234567890123456789012345678901234567890123456789012345678901234",
            "SHA512",
            &[
                59,
                1111111109,
                1111111111,
                1234567890,
                2000000000,
                20000000000,
            ],
            &[
                "90693936", "25091201", "99943326", "93441116", "38618901", "47863826",
            ],
        ),
    ];

    #[test]
    fn totp_matches_rfc6238_appendix_b_vectors() {
        for (secret, alg, times, codes) in APPENDIX_B {
            for (i, t) in times.iter().enumerate() {
                let spec = OtpSpec {
                    kind: OtpKind::Totp,
                    secret: secret.to_vec(),
                    algorithm: hash_from_alg(alg),
                    digits: 8,
                    period: 30,
                    counter: 0,
                };
                let out = compute(&spec, *t).unwrap();
                assert_eq!(out.code, codes[i], "{alg} at t={t}");
            }
        }
    }

    #[test]
    fn totp_defaults_to_sha1_six_digits_thirty_seconds() {
        let spec = parse_totp_seed("JBSWY3DPEHPK3PXP").unwrap();
        assert_eq!(spec.kind, OtpKind::Totp);
        assert_eq!(spec.algorithm, HashAlgo::Sha1);
        assert_eq!(spec.digits, 6);
        assert_eq!(spec.period, 30);
        let out = compute(&spec, 59).unwrap();
        assert_eq!(out.code.len(), 6);
        assert_eq!(out.valid_for, 1);
    }

    #[test]
    fn otpauth_uri_honors_algorithm_digits_and_period() {
        let spec = parse_totp_seed(
            "otpauth://totp/Acme:bob?secret=JBSWY3DPEHPK3PXP&algorithm=SHA256&digits=8&period=45",
        )
        .unwrap();
        assert_eq!(spec.algorithm, HashAlgo::Sha256);
        assert_eq!(spec.digits, 8);
        assert_eq!(spec.period, 45);
        let out = compute(&spec, 0).unwrap();
        assert_eq!(out.valid_for, 45);
    }

    #[test]
    fn otpauth_uri_lowercase_secret_is_accepted() {
        let spec = parse_totp_seed("otpauth://totp/x?secret=jbswy3dpehpk3pxp").unwrap();
        assert_eq!(spec.secret, decode_base32("JBSWY3DPEHPK3PXP").unwrap());
    }

    #[test]
    fn invalid_base32_is_rejected() {
        assert!(parse_totp_seed("####").is_err());
        assert!(parse_totp_seed("").is_err());
    }

    /// RFC 4226 Appendix D — HOTP with ASCII "12345678901234567890".
    /// Truncation test vector for counter 0 is 755224.
    #[test]
    fn hotp_matches_rfc_4226_truncation_vector() {
        let spec = OtpSpec {
            kind: OtpKind::Hotp,
            secret: b"12345678901234567890".to_vec(),
            algorithm: HashAlgo::Sha1,
            digits: 6,
            period: 0,
            counter: 0,
        };
        let out = compute(&spec, 0).unwrap();
        assert_eq!(out.code, "755224");
        assert_eq!(out.counter, Some(0));
        assert_eq!(out.valid_for, 0);
    }

    #[test]
    fn hotp_advances_by_counter() {
        let spec = OtpSpec {
            kind: OtpKind::Hotp,
            secret: b"12345678901234567890".to_vec(),
            algorithm: HashAlgo::Sha1,
            digits: 6,
            period: 0,
            counter: 0,
        };
        let c0 = compute(&spec, 0).unwrap();
        let mut next = spec.clone();
        next.counter = 1;
        let c1 = compute(&next, 0).unwrap();
        assert_eq!(c0.code, "755224");
        assert_eq!(c1.code, "287082");
    }

    #[test]
    fn hotp_seed_parses_digits_and_counter() {
        let spec = parse_hotp_seed("JBSWY3DPEHPK3PXP:8:12").unwrap();
        assert_eq!(spec.digits, 8);
        assert_eq!(spec.counter, 12);
        assert_eq!(render_hotp_seed(&spec), "JBSWY3DPEHPK3PXP:8:12");
    }

    #[test]
    fn hotp_seed_defaults_and_roundtrips_through_render() {
        let spec = parse_hotp_seed("JBSWY3DPEHPK3PXP").unwrap();
        assert_eq!(spec.digits, 6);
        assert_eq!(spec.counter, 0);
        let rendered = render_hotp_seed(&spec);
        assert_eq!(rendered, "JBSWY3DPEHPK3PXP::0");
        assert_eq!(parse_hotp_seed(&rendered).unwrap().counter, 0);
    }

    #[test]
    fn oversized_or_zero_digits_are_rejected_instead_of_miscomputed() {
        let totp =
            parse_totp_seed("otpauth://totp/Acme:bob?secret=JBSWY3DPEHPK3PXP&digits=10").unwrap();
        assert!(
            compute(&totp, 0).is_err(),
            "digits beyond the 31-bit truncation range must error"
        );
        let hotp = parse_hotp_seed("JBSWY3DPEHPK3PXP:10:0").unwrap();
        assert!(compute(&hotp, 0).is_err());
        let zero = parse_hotp_seed("JBSWY3DPEHPK3PXP:0:0").unwrap();
        assert!(compute(&zero, 0).is_err());
        // The maximum representable length still works.
        let nine = OtpSpec {
            kind: OtpKind::Totp,
            secret: decode_base32("JBSWY3DPEHPK3PXP").unwrap(),
            algorithm: HashAlgo::Sha1,
            digits: 9,
            period: 30,
            counter: 0,
        };
        assert_eq!(compute(&nine, 0).unwrap().code.len(), 9);
    }

    #[test]
    fn steam_code_is_five_chars_from_steam_alphabet() {
        let spec = parse_steam_seed("CNBNMZBN").unwrap();
        assert_eq!(spec.kind, OtpKind::Steam);
        assert_eq!(spec.digits, 5);
        assert_eq!(spec.period, 30);
        let out = compute(&spec, 1_700_000_000).unwrap();
        assert!(
            out.code.len() == 5,
            "steam code must be 5 chars: {}",
            out.code
        );
        assert!(out.code.chars().all(|c| STEAM_ALPHABET.contains(c)));
        assert_eq!(out.valid_for, 30 - (1_700_000_000 % 30));
    }

    #[test]
    fn base32_roundtrip_known_vector() {
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert_eq!(decode_base32("MZXW6YTBOI").unwrap(), b"foobar");
    }

    #[test]
    fn base32_ignores_whitespace_dashes_and_padding() {
        assert_eq!(decode_base32("JB S WY3=DP-EHPK-3PXP=").unwrap(), {
            let spec = parse_totp_seed("JBSWY3DPEHPK3PXP").unwrap();
            spec.secret
        });
    }
}
