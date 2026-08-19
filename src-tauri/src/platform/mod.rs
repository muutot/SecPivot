//! OS integration services (Windows/tauri-backed).
//!
//! - [`autotype`] — KeePass-style auto-type sequence parser + `enigo`
//!   keystroke replay, `{REF:...}` field-reference expansion.
//! - [`focus`] — foreground-window title reader (Win32) for global auto-type
//!   matching; TCATO `WM_CHAR` channel injection.
//! - [`shield`] — screen-capture guard for sensitive windows.
//! - [`clipboard`] — clipboard read/clear/write.
//! - [`credential`] — keyring-backed saved-credential store.
//! - [`dpapi`] — Windows DPAPI secret protection for config persistence.

pub mod autotype;
pub mod clipboard;
#[cfg(desktop)]
pub mod credential;
pub mod dpapi;
#[cfg(desktop)]
pub mod focus;
pub mod shield;
