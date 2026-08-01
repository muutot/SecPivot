//! Screen-capture guard for sensitive windows (Windows only).
//!
//! Enables the Win32 `SetWindowDisplayAffinity` `WDA_EXCLUDEFROMCAPTURE`
//! affinity on the main window: the window stays visible on screen but is
//! excluded from screenshots, screen recordings and screen sharing. The guard
//! is active while a vault is open and released on lock/close. No-op
//! everywhere else.
//!
//! Note: `WDA_EXCLUDEFROMCAPTURE` (0x11, Windows 10 2004+) keeps the window
//! rendering normally. `WDA_MONITOR` (0x1) must not be used here — it renders
//! the window as a solid black box on the physical display.

use tauri::{AppHandle, Manager};

/// Enable or disable the capture guard on the main application window.
pub fn set_capture_guard(app: &AppHandle, enabled: bool) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
        };

        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        let affinity = if enabled {
            WDA_EXCLUDEFROMCAPTURE
        } else {
            WDA_NONE
        };
        // SAFETY: `hwnd` is the live main window owned by the Tauri runtime and
        // remains valid for the lifetime of the app.
        unsafe {
            SetWindowDisplayAffinity(hwnd.0, affinity);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (app, enabled);
    }
}
