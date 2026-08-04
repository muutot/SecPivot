//! Clipboard read/clear IPC commands (extracted from commands.rs).

use crate::platform::clipboard;
// ---------------------------------------------------------------------------
// Clipboard commands
// ---------------------------------------------------------------------------

/// Current clipboard text (or `null` when it holds non-text content).
/// Used by the scheduled wipe to avoid destroying text the user copied in
/// another app after ours.
#[tauri::command]
pub(crate) fn clipboard_read_text() -> Result<Option<String>, String> {
    clipboard::read_clipboard_text()
}

/// Empty the clipboard. The frontend calls this only after verifying the
/// clipboard still holds our own text (or on lock with `clearOnLock`).
#[tauri::command]
pub(crate) fn clipboard_clear() -> Result<(), String> {
    clipboard::clear_clipboard()
}
