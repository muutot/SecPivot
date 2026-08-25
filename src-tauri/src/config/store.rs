//! Atomic persistence of the normalized `AppConfig` to
//! `<project_dir>/conf/config.json`, plus the managed `ConfigStore`
//! (extracted from config.rs).

use super::{normalize_config, AppConfig, RemoteSettings};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const CONFIG_SUBDIR: &str = "conf";

const CONFIG_FILE: &str = "config.json";

/// Marker file shipped in the portable ZIP: its presence beside the
/// executable forces portable mode even before the first config write.
const PORTABLE_MARKER: &str = "portable.flag";

/// Resolve the data root ("project dir") and the portable flag. Portable mode
/// keeps everything (config, remote mirrors) beside the executable so a
/// directory copied to a USB stick travels as one unit; it is active when a
/// `portable.flag` marker or a legacy `conf/config.json` sits beside the
/// executable. Otherwise the standard per-user app-data directory is used
/// (installed builds must not write into `Program Files`). When neither
/// location is available the executable dir (or `.`, dev fallback) is used.
pub fn resolve_data_dir(exe_dir: Option<&Path>, app_data_dir: Option<PathBuf>) -> (PathBuf, bool) {
    if let Some(dir) = exe_dir {
        if dir.join(PORTABLE_MARKER).is_file()
            || dir.join(CONFIG_SUBDIR).join(CONFIG_FILE).is_file()
        {
            return (dir.to_path_buf(), true);
        }
    }
    match app_data_dir {
        Some(dir) => (dir, false),
        None => (
            exe_dir
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from(".")),
            false,
        ),
    }
}

fn config_path(project_dir: &Path) -> PathBuf {
    project_dir.join(CONFIG_SUBDIR).join(CONFIG_FILE)
}

fn read_config(project_dir: &Path) -> Result<AppConfig, String> {
    let path = config_path(project_dir);
    if path.exists() {
        let text = fs::read_to_string(&path).map_err(|e| format!("读取配置失败: {e}"))?;
        // A hand-edited or half-written file with wrong-typed scalars (e.g. a
        // fraction where an integer is expected) fails the strict parse and
        // used to abort the setup hook, bricking startup until the file was
        // deleted by hand. Salvage instead: keep the corrupt file for
        // inspection and start on defaults.
        let value: AppConfig = match serde_json::from_str(&text) {
            Ok(value) => value,
            Err(e) => {
                let backup = path.with_extension("json.bak");
                let renamed = fs::rename(&path, &backup).is_ok();
                eprintln!(
                    "配置文件解析失败（{}），使用默认配置启动: {e}",
                    if renamed {
                        format!("已备份为 {}", backup.display())
                    } else {
                        "备份失败，原文件保留".to_owned()
                    }
                );
                return Ok(normalize_config(AppConfig::default()));
            }
        };
        let mut value = value;
        for profile in &mut value.remote_profiles {
            decrypt_profile_creds(&mut profile.settings);
        }
        Ok(normalize_config(value))
    } else {
        Ok(normalize_config(AppConfig::default()))
    }
}

fn write_config(project_dir: &Path, config: &AppConfig) -> Result<(), String> {
    let dir = project_dir.join(CONFIG_SUBDIR);
    fs::create_dir_all(&dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    let path = dir.join(CONFIG_FILE);
    let mut persisted = config.clone();
    for profile in &mut persisted.remote_profiles {
        encrypt_profile_creds(&mut profile.settings)?;
    }
    let text =
        serde_json::to_string_pretty(&persisted).map_err(|e| format!("序列化配置失败: {e}"))?;
    crate::util::atomic_write(&path, text.as_bytes(), "配置")
}

fn decrypt_profile_creds(settings: &mut RemoteSettings) {
    settings.access_key = crate::platform::dpapi::decrypt(&settings.access_key);
    settings.secret_key = crate::platform::dpapi::decrypt(&settings.secret_key);
}

fn encrypt_profile_creds(settings: &mut RemoteSettings) -> Result<(), String> {
    settings.access_key = crate::platform::dpapi::encrypt_for_storage(&settings.access_key)?;
    settings.secret_key = crate::platform::dpapi::encrypt_for_storage(&settings.secret_key)?;
    Ok(())
}

/// Managed state: the in-memory normalized config plus its data root.
pub struct ConfigStore {
    project_dir: PathBuf,
    portable: bool,
    config: Mutex<AppConfig>,
}

impl ConfigStore {
    pub fn load(project_dir: PathBuf) -> Result<Self, String> {
        Self::load_with_mode(project_dir, false)
    }

    pub fn load_with_mode(project_dir: PathBuf, portable: bool) -> Result<Self, String> {
        let config = read_config(&project_dir)?;
        Ok(Self {
            project_dir,
            portable,
            config: Mutex::new(config),
        })
    }

    /// Data root for everything that travels with the installation:
    /// `conf/config.json` and the remote mirror hierarchy live under it.
    pub fn data_dir(&self) -> &Path {
        &self.project_dir
    }

    /// Whether the app runs in portable mode (data beside the executable).
    pub fn is_portable(&self) -> bool {
        self.portable
    }

    pub fn config_path(&self) -> PathBuf {
        config_path(&self.project_dir)
    }

    pub fn get(&self) -> Result<AppConfig, String> {
        self.config
            .lock()
            .map(|guard| guard.clone())
            .map_err(|_| "配置锁已损坏".to_owned())
    }

    pub fn set(&self, config: AppConfig) -> Result<AppConfig, String> {
        let normalized = normalize_config(config);
        write_config(&self.project_dir, &normalized)?;
        let mut guard = self.config.lock().map_err(|_| "配置锁已损坏".to_owned())?;
        *guard = normalized.clone();
        Ok(normalized)
    }

    /// Resolve a canonical `<kind>/<name>` path to decrypted settings. Remote
    /// credentials remain backend-owned and never cross the IPC boundary.
    pub fn remote_settings(&self, path: &str) -> Result<RemoteSettings, String> {
        self.remote_profile(path).map(|(_, settings)| settings)
    }

    /// Canonical path + settings for one profile. The path also determines the
    /// local mirror hierarchy (`Storage/remote/<kind>/<name>`).
    pub fn remote_profile(&self, path: &str) -> Result<(String, RemoteSettings), String> {
        let guard = self.config.lock().map_err(|_| "配置锁已损坏".to_owned())?;
        let profile = guard
            .remote_profiles
            .iter()
            .find(|profile| profile.path() == path)
            .ok_or_else(|| format!("远程配置不存在: {path}"))?;
        Ok((profile.path(), profile.settings.clone()))
    }
}
