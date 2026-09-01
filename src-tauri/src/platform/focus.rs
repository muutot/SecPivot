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

const TCATO_TITLE: &str = "TCATO 两通道填充";

#[cfg(target_os = "windows")]
fn foreground_is_tcato(hwnd: *mut core::ffi::c_void) -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowTextLengthW, GetWindowTextW};
    if hwnd.is_null() {
        return false;
    }
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len == 0 {
            return false;
        }
        let mut buffer = vec![0u16; (len + 1) as usize];
        let read = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        if read == 0 {
            return false;
        }
        let title = String::from_utf16_lossy(&buffer[..read as usize]);
        title == TCATO_TITLE
    }
}

/// Make the TCATO overlay window non-activatable so clicking its buttons does
/// not steal focus from the target application. The `WM_CHAR` injection then
/// reaches the previously focused window without an extra Z-order lookup.
#[cfg(target_os = "windows")]
pub fn apply_tcato_no_activate(window: &tauri::WebviewWindow) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongW, SetWindowLongW, GWL_EXSTYLE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    };
    let Ok(hwnd) = window.hwnd() else {
        return;
    };
    unsafe {
        let style = GetWindowLongW(hwnd.0, GWL_EXSTYLE);
        let new_style = style | WS_EX_NOACTIVATE as i32 | WS_EX_TOOLWINDOW as i32;
        SetWindowLongW(hwnd.0, GWL_EXSTYLE, new_style);
    }
}

#[cfg(not(target_os = "windows"))]
pub fn apply_tcato_no_activate(_window: &tauri::WebviewWindow) {}

/// Send `text` to the window currently in focus (TCATO channel).
///
/// Windows: one `WM_CHAR` message per UTF-16 code unit, bypassing keyboard
/// hooks. Other platforms fall back to simulated typing via the auto-type
/// engine. When the TCATO overlay itself is the foreground window (its buttons
/// were just clicked), the next visible top-level window in Z-order is used
/// instead so the keystrokes land in the intended target.
pub fn send_text_to_foreground(text: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetForegroundWindow, GetWindow, IsWindowVisible, PostMessageW, GW_HWNDNEXT, WM_CHAR,
        };
        // SAFETY: pure Win32 calls on the current thread.
        unsafe {
            let mut hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return Err("没有前台窗口可接收输入".to_owned());
            }
            if foreground_is_tcato(hwnd) {
                let mut next = GetWindow(hwnd, GW_HWNDNEXT);
                let mut attempts = 0;
                while !next.is_null() && attempts < 32 {
                    if IsWindowVisible(next) != 0 && !foreground_is_tcato(next) {
                        hwnd = next;
                        break;
                    }
                    next = GetWindow(next, GW_HWNDNEXT);
                    attempts += 1;
                }
                if foreground_is_tcato(hwnd) {
                    return Err("请先将焦点移到目标窗口，再点击注入按钮".to_owned());
                }
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
        crate::platform::autotype::execute_tokens(&[
            crate::platform::autotype::AutotypeToken::Text(text.to_owned()),
        ])
        .map_err(|e| e.to_string())
    }
}
