//! Clipboard read/clear IPC commands (extracted from commands.rs), plus the
//! backend-enforced scheduled wipe: a safety net that clears copied secrets
//! even if the webview process dies before the renderer's JS timer fires.
//!
//! One worker holds only the latest zeroizing fingerprint and deadline.
//! Replacement/cancel drops pending state immediately, while an in-flight
//! wipe verifies its generation and clipboard content under the native lock.

use crate::platform::clipboard;
mod scheduler;

use scheduler::LatestDeadline;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use zeroize::Zeroizing;
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

/// Explicit clear on lock. False means no native bridge and requires the
/// renderer Clipboard API; never report a non-Windows no-op as success.
#[tauri::command]
pub(crate) fn clipboard_clear() -> Result<bool, String> {
    if !cfg!(target_os = "windows") {
        return Ok(false);
    }
    clipboard::clear_clipboard().map(|()| true)
}

/// Windows comparison and clearing use one clipboard lock. None tells other
/// platforms to use the renderer's clipboard API rather than claiming a clear.
#[tauri::command]
pub(crate) fn clipboard_clear_if_matches(fingerprint: [u8; 32]) -> Result<Option<bool>, String> {
    let fingerprint = WipeFingerprint(Zeroizing::new(fingerprint));
    if !cfg!(target_os = "windows") {
        return Ok(None);
    }
    clipboard::clear_clipboard_text_if(|current| fingerprint.matches(current.map(str::as_bytes)))
        .map(Some)
}

enum ClipboardFormat {
    Text,
    Entries,
}

struct WipeTask {
    format: ClipboardFormat,
    fingerprint: WipeFingerprint,
}

static WIPE_SCHEDULER: OnceLock<Arc<LatestDeadline<WipeTask>>> = OnceLock::new();
static WIPE_WORKER: OnceLock<Result<(), String>> = OnceLock::new();

fn wipe_scheduler() -> &'static Arc<LatestDeadline<WipeTask>> {
    WIPE_SCHEDULER.get_or_init(LatestDeadline::new)
}

fn ensure_wipe_worker() -> Result<(), String> {
    WIPE_WORKER
        .get_or_init(|| {
            wipe_scheduler()
                .start_worker(perform_wipe)
                .map(|_worker| ())
        })
        .as_ref()
        .copied()
        .map_err(Clone::clone)
}

fn perform_wipe(scheduler: &LatestDeadline<WipeTask>, generation: u64, task: WipeTask) {
    if !scheduler.is_current(generation) {
        return;
    }
    match task.format {
        ClipboardFormat::Text => {
            let _ = clipboard::clear_clipboard_text_if(|current| {
                should_wipe(
                    scheduler.is_current(generation),
                    &task.fingerprint,
                    current.map(str::as_bytes),
                )
            });
        }
        ClipboardFormat::Entries => {
            let _ = clipboard::clear_clipboard_bytes_if(
                crate::vault::exchange::CLIP_FORMAT_ENTRIES,
                |current| should_wipe(scheduler.is_current(generation), &task.fingerprint, current),
            );
        }
    }
}

/// A scheduled task must never retain the copied plaintext or entry payload.
/// The short-lived source buffer is zeroized before the task is started.
struct WipeFingerprint(Zeroizing<[u8; 32]>);

impl WipeFingerprint {
    fn from_bytes(bytes: &[u8]) -> Self {
        Self(Zeroizing::new(crate::crypto::sha256_bytes(bytes)))
    }

    fn matches(&self, current: Option<&[u8]>) -> bool {
        current.is_some_and(|bytes| crate::crypto::sha256_bytes(bytes) == *self.0)
    }
}

/// Generation and content must both match inside the clipboard lock.
fn should_wipe(is_current: bool, fingerprint: &WipeFingerprint, current: Option<&[u8]>) -> bool {
    is_current && fingerprint.matches(current)
}

/// Cancel without creating a worker when no wipe has ever been scheduled.
#[tauri::command]
pub(crate) fn clipboard_cancel_scheduled_wipe() -> Result<(), String> {
    wipe_scheduler().cancel()
}

/// One shared worker survives renderer death. Zero seconds cancels an older
/// schedule; unsupported platforms rely on their renderer's clipboard API.
#[tauri::command]
pub(crate) fn clipboard_schedule_wipe(text: String, seconds: u64) -> Result<(), String> {
    let text = Zeroizing::new(text);
    let scheduler = wipe_scheduler();
    let generation = scheduler.reserve()?;
    if seconds == 0 || !cfg!(target_os = "windows") {
        return Ok(());
    }
    let fingerprint = WipeFingerprint::from_bytes(text.as_bytes());
    drop(text);
    ensure_wipe_worker()?;
    scheduler
        .schedule_reserved(
            generation,
            WipeTask {
                format: ClipboardFormat::Text,
                fingerprint,
            },
            Duration::from_secs(seconds),
        )
        .map(|_installed| ())
}

/// Binary entry exchange uses the same latest-task slot as text. Its
/// short-lived read is zeroized before retaining only a fingerprint.
#[tauri::command]
pub(crate) fn clipboard_schedule_exchange_wipe(seconds: u64) -> Result<(), String> {
    let scheduler = wipe_scheduler();
    let generation = scheduler.reserve()?;
    if seconds == 0 || !cfg!(target_os = "windows") {
        return Ok(());
    }
    let Some(ours) = clipboard::read_clipboard_bytes(crate::vault::exchange::CLIP_FORMAT_ENTRIES)?
    else {
        return Ok(());
    };
    let ours = Zeroizing::new(ours);
    let fingerprint = WipeFingerprint::from_bytes(ours.as_slice());
    drop(ours);
    ensure_wipe_worker()?;
    scheduler
        .schedule_reserved(
            generation,
            WipeTask {
                format: ClipboardFormat::Entries,
                fingerprint,
            },
            Duration::from_secs(seconds),
        )
        .map(|_installed| ())
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
        if std::env::var("SECPIVOT_CLIPBOARD_LIVE").as_deref() != Ok("1") {
            eprintln!("skip: live clipboard requires SECPIVOT_CLIPBOARD_LIVE=1");
            return;
        }
        let _guard = lock_clipboard();
        let saved = read_clipboard_text().ok().flatten();
        if !clipboard_is_usable("secpivot-wipe-probe") {
            eprintln!("no interactive clipboard on this station; skipping wipe assertion");
            restore_clipboard(saved);
            return;
        }
        write_clipboard_text("secpivot-scheduled-wipe").unwrap();
        clipboard_schedule_wipe("secpivot-scheduled-wipe".into(), 1).unwrap();
        // The worker waits a full second against the *shared* Windows
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

    /// Content/generation gate used by the actual worker callback; scheduler
    /// tests additionally exercise replacement and in-flight invalidation.
    #[test]
    fn superseded_generation_never_wipes() {
        let victim = WipeFingerprint::from_bytes(b"victim");
        let other = WipeFingerprint::from_bytes(b"other");
        // A superseded task cannot clear even if its fingerprint still matches.
        assert!(!should_wipe(false, &victim, Some(b"victim")));
        // The surviving generation only wipes for its own exact secret.
        assert!(!should_wipe(true, &other, Some(b"victim")));
        assert!(should_wipe(true, &victim, Some(b"victim")));
        // Empty, non-text, or foreign content is never ours to clear.
        assert!(!should_wipe(true, &victim, None));
        assert!(!should_wipe(true, &victim, Some(b"someone-else")));
        // An explicit cancel bumps the generation just like a new schedule.
        assert!(!should_wipe(false, &victim, Some(b"victim")));
    }

    #[test]
    fn binary_fingerprint_preserves_exact_content_ownership() {
        let ours = WipeFingerprint::from_bytes(&[0, 255, 0, 42]);
        assert!(should_wipe(true, &ours, Some(&[0, 255, 0, 42])));
        assert!(!should_wipe(true, &ours, Some(&[0, 255, 0, 43])));
        assert!(!should_wipe(true, &ours, Some(&[0, 255, 0, 42, 0])));
        assert!(!should_wipe(false, &ours, Some(&[0, 255, 0, 42])));
        assert!(!should_wipe(true, &ours, None));
        let empty = WipeFingerprint::from_bytes(b"");
        assert!(empty.matches(Some(b"")));
        assert!(!empty.matches(None));
    }

    #[test]
    fn zero_seconds_does_not_start_a_wipe() {
        assert!(clipboard_schedule_wipe("x".into(), 0).is_ok());
        assert!(clipboard_schedule_exchange_wipe(0).is_ok());
    }
}
