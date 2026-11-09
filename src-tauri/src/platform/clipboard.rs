//! Windows clipboard helpers. The scheduled clipboard wipe must only clear
//! the text *we* copied: blindly emptying the clipboard could destroy
//! something the user copied in another app while the timer was pending.
//! Conditional clears hold one Win32 clipboard lock across read/compare/clear.

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

#[cfg(target_os = "windows")]
const MAX_TEXT_FORMAT_BYTES: usize = 4 * 1024 * 1024;

#[cfg(target_os = "windows")]
struct OpenClipboardGuard;

#[cfg(target_os = "windows")]
impl Drop for OpenClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: this guard exists only after this thread opened the clipboard.
        unsafe { CloseClipboard() };
    }
}

#[cfg(target_os = "windows")]
fn open_clipboard() -> Result<OpenClipboardGuard, String> {
    // SAFETY: null HWND selects the current thread's window station.
    if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
        return Err("打开剪贴板失败".to_owned());
    }
    Ok(OpenClipboardGuard)
}

#[cfg(target_os = "windows")]
fn decode_clipboard_text(units: &[u16]) -> Result<String, String> {
    let end = units
        .iter()
        .position(|unit| *unit == 0)
        .ok_or_else(|| "剪贴板文本缺少终止符".to_owned())?;
    String::from_utf16(&units[..end]).map_err(|_| "剪贴板文本编码无效".to_owned())
}

/// Current clipboard text, if the clipboard holds Unicode text. Returns
/// `None` for empty or non-text content.
#[cfg(target_os = "windows")]
pub fn read_clipboard_text() -> Result<Option<String>, String> {
    let _open = open_clipboard()?;
    read_text_open()
}

/// Caller holds OpenClipboardGuard throughout every access to the OS handle.
#[cfg(target_os = "windows")]
fn read_text_open() -> Result<Option<String>, String> {
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT) };
    if handle.is_null() {
        return Ok(None);
    }
    let byte_len = unsafe { GlobalSize(handle) };
    if !(2..=MAX_TEXT_FORMAT_BYTES).contains(&byte_len) || !byte_len.is_multiple_of(2) {
        return Err("剪贴板文本大小无效".to_owned());
    }
    let ptr = unsafe { GlobalLock(handle) } as *const u16;
    if ptr.is_null() {
        return Err("锁定剪贴板内存失败".to_owned());
    }
    // SAFETY: GlobalSize bounds this locked allocation; the length is even,
    // capped before allocation, and the clipboard remains open on this thread.
    let slice = unsafe { std::slice::from_raw_parts(ptr, byte_len / 2) };
    let text = decode_clipboard_text(slice);
    unsafe { GlobalUnlock(handle) };
    text.map(Some)
}

/// Compare and clear text while the clipboard is exclusively open, so a
/// foreign copy cannot arrive between the comparison and EmptyClipboard.
#[cfg(target_os = "windows")]
pub fn clear_clipboard_text_if(
    predicate: impl FnOnce(Option<&str>) -> bool,
) -> Result<bool, String> {
    let _open = open_clipboard()?;
    let text = read_text_open()?.map(zeroize::Zeroizing::new);
    if !predicate(text.as_ref().map(|text| text.as_str())) {
        return Ok(false);
    }
    empty_open()?;
    Ok(true)
}

#[cfg(not(target_os = "windows"))]
pub fn clear_clipboard_text_if(
    _predicate: impl FnOnce(Option<&str>) -> bool,
) -> Result<bool, String> {
    Ok(false)
}

#[cfg(target_os = "windows")]
fn empty_open() -> Result<(), String> {
    if unsafe { EmptyClipboard() } == 0 {
        return Err("清空剪贴板失败".to_owned());
    }
    Ok(())
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
    let _open = open_clipboard()?;
    empty_open()
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
    let id = register_format(format)?;
    let _open = open_clipboard()?;
    read_bytes_open(id)
}

#[cfg(target_os = "windows")]
fn read_bytes_open(id: u32) -> Result<Option<Vec<u8>>, String> {
    let handle = unsafe { GetClipboardData(id) };
    if handle.is_null() {
        return Ok(None);
    }
    let len = unsafe { GlobalSize(handle) };
    if len == 0 || len > MAX_CUSTOM_FORMAT_BYTES {
        return Err("剪贴板数据大小无效".to_owned());
    }
    let ptr = unsafe { GlobalLock(handle) } as *const u8;
    if ptr.is_null() {
        return Err("锁定剪贴板内存失败".to_owned());
    }
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) }.to_vec();
    unsafe {
        GlobalUnlock(handle);
    }
    Ok(Some(bytes))
}

#[cfg(target_os = "windows")]
pub fn clear_clipboard_bytes_if(
    format: &str,
    predicate: impl FnOnce(Option<&[u8]>) -> bool,
) -> Result<bool, String> {
    let id = register_format(format)?;
    let _open = open_clipboard()?;
    let bytes = read_bytes_open(id)?.map(zeroize::Zeroizing::new);
    if !predicate(bytes.as_ref().map(|bytes| bytes.as_slice())) {
        return Ok(false);
    }
    empty_open()?;
    Ok(true)
}

#[cfg(not(target_os = "windows"))]
pub fn clear_clipboard_bytes_if(
    _format: &str,
    _predicate: impl FnOnce(Option<&[u8]>) -> bool,
) -> Result<bool, String> {
    Ok(false)
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
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    #[cfg(target_os = "windows")]
    #[test]
    fn text_read_stops_at_nul_before_allocation_padding() {
        let units = [b'A' as u16, 0, 0, b'X' as u16, 0];
        assert_eq!(decode_clipboard_text(&units).unwrap(), "A");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn malformed_text_is_rejected_without_lossy_replacement() {
        assert!(decode_clipboard_text(&[b'A' as u16]).is_err());
        assert!(decode_clipboard_text(&[0xD800, 0]).is_err());
        assert_eq!(decode_clipboard_text(&[0]).unwrap(), "");
        let emoji: Vec<u16> = "密码🔑".encode_utf16().chain([0, 0]).collect();
        assert_eq!(decode_clipboard_text(&emoji).unwrap(), "密码🔑");
    }

    /// The Windows clipboard is a process-wide resource. One shared guard for
    /// every clipboard test in the crate — a per-module lock left the platform
    /// and command tests free to interleave, so `OpenClipboard` contention made
    /// them read back each other's (or the developer's) text. Process-wide only:
    /// `cargo nextest` gives every test its own process, so the tests below
    /// also probe for a usable clipboard and skip instead of asserting on
    /// foreign content.
    pub(crate) static CLIPBOARD_LOCK: Mutex<()> = Mutex::new(());

    /// Lock the shared clipboard guard, tolerating a poisoned mutex left by an
    /// earlier failing test.
    pub(crate) fn lock_clipboard() -> std::sync::MutexGuard<'static, ()> {
        CLIPBOARD_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    /// Whether this station has an interactive clipboard at all. A successful
    /// write followed by `Ok(None)`/`Err` means there is no window station to
    /// test against (CI, service session, locked desktop) — the caller must
    /// skip rather than assert against whatever else holds the clipboard.
    pub(crate) fn clipboard_is_usable(probe: &str) -> bool {
        matches!(write_clipboard_text(probe), Ok(()))
            && matches!(read_clipboard_text(), Ok(Some(ref text)) if text == probe)
    }

    /// Put back whatever the clipboard held before the test ran, so a test
    /// never destroys the developer's real clipboard contents.
    pub(crate) fn restore_clipboard(saved: Option<String>) {
        match saved {
            Some(text) => {
                let _ = write_clipboard_text(&text);
            }
            None => {
                let _ = clear_clipboard();
            }
        }
    }

    #[test]
    fn clipboard_read_write_round_trip() {
        if std::env::var("SECPIVOT_CLIPBOARD_LIVE").as_deref() != Ok("1") {
            eprintln!("skip: live clipboard requires SECPIVOT_CLIPBOARD_LIVE=1");
            return;
        }
        let _guard = lock_clipboard();
        let saved = read_clipboard_text().ok().flatten();
        if !clipboard_is_usable("secpivot-clipboard-probe") {
            eprintln!("no interactive clipboard on this station; skipping round-trip assertions");
            restore_clipboard(saved);
            return;
        }
        write_clipboard_text("secpivot-clipboard-test").unwrap();
        assert!(
            read_clipboard_text().ok().flatten().as_deref() == Some("secpivot-clipboard-test"),
            "a successful write must read back byte-for-byte"
        );
        restore_clipboard(saved);
    }

    #[test]
    fn clear_clipboard_empties_text() {
        if std::env::var("SECPIVOT_CLIPBOARD_LIVE").as_deref() != Ok("1") {
            eprintln!("skip: live clipboard requires SECPIVOT_CLIPBOARD_LIVE=1");
            return;
        }
        let _guard = lock_clipboard();
        let saved = read_clipboard_text().ok().flatten();
        if !clipboard_is_usable("secpivot-clear-probe") {
            eprintln!("no interactive clipboard on this station; skipping clear assertions");
            restore_clipboard(saved);
            return;
        }
        write_clipboard_text("to-be-emptied").unwrap();
        clear_clipboard().unwrap();
        assert_ne!(
            read_clipboard_text().unwrap().as_deref(),
            Some("to-be-emptied"),
            "clear_clipboard must remove the text it just wrote"
        );
        restore_clipboard(saved);
    }
}
