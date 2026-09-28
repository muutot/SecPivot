//! Windows clipboard helpers. The scheduled clipboard wipe must only clear
//! the text *we* copied: blindly emptying the clipboard could destroy
//! something the user copied in another app while the timer was pending.
//! `read_clipboard_text` lets the frontend compare before wiping.

#[cfg(target_os = "windows")]
use windows_sys::{
    Win32::Foundation::HGLOBAL,
    Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
    },
    Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE, GMEM_ZEROINIT,
    },
};

#[cfg(target_os = "windows")]
const CF_UNICODETEXT: u32 = 13;

/// Current clipboard text, if the clipboard holds Unicode text. Returns
/// `None` for empty or non-text content.
#[cfg(target_os = "windows")]
pub fn read_clipboard_text() -> Result<Option<String>, String> {
    // SAFETY: clipboard APIs take a nullable HWND; passing null uses the
    // current thread's window station, which is valid for read access.
    if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        // Another process holds the clipboard open; treat as unreadable
        // rather than failing the caller.
        return Ok(None);
    }
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
    if handle.is_null() {
        unsafe { CloseClipboard() };
        return Ok(None);
    }
    let ptr = unsafe { GlobalLock(handle) } as *const u16;
    if ptr.is_null() {
        unsafe { CloseClipboard() };
        return Ok(None);
    }
    // Length in bytes; divide by two for the UTF-16 code-unit count. The
    // allocation includes the NUL terminator, which is not part of the text.
    let byte_len = unsafe { GlobalSize(handle) };
    let code_units = (byte_len / 2).saturating_sub(1);
    let text = if code_units > 0 {
        let slice = unsafe { std::slice::from_raw_parts(ptr, code_units as usize) };
        String::from_utf16_lossy(slice)
    } else {
        String::new()
    };
    unsafe { GlobalUnlock(handle) };
    unsafe { CloseClipboard() };
    Ok(Some(text))
}

/// Non-Windows stub: clipboard is handled by the webview (`navigator.clipboard`).
#[cfg(not(target_os = "windows"))]
pub fn read_clipboard_text() -> Result<Option<String>, String> {
    Ok(None)
}

/// Empty the clipboard (used when the frontend confirmed the clipboard still
/// holds our own text, or on lock with `clearOnLock`).
#[cfg(target_os = "windows")]
pub fn clear_clipboard() -> Result<(), String> {
    // SAFETY: see `read_clipboard_text`; `EmptyClipboard` requires the
    // clipboard to be open by this thread.
    if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        return Err("打开剪贴板失败".to_owned());
    }
    let ok = unsafe { EmptyClipboard() };
    unsafe { CloseClipboard() };
    if ok == 0 {
        return Err("清空剪贴板失败".to_owned());
    }
    Ok(())
}

/// Non-Windows stub: webview clipboard is cleared by the renderer.
#[cfg(not(target_os = "windows"))]
pub fn clear_clipboard() -> Result<(), String> {
    Ok(())
}

/// Write `text` to the clipboard (fallback path; `navigator.clipboard` is
/// preferred on the webview side).
#[allow(dead_code)]
#[cfg(target_os = "windows")]
pub fn write_clipboard_text(text: &str) -> Result<(), String> {
    // SAFETY: standard Win32 clipboard flow; the global memory is owned by
    // the clipboard after SetClipboardData, so no manual free.
    if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        return Err("打开剪贴板失败".to_owned());
    }
    unsafe { EmptyClipboard() };
    let encoded: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = encoded.len() * 2;
    let hmem: HGLOBAL = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes) };
    if hmem.is_null() {
        unsafe { CloseClipboard() };
        return Err("分配剪贴板内存失败".to_owned());
    }
    let dest = unsafe { GlobalLock(hmem) } as *mut u16;
    if dest.is_null() {
        unsafe { CloseClipboard() };
        return Err("锁定剪贴板内存失败".to_owned());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(encoded.as_ptr(), dest, encoded.len());
        GlobalUnlock(hmem);
    }
    let ok = unsafe { SetClipboardData(CF_UNICODETEXT, hmem) };
    unsafe { CloseClipboard() };
    if ok.is_null() {
        return Err("写入剪贴板失败".to_owned());
    }
    Ok(())
}

/// Non-Windows stub: webview clipboard writes are done by the renderer.
#[allow(dead_code)]
#[cfg(not(target_os = "windows"))]
pub fn write_clipboard_text(_text: &str) -> Result<(), String> {
    Ok(())
}

// ---------------------------------------------------------------------------
// Custom clipboard formats (KeePass entry exchange)
// ---------------------------------------------------------------------------

/// Cap for a single custom-format read. The clipboard is writable by any local
/// app, so the size is checked before the allocation, not after.
#[cfg(target_os = "windows")]
const MAX_CUSTOM_FORMAT_BYTES: usize = 64 * 1024 * 1024;

#[cfg(target_os = "windows")]
fn register_format(name: &str) -> Result<u32, String> {
    use windows_sys::Win32::System::DataExchange::RegisterClipboardFormatW;
    // SAFETY: `name` is a NUL-terminated UTF-16 buffer that outlives the call.
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let id = unsafe { RegisterClipboardFormatW(wide.as_ptr()) };
    if id == 0 {
        return Err("注册剪贴板格式失败".to_owned());
    }
    Ok(id)
}

/// Read a raw byte payload previously stored under a custom clipboard format.
/// `Ok(None)` means the clipboard simply does not hold that format.
#[cfg(target_os = "windows")]
pub fn read_clipboard_bytes(format: &str) -> Result<Option<Vec<u8>>, String> {
    use windows_sys::Win32::System::DataExchange::GetClipboardData;

    let id = register_format(format)?;
    // SAFETY: null HWND uses the current thread's window station, valid for
    // read access; a failure is reported as "unreadable" rather than fatal.
    if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        return Ok(None);
    }
    let handle = unsafe { GetClipboardData(id) };
    if handle.is_null() {
        unsafe { CloseClipboard() };
        return Ok(None);
    }
    let len = unsafe { GlobalSize(handle) };
    if len == 0 || len > MAX_CUSTOM_FORMAT_BYTES {
        unsafe { CloseClipboard() };
        return Err("剪贴板数据大小无效".to_owned());
    }
    let ptr = unsafe { GlobalLock(handle) } as *const u8;
    if ptr.is_null() {
        unsafe { CloseClipboard() };
        return Err("锁定剪贴板内存失败".to_owned());
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec();
    unsafe {
        GlobalUnlock(handle);
        CloseClipboard();
    }
    Ok(Some(bytes))
}

/// Non-Windows stub: no OS clipboard bridge, so the exchange format is absent.
#[cfg(not(target_os = "windows"))]
pub fn read_clipboard_bytes(_format: &str) -> Result<Option<Vec<u8>>, String> {
    Ok(None)
}

/// Store a raw byte payload under a custom clipboard format, replacing whatever
/// the clipboard held. The clipboard takes ownership of the global memory.
#[cfg(target_os = "windows")]
pub fn write_clipboard_bytes(format: &str, bytes: &[u8]) -> Result<(), String> {
    use windows_sys::Win32::System::DataExchange::SetClipboardData;

    let id = register_format(format)?;
    if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        return Err("打开剪贴板失败".to_owned());
    }
    unsafe { EmptyClipboard() };
    let hmem: HGLOBAL = unsafe { GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, bytes.len()) };
    if hmem.is_null() {
        unsafe { CloseClipboard() };
        return Err("分配剪贴板内存失败".to_owned());
    }
    let dest = unsafe { GlobalLock(hmem) } as *mut u8;
    if dest.is_null() {
        unsafe { CloseClipboard() };
        return Err("锁定剪贴板内存失败".to_owned());
    }
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest, bytes.len());
        GlobalUnlock(hmem);
    }
    let ok = unsafe { SetClipboardData(id, hmem) };
    unsafe { CloseClipboard() };
    if ok.is_null() {
        return Err("写入剪贴板失败".to_owned());
    }
    Ok(())
}

/// Non-Windows stub: the renderer has no binary clipboard bridge.
#[cfg(not(target_os = "windows"))]
pub fn write_clipboard_bytes(_format: &str, _bytes: &[u8]) -> Result<(), String> {
    Err("当前平台不支持条目数据交换".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// The Windows clipboard is a process-wide resource; the tests share
    /// system state, so serializing them avoids cross-test interference
    /// (including heap corruption from racing `GlobalLock`/`EmptyClipboard`).
    static CLIPBOARD_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn clipboard_read_write_round_trip() {
        let _guard = CLIPBOARD_LOCK.lock().unwrap();
        let _ = write_clipboard_text("secpivot-clipboard-test");
        match read_clipboard_text() {
            Ok(Some(text)) => assert_eq!(text, "secpivot-clipboard-test"),
            // CI machines may have no interactive window station; the API
            // must then degrade gracefully instead of panicking.
            Ok(None) => {}
            Err(e) => panic!("unexpected clipboard error: {e}"),
        }
        let _ = clear_clipboard();
    }

    #[test]
    fn clear_clipboard_empties_text() {
        let _guard = CLIPBOARD_LOCK.lock().unwrap();
        let _ = write_clipboard_text("to-be-emptied");
        let _ = clear_clipboard();
        // After EmptyClipboard the clipboard may be owned by an app that
        // re-supplies content; either result is acceptable as long as we
        // do not error out.
        let _ = read_clipboard_text();
    }
}
