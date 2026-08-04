//! Tauri IPC command handlers, grouped by domain. Thin wrappers around the
//! backend services; passwords and keys never cross IPC.

pub(crate) mod bridge;
pub(crate) mod clipboard;
pub(crate) mod config;
pub(crate) mod credential;
pub(crate) mod entries;
pub(crate) mod favicon;
pub(crate) mod groups;
pub(crate) mod remote;
pub(crate) mod tcato;
#[cfg(test)]
mod tests;
pub(crate) mod vault;

pub(crate) use self::bridge::*;
pub(crate) use self::clipboard::*;
pub(crate) use self::config::*;
pub(crate) use self::credential::*;
pub(crate) use self::entries::*;
pub(crate) use self::favicon::*;
pub(crate) use self::groups::*;
pub(crate) use self::remote::*;
pub(crate) use self::tcato::*;
pub(crate) use self::vault::*;
