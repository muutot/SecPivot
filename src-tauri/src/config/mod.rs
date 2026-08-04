//! App configuration: serde shape mirrors the frontend `AppSettings`
//! (`src/lib/types/settings.ts`), plus normalization and atomic persistence
//! to `<project_dir>/conf/config.json`.

pub mod settings;
pub mod store;
#[cfg(test)]
mod tests;

pub(crate) const RECENT_FILES_MAX: usize = 8;

pub(crate) use self::settings::DEFAULT_BACKUP_TEMPLATE;
pub use self::settings::*;
pub use self::store::ConfigStore;
