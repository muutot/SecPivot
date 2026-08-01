//! Screen-capture guard for sensitive windows (Windows only).
//!
//! Enables the Win32 `SetWindowDisplayAffinity` `WDA_MONITOR` affinity on the
//! main window: the window stays visible on screen but renders as a solid
//! block in screenshots, screen recordings and screen sharing. The guard is
//! active while a vault is open and released on lock/close. No-op everywhere
//! else.

use tauri::{AppHandle, Manager};

/// Enable or disable the capture guard on the main application window.
pub fn set_capture_guard(app: &AppHandle, enabled: bool) {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_MONITOR, WDA_NONE,
        };

        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        let Ok(hwnd) = window.hwnd() else {
            return;
        };
        let affinity = if enabled { WDA_MONITOR } else { WDA_NONE };
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
