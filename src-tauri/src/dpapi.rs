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
/// empty values pass through unchanged; corrupted blobs are returned as-is so
/// the config still loads (the user can re-enter the secret).
pub fn decrypt(stored: &str) -> String {
    if stored.is_empty() {
        return String::new();
    }
    if let Some(encoded) = stored.strip_prefix(PREFIX) {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::Foundation::LocalFree;
            use windows_sys::Win32::Security::Cryptography::{
                CryptUnprotectData, CRYPT_INTEGER_BLOB,
            };

            let Ok(cipher) = BASE64.decode(encoded) else {
                return stored.to_owned();
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
                return stored.to_owned();
            }
            // SAFETY: API-owned buffer sized by `cbData`.
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
            stored.to_owned()
        }
    } else {
        stored.to_owned()
    }
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

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_persists_encrypted_blob() {
        let encrypted = encrypt("super-secret").unwrap();
        assert!(encrypted.starts_with(PREFIX));
        assert!(!encrypted.contains("super-secret"));
        assert_eq!(decrypt(&encrypted), "super-secret");
    }
}
