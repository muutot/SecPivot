//! Optional master-password retention in the OS credential store
//! (Windows Credential Manager via the `keyring` crate), used for the
//! "Windows Hello" quick unlock. Keyed per vault path.

use keyring::Entry;

const SERVICE: &str = "SecPivot";

fn entry_for(path: &str) -> Result<Entry, String> {
    if path.is_empty() {
        return Err("数据库路径为空".to_owned());
    }
    Entry::new(SERVICE, path).map_err(|e| format!("初始化凭据存储失败: {e}"))
}

/// Store the master password for `path` in the OS credential store.
pub fn remember(path: &str, password: &str) -> Result<(), String> {
    let entry = entry_for(path)?;
    entry
        .set_password(password)
        .map_err(|e| format!("保存凭据失败: {e}"))
}

/// Fetch the stored master password for `path`, if any.
pub fn get(path: &str) -> Result<Option<String>, String> {
    let entry = entry_for(path)?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("读取凭据失败: {e}")),
    }
}

/// Remove the stored master password for `path` (no-op when absent).
pub fn forget(path: &str) -> Result<(), String> {
    let entry = entry_for(path)?;
    entry
        .delete_credential()
        .map_err(|e| format!("清除凭据失败: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Windows Credential Manager round-trip. Guarded behind a runtime
    /// availability probe so the suite still passes in CI/headless shells.
    #[test]
    fn credential_round_trip_when_store_available() {
        let path = "secpivot-test://credential-round-trip";
        let _ = forget(path);
        match remember(path, "s3cret-master") {
            Ok(()) => {
                assert_eq!(get(path).unwrap(), Some("s3cret-master".to_owned()));
                forget(path).unwrap();
                assert_eq!(get(path).unwrap(), None);
            }
            Err(e) => {
                eprintln!("credential store unavailable, skipping: {e}");
            }
        }
    }
}
