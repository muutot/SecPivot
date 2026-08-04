//! Credential-store commands (Windows Hello quick unlock; extracted from
//! commands.rs).

use crate::credential;
use zeroize::Zeroize;
/// Store the master password for a vault path in the OS credential store.
#[tauri::command]
pub(crate) fn remember_credential(path: String, mut password: String) -> Result<(), String> {
    let result = credential::remember(&path, &password);
    password.zeroize();
    result
}

/// Fetch the stored master password for a vault path, if any.
#[tauri::command]
pub(crate) fn get_saved_credential(path: String) -> Result<Option<String>, String> {
    credential::get(&path)
}

/// Remove the stored master password for a vault path.
#[tauri::command]
pub(crate) fn clear_saved_credential(path: String) -> Result<(), String> {
    credential::forget(&path)
}
