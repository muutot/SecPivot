//! Remote-profile credential protection for `config.json`.
//!
//! Windows: values are DPAPI-encrypted inline (unchanged legacy behavior,
//! see [`super::dpapi`]). Desktop non-Windows platforms: the secret lives in
//! the OS keyring (`SecPivot` service, entry keyed by `field_key`) and the
//! config stores only a `keyring1:<field_key>` reference, so remote
//! WebDAV/S3 credentials never touch disk in plaintext. Mobile targets have
//! no OS key store wired and keep the legacy plaintext passthrough.
//!
//! Reference semantics mirror dpapi: a reference whose entry is missing or
//! unreadable decrypts to an *empty* string, never the raw reference, and a
//! `keyring1:` value read on Windows (config carried across platforms) also
//! yields empty rather than a bogus literal credential. Legacy plaintext and
//! `dpapi1:` values still pass through on first load; the next save migrates
//! them into the keyring automatically.
//!
//! Removed profiles leave their keyring entries behind (the OS store is not
//! enumerable for one app's entries); they are overwritten on reuse of the
//! same profile name and otherwise inert.

use super::dpapi;

const PREFIX: &str = "keyring1:";
#[cfg(all(desktop, not(target_os = "windows")))]
const SERVICE: &str = "SecPivot";

/// Protect a remote credential for disk persistence under `field_key` and
/// return the value to store in `config.json`.
pub fn store(plain: &str, field_key: &str) -> Result<String, String> {
    if plain.is_empty() {
        return Ok(String::new());
    }
    #[cfg(all(desktop, not(target_os = "windows")))]
    {
        let entry = keyring::Entry::new(SERVICE, field_key)
            .map_err(|e| format!("初始化远程凭据存储失败: {e}"))?;
        entry
            .set_password(plain)
            .map_err(|e| format!("保存远程凭据失败: {e}"))?;
        Ok(format!("{PREFIX}{field_key}"))
    }
    #[cfg(any(not(desktop), target_os = "windows"))]
    {
        let _ = field_key;
        dpapi::encrypt_for_storage(plain)
    }
}

/// Recover the plaintext previously stored by [`store`]. Failure modes
/// (missing entry, locked/unavailable store, cross-platform reference)
/// decrypt to an empty string, matching [`dpapi::decrypt`] semantics.
pub fn load(stored: &str, field_key: &str) -> String {
    if stored.is_empty() {
        return String::new();
    }
    #[cfg(all(desktop, not(target_os = "windows")))]
    {
        let Some(field) = stored.strip_prefix(PREFIX) else {
            // Legacy plaintext or a foreign `dpapi1:` blob; dpapi::decrypt
            // passes legacy plaintext through and empties undecryptable blobs.
            return dpapi::decrypt(stored);
        };
        match keyring::Entry::new(SERVICE, field) {
            Ok(entry) => match entry.get_password() {
                Ok(value) => value,
                Err(_) => String::new(),
            },
            Err(_) => String::new(),
        }
    }
    #[cfg(any(not(desktop), target_os = "windows"))]
    {
        // A `keyring1:` reference cannot be resolved on this platform (the
        // entry belongs to another machine's key store): empty, never the
        // literal reference, so it can never be used verbatim as a credential.
        if stored.starts_with(PREFIX) {
            return String::new();
        }
        let _ = field_key;
        dpapi::decrypt(stored)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Store/load round trip. Guarded behind a runtime availability probe so
    /// the suite still passes in CI/headless shells without a key store.
    #[test]
    fn remote_secret_round_trip_when_store_available() {
        let key = "secpivot-test://remote-secret-round-trip";
        let stored = match store("s3cret-remote", key) {
            Ok(value) => value,
            Err(e) => {
                eprintln!("credential store unavailable, skipping: {e}");
                return;
            }
        };
        assert!(!stored.contains("s3cret-remote"));
        assert_eq!(load(&stored, key), "s3cret-remote");
    }

    #[test]
    fn empty_stays_empty() {
        assert_eq!(store("", "secpivot-test://empty").unwrap(), "");
        assert_eq!(load("", "secpivot-test://empty"), "");
    }

    /// A reference whose entry does not exist (or cannot be resolved on this
    /// platform) decrypts to empty — never the raw reference — so a stale or
    /// cross-machine value can never be used verbatim as a credential.
    #[test]
    fn unresolvable_reference_yields_empty() {
        assert_eq!(
            load(
                "keyring1:secpivot-test//no-such-entry",
                "secpivot-test://any"
            ),
            ""
        );
    }
}
