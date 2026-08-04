//! Atomic persistence of the normalized `AppConfig` to
//! `<project_dir>/conf/config.json`, plus the managed `ConfigStore`
//! (extracted from config.rs).

use super::{normalize_config, AppConfig, RemoteSettings};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const CONFIG_SUBDIR: &str = "conf";

const CONFIG_FILE: &str = "config.json";

fn config_path(project_dir: &Path) -> PathBuf {
    project_dir.join(CONFIG_SUBDIR).join(CONFIG_FILE)
}

fn read_config(project_dir: &Path) -> Result<AppConfig, String> {
    let path = config_path(project_dir);
    if path.exists() {
        let text = fs::read_to_string(&path).map_err(|e| format!("读取配置失败: {e}"))?;
        let mut value: AppConfig =
            serde_json::from_str(&text).map_err(|e| format!("解析配置失败: {e}"))?;
        for profile in &mut value.remote_profiles {
            profile.settings.access_key =
                crate::platform::dpapi::decrypt(&profile.settings.access_key);
            profile.settings.secret_key =
                crate::platform::dpapi::decrypt(&profile.settings.secret_key);
        }
        if let Some(legacy) = &mut value.remote {
            legacy.access_key = crate::platform::dpapi::decrypt(&legacy.access_key);
            legacy.secret_key = crate::platform::dpapi::decrypt(&legacy.secret_key);
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
        profile.settings.access_key =
            crate::platform::dpapi::encrypt_for_storage(&profile.settings.access_key)?;
        profile.settings.secret_key =
            crate::platform::dpapi::encrypt_for_storage(&profile.settings.secret_key)?;
    }
    let text =
        serde_json::to_string_pretty(&persisted).map_err(|e| format!("序列化配置失败: {e}"))?;
    crate::util::atomic_write(&path, text.as_bytes(), "配置")
}

/// Managed state: the in-memory normalized config plus its project dir.
pub struct ConfigStore {
    project_dir: PathBuf,
    config: Mutex<AppConfig>,
}

impl ConfigStore {
    pub fn load(project_dir: PathBuf) -> Result<Self, String> {
        let config = read_config(&project_dir)?;
        Ok(Self {
            project_dir,
            config: Mutex::new(config),
        })
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

    /// Settings of the profile at `index`, clamped to the last valid profile
    /// (mirrors how `active_remote` is normalized). Returns the decrypted
    /// in-memory values — commands resolve profiles here instead of taking
    /// credentials over IPC.
    pub fn remote_settings(&self, index: usize) -> Result<RemoteSettings, String> {
        let guard = self.config.lock().map_err(|_| "配置锁已损坏".to_owned())?;
        let idx = index.min(guard.remote_profiles.len().saturating_sub(1));
        Ok(guard.remote_profiles[idx].settings.clone())
    }

    /// Name + settings of the profile at `index`, clamped to the last valid
    /// profile (mirrors `remote_settings`). Commands resolve the profile here
    /// so the mirror folder can be named after the profile name without
    /// sending the name over IPC.
    pub fn remote_profile(&self, index: usize) -> Result<(String, RemoteSettings), String> {
        let guard = self.config.lock().map_err(|_| "配置锁已损坏".to_owned())?;
        let idx = index.min(guard.remote_profiles.len().saturating_sub(1));
        let profile = &guard.remote_profiles[idx];
        Ok((profile.name.clone(), profile.settings.clone()))
    }
}
