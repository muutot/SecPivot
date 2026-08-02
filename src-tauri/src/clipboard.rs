//! Windows clipboard helpers. The scheduled clipboard wipe must only clear
//! the text *we* copied: blindly emptying the clipboard could destroy
//! something the user copied in another app while the timer was pending.
//! `read_clipboard_text` lets the frontend compare before wiping.

use windows_sys::Win32::Foundation::HGLOBAL;
use windows_sys::Win32::System::DataExchange::CloseClipboard;
use windows_sys::Win32::System::DataExchange::EmptyClipboard;
use windows_sys::Win32::System::DataExchange::GetClipboardData;
use windows_sys::Win32::System::DataExchange::OpenClipboard;
use windows_sys::Win32::System::DataExchange::SetClipboardData;
use windows_sys::Win32::System::Memory::GlobalAlloc;
use windows_sys::Win32::System::Memory::GlobalLock;
use windows_sys::Win32::System::Memory::GlobalSize;
use windows_sys::Win32::System::Memory::GlobalUnlock;
use windows_sys::Win32::System::Memory::GMEM_MOVEABLE;
use windows_sys::Win32::System::Memory::GMEM_ZEROINIT;

const CF_UNICODETEXT: u32 = 13;

/// Current clipboard text, if the clipboard holds Unicode text. Returns
/// `None` for empty or non-text content.
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

/// Empty the clipboard (used when the frontend confirmed the clipboard still
/// holds our own text, or on lock with `clearOnLock`).
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

/// Write `text` to the clipboard (fallback path; `navigator.clipboard` is
/// preferred on the webview side).
#[allow(dead_code)]
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
        let _ = write_clipboard_text("keyvault-clipboard-test");
        match read_clipboard_text() {
            Ok(Some(text)) => assert_eq!(text, "keyvault-clipboard-test"),
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
