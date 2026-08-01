//! App configuration: serde shape mirrors the frontend `AppSettings`
//! (`src/lib/types/settings.ts`), plus normalization and atomic persistence
//! to `<project_dir>/conf/config.json`.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const CONFIG_SUBDIR: &str = "conf";
const CONFIG_FILE: &str = "config.json";
const RECENT_FILES_MAX: usize = 8;

// ---------------------------------------------------------------------------
// Serde shapes (camelCase on the wire)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ThemeColors {
    pub bg: String,
    pub settings_bg: String,
    pub accent: String,
    pub text_primary: String,
    pub text_muted: String,
    pub border: String,
    pub card_bg: String,
    pub surface_bg: String,
    pub status_bar_bg: String,
    pub hover_bg: String,
    pub input_bg: String,
    pub text_secondary: String,
    pub text_faint: String,
    pub placeholder_color: String,
    pub border_subtle: String,
    pub selection_color: String,
    pub success_color: String,
    pub danger_color: String,
    pub warning_color: String,
    pub scrollbar_color: String,
}

impl ThemeColors {
    pub fn dark() -> Self {
        Self {
            bg: "#111111".into(),
            settings_bg: "#1b1b1b".into(),
            accent: "#ff5050".into(),
            text_primary: "#f5f5f5".into(),
            text_muted: "#999999".into(),
            border: "#3a3a3a".into(),
            card_bg: "#1e1e1e".into(),
            surface_bg: "#1e1e1e".into(),
            status_bar_bg: "#181818".into(),
            hover_bg: "#2c2c2c".into(),
            input_bg: "#1a1a1a".into(),
            text_secondary: "#b2b2b2".into(),
            text_faint: "#6e6e6e".into(),
            placeholder_color: "#6e6e6e".into(),
            border_subtle: "#292929".into(),
            selection_color: "#4aa8ff".into(),
            success_color: "#51b96b".into(),
            danger_color: "#e85d5d".into(),
            warning_color: "#e2c05d".into(),
            scrollbar_color: "#858585".into(),
        }
    }

    pub fn light() -> Self {
        Self {
            bg: "#f5f5f5".into(),
            settings_bg: "#ebebeb".into(),
            accent: "#e04040".into(),
            text_primary: "#1a1a1a".into(),
            text_muted: "#666666".into(),
            border: "#cccccc".into(),
            card_bg: "#ffffff".into(),
            surface_bg: "#ffffff".into(),
            status_bar_bg: "#e8e8e8".into(),
            hover_bg: "#e0e0e0".into(),
            input_bg: "#f0f0f0".into(),
            text_secondary: "#444444".into(),
            text_faint: "#999999".into(),
            placeholder_color: "#aaaaaa".into(),
            border_subtle: "#dddddd".into(),
            selection_color: "#2196f3".into(),
            success_color: "#388e3c".into(),
            danger_color: "#d32f2f".into(),
            warning_color: "#f9a825".into(),
            scrollbar_color: "#aaaaaa".into(),
        }
    }
}

impl Default for ThemeColors {
    fn default() -> Self {
        Self::dark()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FontSizes {
    pub base: i32,
    pub secondary: i32,
    pub card_title: i32,
    pub card_preview: i32,
}

impl Default for FontSizes {
    fn default() -> Self {
        Self {
            base: 14,
            secondary: 11,
            card_title: 13,
            card_preview: 11,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DensitySettings {
    pub group_gap: i32,
    pub group_padding_y: i32,
    pub group_indent: i32,
    pub group_radius: i32,
    pub show_group_icon: bool,
    pub show_group_chevron: bool,
}

impl Default for DensitySettings {
    fn default() -> Self {
        Self {
            group_gap: 2,
            group_padding_y: 3,
            group_indent: 12,
            group_radius: 6,
            show_group_icon: true,
            show_group_chevron: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneralSettings {
    pub language: String,
    pub theme: String,
    pub theme_colors: ThemeColors,
    pub custom_presets: Vec<ThemeColors>,
    pub compact_mode: bool,
    pub density: DensitySettings,
    pub show_descriptions: bool,
    pub font_sizes: FontSizes,
    pub window_effect: String,
    pub window_opacity: i32,
    pub remember_last_database: bool,
    pub recent_files: Vec<String>,
    /// Main-window size remembered from the user's resize; the welcome screen
    /// uses a smaller fixed size instead.
    pub window_width: i32,
    pub window_height: i32,
    /// User-resizable pane widths in the main view, remembered across restarts.
    pub panel_widths: PanelWidths,
    /// Global auto-type hotkey (accelerator syntax, e.g. "Ctrl+Shift+A").
    /// Empty means the hotkey is disabled.
    pub global_auto_type_shortcut: String,
}

/// Resizable pane widths of the main view: group tree, detail panel, and the
/// URL column of the entry table.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PanelWidths {
    pub group: i32,
    pub detail: i32,
    /// URL column; the floor is derived from the header text ("网址"
    /// 2 chars × 10px font + 10px) — keep in sync with the frontend clamp.
    pub url_col: i32,
}

impl Default for PanelWidths {
    fn default() -> Self {
        Self {
            group: 200,
            detail: 300,
            url_col: 200,
        }
    }
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            language: "zh-CN".into(),
            theme: "dark".into(),
            theme_colors: ThemeColors::dark(),
            custom_presets: Vec::new(),
            compact_mode: false,
            density: DensitySettings::default(),
            show_descriptions: true,
            font_sizes: FontSizes::default(),
            window_effect: "off".into(),
            window_opacity: 100,
            remember_last_database: true,
            recent_files: Vec::new(),
            window_width: 1100,
            window_height: 720,
            panel_widths: PanelWidths::default(),
            global_auto_type_shortcut: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SecuritySettings {
    pub auto_lock_minutes: i32,
    pub clipboard_clear_seconds: i32,
    pub minimize_to_tray: bool,
    pub clear_on_lock: bool,
    pub lock_after_action: bool,
    pub lock_on_focus_loss: bool,
    pub remember_password: bool,
}

impl Default for SecuritySettings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: 5,
            clipboard_clear_seconds: 20,
            minimize_to_tray: true,
            clear_on_lock: true,
            lock_after_action: false,
            lock_on_focus_loss: false,
            remember_password: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PasswordGeneratorSettings {
    pub length: i32,
    pub include_upper: bool,
    pub include_lower: bool,
    pub include_digits: bool,
    pub include_symbols: bool,
    pub exclude_similar: bool,
    pub exclude_ambiguous: bool,
}

impl Default for PasswordGeneratorSettings {
    fn default() -> Self {
        Self {
            length: 20,
            include_upper: true,
            include_lower: true,
            include_digits: true,
            include_symbols: true,
            exclude_similar: false,
            exclude_ambiguous: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DatabaseDefaults {
    pub kdf: String,
    pub cipher: String,
    pub compression: String,
    pub generator: PasswordGeneratorSettings,
}

impl Default for DatabaseDefaults {
    fn default() -> Self {
        Self {
            kdf: "Argon2id".into(),
            cipher: "Aes256".into(),
            compression: "Gzip".into(),
            generator: PasswordGeneratorSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteSettings {
    /// S3-compatible endpoint, e.g. `https://s3.amazonaws.com` or a MinIO URL.
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub access_key: String,
    /// Plaintext in `config.json` by design — a secondary credential, never a
    /// vault master password. Keep the risk noted in `security-model.md`.
    pub secret_key: String,
    /// Optional key prefix (folder) used by the remote file browser.
    pub prefix: String,
    /// Subdirectory name under `Storage/remote/` for local copies of remote
    /// vaults ("保存到本地" mode).
    pub local_dir: String,
    /// Number of timestamped `.bak` backups kept beside the local copy;
    /// 0 disables backups.
    pub backup_count: i32,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            endpoint: "https://s3.amazonaws.com".into(),
            region: "us-east-1".into(),
            bucket: String::new(),
            access_key: String::new(),
            secret_key: String::new(),
            prefix: String::new(),
            local_dir: "remote".into(),
            backup_count: 3,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppConfig {
    pub general: GeneralSettings,
    pub security: SecuritySettings,
    pub database: DatabaseDefaults,
    pub remote: RemoteSettings,
}

// ---------------------------------------------------------------------------
// Normalization (must mirror `normalizeSettings` in services/settings.ts so
// the round-trip is idempotent)
// ---------------------------------------------------------------------------

fn clamp_i32(value: i32, min: i32, max: i32, fallback: i32) -> i32 {
    if value < min || value > max {
        fallback
    } else {
        value
    }
}

fn valid_hex(value: &str, fallback: &str) -> String {
    let bytes = value.as_bytes();
    let valid_len = bytes.len() == 7 || bytes.len() == 9;
    if valid_len && bytes[0] == b'#' && bytes[1..].iter().all(|b| b.is_ascii_hexdigit()) {
        value.to_owned()
    } else {
        fallback.to_owned()
    }
}

fn normalize_colors(mut colors: ThemeColors) -> ThemeColors {
    let defaults = ThemeColors::dark();
    colors.bg = valid_hex(&colors.bg, &defaults.bg);
    colors.settings_bg = valid_hex(&colors.settings_bg, &defaults.settings_bg);
    colors.accent = valid_hex(&colors.accent, &defaults.accent);
    colors.text_primary = valid_hex(&colors.text_primary, &defaults.text_primary);
    colors.text_muted = valid_hex(&colors.text_muted, &defaults.text_muted);
    colors.border = valid_hex(&colors.border, &defaults.border);
    colors.card_bg = valid_hex(&colors.card_bg, &defaults.card_bg);
    colors.surface_bg = valid_hex(&colors.surface_bg, &defaults.surface_bg);
    colors.status_bar_bg = valid_hex(&colors.status_bar_bg, &defaults.status_bar_bg);
    colors.hover_bg = valid_hex(&colors.hover_bg, &defaults.hover_bg);
    colors.input_bg = valid_hex(&colors.input_bg, &defaults.input_bg);
    colors.text_secondary = valid_hex(&colors.text_secondary, &defaults.text_secondary);
    colors.text_faint = valid_hex(&colors.text_faint, &defaults.text_faint);
    colors.placeholder_color = valid_hex(&colors.placeholder_color, &defaults.placeholder_color);
    colors.border_subtle = valid_hex(&colors.border_subtle, &defaults.border_subtle);
    colors.selection_color = valid_hex(&colors.selection_color, &defaults.selection_color);
    colors.success_color = valid_hex(&colors.success_color, &defaults.success_color);
    colors.danger_color = valid_hex(&colors.danger_color, &defaults.danger_color);
    colors.warning_color = valid_hex(&colors.warning_color, &defaults.warning_color);
    colors.scrollbar_color = valid_hex(&colors.scrollbar_color, &defaults.scrollbar_color);
    colors
}

/// Apply the same range/default rules as the frontend normalizer.
pub fn normalize_config(mut config: AppConfig) -> AppConfig {
    config.general.language =
        if config.general.language == "en" || config.general.language == "zh-CN" {
            config.general.language
        } else {
            "zh-CN".into()
        };
    config.general.theme = match config.general.theme.as_str() {
        "dark" | "light" | "custom" => config.general.theme,
        _ => "dark".into(),
    };
    config.general.window_effect = match config.general.window_effect.as_str() {
        "off" | "acrylic" | "mica" => config.general.window_effect,
        _ => "off".into(),
    };
    config.general.theme_colors = normalize_colors(config.general.theme_colors);
    for preset in &mut config.general.custom_presets {
        *preset = normalize_colors(std::mem::take(preset));
    }
    config.general.font_sizes.base = clamp_i32(config.general.font_sizes.base, 11, 20, 14);
    config.general.font_sizes.secondary = clamp_i32(config.general.font_sizes.secondary, 9, 16, 11);
    config.general.font_sizes.card_title =
        clamp_i32(config.general.font_sizes.card_title, 11, 18, 13);
    config.general.font_sizes.card_preview =
        clamp_i32(config.general.font_sizes.card_preview, 9, 16, 11);
    config.general.window_opacity = clamp_i32(config.general.window_opacity, 40, 100, 100);
    config.general.window_width = clamp_i32(config.general.window_width, 560, 2560, 1100);
    config.general.window_height = clamp_i32(config.general.window_height, 420, 1600, 720);
    config.general.panel_widths.group = clamp_i32(config.general.panel_widths.group, 140, 320, 200);
    config.general.panel_widths.detail =
        clamp_i32(config.general.panel_widths.detail, 260, 640, 300);
    config.general.panel_widths.url_col =
        clamp_i32(config.general.panel_widths.url_col, 30, 400, 200);
    config.general.density.group_gap = clamp_i32(config.general.density.group_gap, 0, 16, 2);
    config.general.density.group_padding_y =
        clamp_i32(config.general.density.group_padding_y, 0, 16, 3);
    config.general.density.group_indent = clamp_i32(config.general.density.group_indent, 4, 32, 12);
    config.general.density.group_radius = clamp_i32(config.general.density.group_radius, 0, 12, 6);

    let recent = std::mem::take(&mut config.general.recent_files);
    let mut seen = std::collections::HashSet::new();
    config.general.recent_files = recent
        .into_iter()
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty() && seen.insert(p.clone()))
        .take(RECENT_FILES_MAX)
        .collect();

    config.security.auto_lock_minutes = clamp_i32(config.security.auto_lock_minutes, 0, 240, 5);
    config.security.clipboard_clear_seconds =
        clamp_i32(config.security.clipboard_clear_seconds, 0, 600, 20);

    config.database.kdf = match config.database.kdf.as_str() {
        "Argon2id" | "Argon2" | "Aes" => config.database.kdf,
        _ => "Argon2id".into(),
    };
    config.database.cipher = match config.database.cipher.as_str() {
        "Aes256" | "ChaCha20" => config.database.cipher,
        _ => "Aes256".into(),
    };
    config.database.compression = match config.database.compression.as_str() {
        "None" | "Gzip" => config.database.compression,
        _ => "Gzip".into(),
    };
    config.database.generator.length = clamp_i32(config.database.generator.length, 8, 128, 20);

    config.remote.endpoint = config.remote.endpoint.trim().to_owned();
    config.remote.region = config.remote.region.trim().to_owned();
    config.remote.bucket = config.remote.bucket.trim().to_owned();
    config.remote.access_key = config.remote.access_key.trim().to_owned();
    config.remote.secret_key = config.remote.secret_key.trim().to_owned();
    config.remote.prefix = config.remote.prefix.trim().to_owned();
    config.remote.local_dir = config.remote.local_dir.trim().to_owned();
    if config.remote.local_dir.is_empty() {
        config.remote.local_dir = "remote".into();
    }
    config.remote.backup_count = clamp_i32(config.remote.backup_count, 0, 10, 3);

    config
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

fn config_path(project_dir: &Path) -> PathBuf {
    project_dir.join(CONFIG_SUBDIR).join(CONFIG_FILE)
}

fn read_config(project_dir: &Path) -> Result<AppConfig, String> {
    let path = config_path(project_dir);
    if path.exists() {
        let text = fs::read_to_string(&path).map_err(|e| format!("读取配置失败: {e}"))?;
        let value: AppConfig =
            serde_json::from_str(&text).map_err(|e| format!("解析配置失败: {e}"))?;
        Ok(normalize_config(value))
    } else {
        Ok(AppConfig::default())
    }
}

fn write_config(project_dir: &Path, config: &AppConfig) -> Result<(), String> {
    let dir = project_dir.join(CONFIG_SUBDIR);
    fs::create_dir_all(&dir).map_err(|e| format!("创建配置目录失败: {e}"))?;
    let path = dir.join(CONFIG_FILE);
    let tmp = dir.join("config.json.tmp");
    let text = serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {e}"))?;
    fs::write(&tmp, text).map_err(|e| format!("写入配置失败: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("保存配置失败: {e}"))?;
    Ok(())
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_round_trip_and_persist() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let defaults = store.get().unwrap();
        assert_eq!(defaults.general.theme, "dark");
        assert_eq!(defaults.database.kdf, "Argon2id");
        assert_eq!(defaults.general.font_sizes.base, 14);
        assert_eq!(defaults.general.density.group_gap, 2);
        assert_eq!(defaults.general.density.group_radius, 6);

        let saved = store.set(defaults.clone()).unwrap();
        assert_eq!(saved.general.theme, "dark");

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let again = reloaded.get().unwrap();
        assert_eq!(again.general.theme, "dark");
        assert_eq!(again.general.theme_colors.accent, "#ff5050");
        assert_eq!(again.general.density.group_gap, 2);
    }

    #[test]
    fn old_config_without_density_loads_with_defaults() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("conf");
        fs::create_dir_all(&conf).unwrap();
        let text = r#"{
          "general": {
            "language": "zh-CN",
            "theme": "light",
            "themeColors": {},
            "customPresets": [],
            "compactMode": true,
            "showDescriptions": true,
            "fontSizes": { "base": 14, "secondary": 11, "cardTitle": 13, "cardPreview": 11 },
            "windowEffect": "off",
            "windowOpacity": 100,
            "rememberLastDatabase": true
          },
          "security": { "autoLockMinutes": 5, "clipboardClearSeconds": 20, "minimizeToTray": true, "clearOnLock": true, "lockAfterAction": false },
          "database": { "kdf": "Argon2id", "cipher": "Aes256", "compression": "Gzip", "generator": { "length": 20, "includeUpper": true, "includeLower": true, "includeDigits": true, "includeSymbols": true, "excludeSimilar": false, "excludeAmbiguous": false } }
        }"#;
        fs::write(conf.join("config.json"), text).unwrap();

        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let config = store.get().unwrap();
        assert_eq!(config.general.theme, "light");
        assert!(config.general.compact_mode);
        assert_eq!(config.general.density.group_gap, 2);
        assert!(config.general.density.show_group_icon);
    }

    #[test]
    fn remote_settings_survive_deserialize_write_reload() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let mut config = AppConfig::default();
        config.remote.endpoint = "http://127.0.0.1:9000".into();
        config.remote.region = "us-east-1".into();
        config.remote.bucket = "my-vaults".into();
        config.remote.access_key = "AKIA-test".into();
        config.remote.secret_key = "s3cret".into();
        config.remote.prefix = "vaults/".into();
        config.remote.local_dir = "backups".into();
        config.remote.backup_count = 5;
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"secretKey\": \"s3cret\""));
        assert!(text.contains("\"backupCount\": 5"));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let again = reloaded.get().unwrap();
        assert_eq!(again.remote.endpoint, "http://127.0.0.1:9000");
        assert_eq!(again.remote.bucket, "my-vaults");
        assert_eq!(again.remote.secret_key, "s3cret");
        assert_eq!(again.remote.local_dir, "backups");
        assert_eq!(again.remote.backup_count, 5);
    }

    #[test]
    fn remote_defaults_and_normalization_rules() {
        let mut config = AppConfig::default();
        assert_eq!(config.remote.endpoint, "https://s3.amazonaws.com");
        assert_eq!(config.remote.local_dir, "remote");
        assert_eq!(config.remote.backup_count, 3);

        config.remote.endpoint = "  https://s3.example.com  ".into();
        config.remote.local_dir = "   ".into();
        config.remote.backup_count = 99;
        let normalized = normalize_config(config);
        assert_eq!(normalized.remote.endpoint, "https://s3.example.com");
        assert_eq!(normalized.remote.local_dir, "remote");
        assert_eq!(normalized.remote.backup_count, 3);
    }

    #[test]
    fn empty_config_object_loads_with_defaults() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("conf");
        fs::create_dir_all(&conf).unwrap();
        fs::write(conf.join("config.json"), "{}").unwrap();

        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let config = store.get().unwrap();
        assert_eq!(config.general.theme, "dark");
        assert_eq!(config.general.density.group_gap, 2);
        assert_eq!(config.security.clipboard_clear_seconds, 20);
        assert_eq!(config.database.generator.length, 20);
        assert!(!config.security.lock_on_focus_loss);
        assert!(!config.security.remember_password);
    }

    #[test]
    fn remember_password_survives_deserialize_write_reload() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let mut config = AppConfig::default();
        config.security.remember_password = true;
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"rememberPassword\": true"));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let again = reloaded.get().unwrap();
        assert!(again.security.remember_password);
    }

    #[test]
    fn lock_on_focus_loss_survives_deserialize_write_reload() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let mut config = AppConfig::default();
        config.security.lock_on_focus_loss = true;
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"lockOnFocusLoss\": true"));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let again = reloaded.get().unwrap();
        assert!(again.security.lock_on_focus_loss);
    }

    #[test]
    fn recent_files_survive_deserialize_write_reload() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let mut config = AppConfig::default();
        config.general.recent_files = vec!["C:/vault-a.kdbx".into(), "C:/vault-b.kdbx".into()];
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"recentFiles\""));
        assert!(text.contains("C:/vault-a.kdbx"));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let again = reloaded.get().unwrap();
        assert_eq!(
            again.general.recent_files,
            vec!["C:/vault-a.kdbx".to_owned(), "C:/vault-b.kdbx".to_owned()]
        );
    }

    #[test]
    fn recent_files_normalize_trims_dedups_and_caps() {
        let mut config = AppConfig::default();
        config.general.recent_files = vec![
            "  dup.kdbx ".into(),
            "dup.kdbx".into(),
            "".into(),
            "  ".into(),
            "keep.kdbx".into(),
        ];
        (0..RECENT_FILES_MAX).for_each(|i| config.general.recent_files.push(format!("f{i}.kdbx")));

        let normalized = normalize_config(config);
        assert_eq!(
            normalized.general.recent_files,
            vec![
                "dup.kdbx".to_owned(),
                "keep.kdbx".to_owned(),
                "f0.kdbx".to_owned(),
                "f1.kdbx".to_owned(),
                "f2.kdbx".to_owned(),
                "f3.kdbx".to_owned(),
                "f4.kdbx".to_owned(),
                "f5.kdbx".to_owned(),
            ]
        );
    }

    #[test]
    fn window_and_panel_sizes_survive_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let mut config = AppConfig::default();
        config.general.window_width = 1024;
        config.general.window_height = 768;
        config.general.panel_widths.group = 220;
        config.general.panel_widths.detail = 360;
        config.general.panel_widths.url_col = 260;
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"panelWidths\""));
        assert!(text.contains("\"urlCol\": 260"));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let again = reloaded.get().unwrap();
        assert_eq!(again.general.window_width, 1024);
        assert_eq!(again.general.window_height, 768);
        assert_eq!(again.general.panel_widths.group, 220);
        assert_eq!(again.general.panel_widths.detail, 360);
        assert_eq!(again.general.panel_widths.url_col, 260);
    }

    #[test]
    fn window_and_panel_sizes_are_clamped() {
        let mut config = AppConfig::default();
        config.general.window_width = 300;
        config.general.window_height = 99_999;
        config.general.panel_widths.group = 100;
        config.general.panel_widths.detail = 999;
        config.general.panel_widths.url_col = 1;

        let normalized = normalize_config(config);
        assert_eq!(normalized.general.window_width, 1100);
        assert_eq!(normalized.general.window_height, 720);
        assert_eq!(normalized.general.panel_widths.group, 200);
        assert_eq!(normalized.general.panel_widths.detail, 300);
        assert_eq!(normalized.general.panel_widths.url_col, 200);
    }

    #[test]
    fn density_survives_deserialize_write_reload() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let mut config = AppConfig::default();
        config.general.density.group_gap = 8;
        config.general.density.group_padding_y = 6;
        config.general.density.group_indent = 20;
        config.general.density.group_radius = 4;
        config.general.density.show_group_icon = false;
        config.general.density.show_group_chevron = false;
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"density\""));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let again = reloaded.get().unwrap();
        assert_eq!(again.general.density.group_gap, 8);
        assert_eq!(again.general.density.group_indent, 20);
        assert!(!again.general.density.show_group_chevron);
    }

    #[test]
    fn normalization_clamps_and_fixes_enums() {
        let mut config = AppConfig::default();
        config.general.theme = "neon".into();
        config.general.window_effect = "glass".into();
        config.general.window_opacity = 500;
        config.security.auto_lock_minutes = 9999;
        config.database.kdf = "scrypt".into();
        config.database.cipher = "des".into();
        config.database.compression = "zip".into();
        config.database.generator.length = 3;
        config.general.font_sizes.base = 5;
        config.general.theme_colors.accent = "not-a-color".into();
        config.general.density.group_gap = 99;
        config.general.density.group_indent = 0;

        let normalized = normalize_config(config);
        assert_eq!(normalized.general.theme, "dark");
        assert_eq!(normalized.general.window_effect, "off");
        assert_eq!(normalized.general.window_opacity, 100);
        assert_eq!(normalized.security.auto_lock_minutes, 5);
        assert_eq!(normalized.database.kdf, "Argon2id");
        assert_eq!(normalized.database.cipher, "Aes256");
        assert_eq!(normalized.database.compression, "Gzip");
        assert_eq!(normalized.database.generator.length, 20);
        assert_eq!(normalized.general.font_sizes.base, 14);
        assert_eq!(normalized.general.theme_colors.accent, "#ff5050");
        assert_eq!(normalized.general.density.group_gap, 2);
        assert_eq!(normalized.general.density.group_indent, 12);
    }

    #[test]
    fn normalization_is_idempotent() {
        let mut config = AppConfig::default();
        config.general.compact_mode = true;
        let once = normalize_config(config);
        let twice = normalize_config(once.clone());
        assert_eq!(
            serde_json::to_string(&once).unwrap(),
            serde_json::to_string(&twice).unwrap()
        );
    }
}
