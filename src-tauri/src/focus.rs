//! Windows-only helper that reads the title of the window currently in focus.
//!
//! The global auto-type hotkey uses it to pick the entry whose URL or title
//! matches the target window, so credentials are filled into the right place.

/// Title of the foreground window, if one can be read.
pub fn foreground_window_title() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW,
        };
        // SAFETY: pure Win32 queries on the current thread.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return None;
            }
            let len = GetWindowTextLengthW(hwnd);
            if len == 0 {
                return None;
            }
            let mut buffer = vec![0u16; (len + 1) as usize];
            let read = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
            if read == 0 {
                return None;
            }
            Some(String::from_utf16_lossy(&buffer[..read as usize]))
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}
