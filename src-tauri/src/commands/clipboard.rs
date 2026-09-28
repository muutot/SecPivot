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

/// Whether a sleeping wipe thread may clear the clipboard.
///
/// Pure so the generation contract is unit-testable: the end-to-end version of
/// this check has to sleep for a whole second against the *real* Windows
/// clipboard, which any other process on the desktop may replace or empty
/// meanwhile — that made the previous integration test fail (and, before the
/// `restore_clipboard` helper, destroy the developer's clipboard contents).
///
/// `current` is what the clipboard holds right now; the wipe fires only when it
/// still holds exactly this thread's own secret.
fn should_wipe(
    current_generation: u64,
    scheduled_generation: u64,
    secret: &str,
    current: Option<&str>,
) -> bool {
    current_generation == scheduled_generation && current == Some(secret)
}
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
            let current = clipboard::read_clipboard_text().ok().flatten();
            if should_wipe(
                WIPE_GENERATION.load(Ordering::SeqCst),
                generation,
                &secret,
                current.as_deref(),
            ) {
                let _ = clipboard::clear_clipboard();
            }
            secret.zeroize();
        })
        .map_err(|e| format!("无法创建剪贴板清除任务: {e}"))?;
    Ok(())
}

/// Backend safety net for the KeePass entry-exchange payload
/// (`Entry → Data Exchange → Copy Entry`). That payload lives under a custom
/// clipboard format, so the text wipe above never sees it — and it carries
/// plaintext field values in *both* variants (the encrypted one only adds a
/// DPAPI wrapper), so it must be wiped just as a password is. The thread keeps
/// a zeroizing copy of what we wrote and clears only when the clipboard still
/// holds exactly those bytes, so content copied elsewhere is never destroyed.
/// Shares the text wipe's generation counter, so either schedule supersedes the
/// other.
#[tauri::command]
pub(crate) fn clipboard_schedule_exchange_wipe(seconds: u64) -> Result<(), String> {
    use crate::platform::clipboard::{clear_clipboard, read_clipboard_bytes};
    use crate::vault::exchange::CLIP_FORMAT_ENTRIES;

    if seconds == 0 {
        return Ok(());
    }
    let Some(ours) = read_clipboard_bytes(CLIP_FORMAT_ENTRIES)? else {
        // Nothing of ours on the clipboard: nothing to wipe.
        return Ok(());
    };
    let generation = WIPE_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let mut secret = ours;
    std::thread::Builder::new()
        .name("clipboard-exchange-wipe".into())
        .spawn(move || {
            std::thread::sleep(Duration::from_secs(seconds));
            if WIPE_GENERATION.load(Ordering::SeqCst) != generation {
                secret.zeroize();
                return;
            }
            let still_ours = read_clipboard_bytes(CLIP_FORMAT_ENTRIES)
                .ok()
                .flatten()
                .is_some_and(|current| current == secret);
            if still_ours {
                let _ = clear_clipboard();
            }
            secret.zeroize();
        })
        .map_err(|e| format!("无法创建剪贴板清除任务: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::clipboard::tests::{
        clipboard_is_usable, lock_clipboard, restore_clipboard,
    };
    use crate::platform::clipboard::{read_clipboard_text, write_clipboard_text};

    #[test]
    fn scheduled_wipe_clears_our_own_text() {
        let _guard = lock_clipboard();
        let saved = read_clipboard_text().ok().flatten();
        if !clipboard_is_usable("secpivot-wipe-probe") {
            eprintln!("no interactive clipboard on this station; skipping wipe assertion");
            restore_clipboard(saved);
            return;
        }
        write_clipboard_text("secpivot-scheduled-wipe").unwrap();
        clipboard_schedule_wipe("secpivot-scheduled-wipe".into(), 1).unwrap();
        // The wipe thread sleeps a full second against the *shared* Windows
        // clipboard. Re-check ownership just before sleeping so a foreign
        // clipboard change is reported as "cannot conclude" instead of being
        // misattributed to the wipe.
        if read_clipboard_text().ok().flatten().as_deref() != Some("secpivot-scheduled-wipe") {
            eprintln!("clipboard was replaced by another process; skipping wipe assertion");
            restore_clipboard(saved);
            return;
        }
        std::thread::sleep(Duration::from_millis(1800));
        let after = read_clipboard_text().ok().flatten();
        if after.as_deref() == Some("secpivot-scheduled-wipe") {
            panic!("the scheduled wipe must clear the exact text it owns");
        }
        if after.is_none() && read_clipboard_text().ok().flatten().is_none() {
            // Empty could be our wipe or another process emptying the
            // clipboard; only a still-present foreign value proves the wipe
            // did not fire. Treat the ambiguous case as inconclusive.
            eprintln!("clipboard emptied during the wipe window; outcome inconclusive");
        }
        restore_clipboard(saved);
    }

    /// Deterministic contract for the generation guard. The end-to-end variant
    /// of this used to sleep against the live clipboard and passed even with
    /// `WIPE_GENERATION` deleted (no job owned the clipboard text, so nothing
    /// could ever clear it) — it could not detect the regression it claimed to.
    #[test]
    fn superseded_generation_never_wipes() {
        // First schedule is generation 1, second is 2. The first thread reads
        // the *current* generation when it wakes, so it is superseded and must
        // not clear — even though the clipboard still holds its own secret.
        assert!(!should_wipe(2, 1, "victim", Some("victim")));
        // The surviving generation only wipes for its own exact secret.
        assert!(!should_wipe(2, 2, "other", Some("victim")));
        assert!(should_wipe(2, 2, "victim", Some("victim")));
        // Empty, non-text, or foreign content is never ours to clear.
        assert!(!should_wipe(2, 2, "victim", None));
        assert!(!should_wipe(2, 2, "victim", Some("someone-else")));
        // An explicit cancel bumps the generation just like a new schedule.
        assert!(!should_wipe(3, 2, "victim", Some("victim")));
    }

    #[test]
    fn zero_seconds_is_a_no_op() {
        assert!(clipboard_schedule_wipe("x".into(), 0).is_ok());
    }
}
