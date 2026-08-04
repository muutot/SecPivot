//! Windows DPAPI protection for sensitive config fields.
//!
//! On Windows, values are encrypted with `CryptProtectData` (current-user
//! scope, non-exportable) and persisted as `dpapi1:<base64>`. Other platforms
//! store plaintext (no protection available). Decryption accepts legacy
//! plaintext values, so old config files keep working.

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;

const PREFIX: &str = "dpapi1:";

/// Encrypt a config secret for disk persistence. Windows only; other
/// platforms return the plaintext unchanged.
pub fn encrypt(plain: &str) -> Result<String, String> {
    if plain.is_empty() {
        return Ok(String::new());
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Cryptography::{CryptProtectData, CRYPT_INTEGER_BLOB};

        let bytes = plain.as_bytes();
        let input = CRYPT_INTEGER_BLOB {
            cbData: bytes.len() as u32,
            pbData: bytes.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB {
            cbData: 0,
            pbData: std::ptr::null_mut(),
        };
        // SAFETY: both blobs point at valid memory for the call duration;
        // `output` is filled by the API and freed below.
        let ok = unsafe {
            CryptProtectData(
                &input,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                &mut output,
            )
        };
        if ok == 0 {
            return Err("DPAPI 加密失败".into());
        }
        // SAFETY: on success `output` references memory owned by the API,
        // sized by `cbData` and released via `LocalFree`.
        let cipher = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) };
        let encoded = BASE64.encode(cipher);
        unsafe {
            LocalFree(output.pbData as _);
        }
        Ok(format!("{PREFIX}{encoded}"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        Ok(plain.to_owned())
    }
}

/// Decrypt a value previously produced by [`encrypt`]. Legacy plaintext and
/// empty values pass through unchanged. Wrapped `dpapi1:` layers are unwrapped
/// repeatedly — including values that were accidentally double-encrypted —
/// until real plaintext (or an undecryptable layer) is reached. Any layer that
/// cannot be decrypted (a blob protected by a different user/machine,
/// corrupted, or a Windows blob read on a non-Windows platform) yields an
/// *empty* string, never the raw blob, which would otherwise be used verbatim
/// as the credential and break SigV4 signing (the base64 blob can contain
/// `/`, which malforms the `/`-delimited `X-Amz-Credential`).
pub fn decrypt(stored: &str) -> String {
    if stored.is_empty() {
        return String::new();
    }
    let mut current = stored.to_owned();
    // Unwrap nested layers (e.g. legacy accidental double-encryption).
    let mut depth = 0;
    while current.starts_with(PREFIX) && depth < 4 {
        current = decrypt_one(&current);
        depth += 1;
    }
    current
}

/// Decrypt a single `dpapi1:` layer. Returns the plaintext on success, an
/// empty string on any failure, or the input unchanged when it is not a
/// `dpapi1:` value.
fn decrypt_one(stored: &str) -> String {
    if stored.is_empty() {
        return String::new();
    }
    if stored.strip_prefix(PREFIX).is_some() {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::Foundation::LocalFree;
            use windows_sys::Win32::Security::Cryptography::{
                CryptUnprotectData, CRYPT_INTEGER_BLOB,
            };

            let Some(encoded) = stored.strip_prefix(PREFIX) else {
                return String::new();
            };
            let Ok(cipher) = BASE64.decode(encoded) else {
                return String::new();
            };
            let input = CRYPT_INTEGER_BLOB {
                cbData: cipher.len() as u32,
                pbData: cipher.as_ptr() as *mut u8,
            };
            let mut output = CRYPT_INTEGER_BLOB {
                cbData: 0,
                pbData: std::ptr::null_mut(),
            };
            // SAFETY: `input` is valid for the call; `output` is API-owned.
            let ok = unsafe {
                CryptUnprotectData(
                    &input,
                    std::ptr::null_mut(),
                    std::ptr::null(),
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    &mut output,
                )
            };
            if ok == 0 {
                return String::new();
            }
            // SAFETY: plain-owned buffer sized by `cbData`.
            let plain = unsafe {
                std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
            };
            unsafe {
                LocalFree(output.pbData as _);
            }
            String::from_utf8_lossy(&plain).into_owned()
        }
        #[cfg(not(target_os = "windows"))]
        {
            String::new()
        }
    } else {
        stored.to_owned()
    }
}

/// Prepare a secret for disk persistence, guarding against accidental
/// double-encryption: if `value` is already a `dpapi1:` blob (e.g. a stale
/// config echoed back through the frontend), it is unwrapped first so the
/// stored result is a single clean layer. Values that cannot be unwrapped
/// become empty (cleared) rather than being re-encrypted as garbage.
pub fn encrypt_for_storage(value: &str) -> Result<String, String> {
    let plain = if value.starts_with(PREFIX) {
        decrypt(value)
    } else {
        value.to_owned()
    };
    encrypt(&plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_recovers_plaintext() {
        let encrypted = encrypt("s3cret-key").unwrap();
        assert_eq!(decrypt(&encrypted), "s3cret-key");
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(encrypt("").unwrap(), "");
        assert_eq!(decrypt(""), "");
    }

    #[test]
    fn legacy_plaintext_passes_through() {
        assert_eq!(decrypt("AKIA-legacy-plain"), "AKIA-legacy-plain");
    }

    #[test]
    fn undecryptable_blob_returns_empty_not_raw() {
        assert_eq!(decrypt("dpapi1:bm90LWEtcmVhbC1ibG9i"), "");
        assert_eq!(decrypt("dpapi1:###not-valid-base64###"), "");
    }

    #[test]
    fn encrypt_for_storage_never_double_wraps() {
        // A valid plaintext secret is wrapped exactly once.
        let one = encrypt_for_storage("AKIA-secret").unwrap();
        assert!(one.starts_with(PREFIX));
        assert_eq!(decrypt(&one), "AKIA-secret");
        // An already-wrapped value is unwrapped then re-wrapped, never layered.
        let twice = encrypt_for_storage(&one).unwrap();
        assert!(twice.starts_with(PREFIX));
        assert_eq!(decrypt(&twice), "AKIA-secret");
        // DPAPI salts per call, so bytes differ — but it must stay a single
        // layer (a nested wrap would reveal another `dpapi1:` on first unwrap).
        assert!(!decrypt(&twice).starts_with(PREFIX));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_persists_encrypted_blob() {
        let encrypted = encrypt("super-secret").unwrap();
        assert!(encrypted.starts_with(PREFIX));
        assert!(!encrypted.contains("super-secret"));
        assert_eq!(decrypt(&encrypted), "super-secret");
    }
}
