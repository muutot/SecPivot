//! Clipboard read/clear IPC commands (extracted from commands.rs), plus the
//! backend-enforced scheduled wipe: a safety net that clears copied secrets
//! even if the webview process dies before the renderer's JS timer fires.
//!
//! The wipe thread holds a zeroizing copy of the secret, sleeps for the
//! configured interval, then verifies the clipboard still holds *our* text
//! before clearing — the app must never destroy content the user copied in
//! another app in the meantime. A newer schedule (or an explicit cancel)
//! bumps a generation counter so superseded threads exit without touching
//! the clipboard.

use crate::platform::clipboard;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use zeroize::Zeroize;
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

/// Bumped on every new wipe schedule and on explicit cancel; a sleeping wipe
/// thread whose generation is no longer current was superseded and must not
/// touch the clipboard.
static WIPE_GENERATION: AtomicU64 = AtomicU64::new(0);

/// Cancel every pending scheduled wipe (called on explicit clear / lock).
#[tauri::command]
pub(crate) fn clipboard_cancel_scheduled_wipe() {
    WIPE_GENERATION.fetch_add(1, Ordering::SeqCst);
}

/// Backend safety net for the renderer's scheduled wipe: keep a zeroizing
/// copy of the copied text and clear the clipboard after `seconds` if it
/// still holds exactly that text. The renderer timer stays primary (faster
/// and works in the browser demo); this survives webview death. Scheduling
/// with `seconds == 0` is a no-op (clear-on-copy disabled).
#[tauri::command]
pub(crate) fn clipboard_schedule_wipe(text: String, seconds: u64) -> Result<(), String> {
    if seconds == 0 {
        return Ok(());
    }
    let generation = WIPE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let mut secret = text;
    std::thread::Builder::new()
        .name("clipboard-wipe".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(seconds));
            if WIPE_GENERATION.load(Ordering::SeqCst) != generation {
                // Superseded by a newer copy or an explicit cancel.
                secret.zeroize();
                return;
            }
            let still_ours = match clipboard::read_clipboard_text() {
                Ok(Some(current)) => current == secret,
                _ => false,
            };
            if still_ours {
                let _ = clipboard::clear_clipboard();
            }
            secret.zeroize();
        })
        .map_err(|e| format!("无法创建剪贴板清除任务: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::clipboard::write_clipboard_text;

    /// The Windows clipboard is a process-wide resource shared with the
    /// platform tests; serialize to avoid cross-test interference.
    static CLIPBOARD_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn scheduled_wipe_clears_our_own_text() {
        let _guard = CLIPBOARD_LOCK.lock().unwrap();
        let _ = write_clipboard_text("secpivot-scheduled-wipe");
        clipboard_schedule_wipe("secpivot-scheduled-wipe".into(), 1).unwrap();
        std::thread::sleep(Duration::from_millis(1800));
        match clipboard::read_clipboard_text() {
            // Cleared, or the station has no interactive clipboard (CI): both fine.
            Ok(Some(text)) => assert_ne!(text, "secpivot-scheduled-wipe"),
            Ok(None) => {}
            Err(e) => panic!("unexpected clipboard error: {e}"),
        }
    }

    #[test]
    fn superseded_wipe_never_touches_the_clipboard() {
        let _guard = CLIPBOARD_LOCK.lock().unwrap();
        let _ = write_clipboard_text("keep-me");
        // First job owns "other-text"; the second schedule supersedes it.
        clipboard_schedule_wipe("other-text".into(), 1).unwrap();
        clipboard_schedule_wipe("never-matches-keep-me".into(), 1).unwrap();
        std::thread::sleep(Duration::from_millis(1800));
        match clipboard::read_clipboard_text() {
            Ok(Some(text)) => assert_eq!(text, "keep-me", "superseded wipe must not clear"),
            Ok(None) => {}
            Err(e) => panic!("unexpected clipboard error: {e}"),
        }
        let _ = clipboard::clear_clipboard();
    }

    #[test]
    fn zero_seconds_is_a_no_op() {
        assert!(clipboard_schedule_wipe("x".into(), 0).is_ok());
    }
}
