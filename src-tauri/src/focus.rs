//! Windows-only helpers around the window currently in focus.
//!
//! The global auto-type hotkey reads the foreground window title to pick a
//! matching entry; the TCATO overlay sends text to the foreground window
//! through `WM_CHAR` messages, which — unlike simulated key presses — are not
//! observable by low-level keyboard hooks.

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

/// Send `text` to the window currently in focus (TCATO channel).
///
/// Windows: one `WM_CHAR` message per UTF-16 code unit, bypassing keyboard
/// hooks. Other platforms fall back to simulated typing via the auto-type
/// engine.
pub fn send_text_to_foreground(text: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, PostMessageW, WM_CHAR,
        };
        // SAFETY: pure Win32 calls on the current thread.
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return Err("没有前台窗口可接收输入".to_owned());
            }
            for unit in text.encode_utf16() {
                if PostMessageW(hwnd, WM_CHAR, unit as usize, 0) == 0 {
                    return Err("向目标窗口发送输入失败".to_owned());
                }
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        crate::autotype::execute_tokens(&[crate::autotype::AutotypeToken::Text(text.to_owned())])
            .map_err(|e| e.to_string())
    }
}
