//! App configuration: serde shape mirrors the frontend `AppSettings`
//! (`src/lib/types/settings.ts`), plus normalization and atomic persistence
//! to `<project_dir>/conf/config.json`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
const CONFIG_SUBDIR: &str = "conf";

const CONFIG_FILE: &str = "config.json";

const RECENT_FILES_MAX: usize = 8;

/// Default backup file name template. `{name}` = file stem, `{timestamp}` =
/// `YYYYMMDDHHmmssSSS`, `{ext}` = original extension.
const DEFAULT_BACKUP_TEMPLATE: &str = "{name}.{timestamp}.{ext}.bak";
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

/// One entry-table column's persisted state (KeePass-style list). `id` is
/// the built-in column id ("title", "username", "password", "url", "totp",
/// "notes", "tags", "created", "modified", "expires") or `custom:<field name>`
/// for entry custom fields. `width` is px; the "title" column uses `0` as an
/// auto sentinel (the frontend resolves it to the default column width).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EntryColumnState {
    pub id: String,
    pub visible: bool,
    pub width: i32,
}

impl Default for EntryColumnState {
    fn default() -> Self {
        Self {
            id: String::new(),
            visible: true,
            width: 120,
        }
    }
}

fn col(id: &str, visible: bool, width: i32) -> EntryColumnState {
    EntryColumnState {
        id: id.into(),
        visible,
        width,
    }
}

/// Default entry-table columns, mirroring the frontend
/// `DEFAULT_ENTRY_COLUMNS` in `src/lib/services/settings.ts`.
pub fn default_entry_columns() -> Vec<EntryColumnState> {
    vec![
        col("title", true, 0),
        col("username", true, 120),
        col("password", true, 100),
        col("url", true, 180),
        col("totp", true, 96),
        col("notes", false, 160),
        col("tags", false, 120),
        col("created", false, 140),
        col("modified", false, 140),
        col("expires", false, 140),
    ]
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
    /// Toolbar control buttons show icons only (labels on hover tooltips).
    pub icon_only_buttons: bool,
    /// Legacy global auto-type hotkey from configs written before the
    /// `keyboard` section existed; migrated into `keyboard.auto_type_global`
    /// on load, never re-serialized.
    #[serde(default, skip_serializing)]
    pub global_auto_type_shortcut: String,
    /// Entry-table column layout (visible + px width per column id).
    pub entry_columns: Vec<EntryColumnState>,
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
            icon_only_buttons: false,
            global_auto_type_shortcut: String::new(),
            entry_columns: default_entry_columns(),
        }
    }
}

/// Keyboard section: the global auto-type hotkey plus app-window shortcuts
/// for common actions (save, lock, edit, …). Mirrors the frontend
/// `KeyboardSettings` in `src/lib/types/settings.ts`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct KeyboardSettings {
    /// Global auto-type hotkey (accelerator syntax, e.g. "Ctrl+Shift+A").
    /// Empty means the hotkey is disabled.
    pub auto_type_global: String,
    /// App-window shortcuts: action id → accelerator. An absent key or empty
    /// value means the action is unbound.
    pub shortcuts: HashMap<String, String>,
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
    /// Exclude the main window from screenshots/recordings while a vault is
    /// open (Windows `WDA_EXCLUDEFROMCAPTURE`). Default off — opt-in from the
    /// welcome page; see `shield.rs` for why `WDA_MONITOR` must not be used.
    pub screen_capture_guard: bool,
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
            screen_capture_guard: false,
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
    /// File extension used as the default in "另存为" and as the fallback
    /// extension when a backup target has none. Stored without the leading dot.
    pub file_extension: String,
}

impl Default for DatabaseDefaults {
    fn default() -> Self {
        Self {
            kdf: "Argon2id".into(),
            cipher: "Aes256".into(),
            compression: "Gzip".into(),
            generator: PasswordGeneratorSettings::default(),
            file_extension: "kdbx".into(),
        }
    }
}

/// Favicon download behavior ("Download Favicons"). Mirrors the frontend
/// `FaviconSettings` in `src/lib/types/settings.ts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct FaviconSettings {
    /// How many distinct hosts may be fetched at once. 8 is a safe default:
    /// parallel enough to be fast, low enough not to overwhelm the system
    /// proxy (clash-verge & co.) with hundreds of simultaneous tunnels.
    pub concurrency: i32,
}

impl Default for FaviconSettings {
    fn default() -> Self {
        Self { concurrency: 8 }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteSettings {
    /// Transport kind: `"s3"` (S3-compatible object storage) or `"webdav"`.
    /// Normalized to one of the two on load.
    pub kind: String,
    /// S3-compatible endpoint, e.g. `https://s3.amazonaws.com` or a MinIO URL.
    /// For WebDAV this is the WebDAV base URL (e.g. a davfs/Nextcloud mount).
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
    /// Backup file name template. Placeholders: `{name}` (file stem),
    /// `{timestamp}` (`YYYYMMDDHHmmssSSS`), `{ext}` (original extension).
    pub backup_template: String,
}

impl Default for RemoteSettings {
    fn default() -> Self {
        Self {
            kind: "s3".into(),
            endpoint: "https://s3.amazonaws.com".into(),
            region: "us-east-1".into(),
            bucket: String::new(),
            access_key: String::new(),
            secret_key: String::new(),
            prefix: String::new(),
            local_dir: "remote".into(),
            backup_count: 3,
            backup_template: DEFAULT_BACKUP_TEMPLATE.into(),
        }
    }
}

/// One named S3 configuration. Multiple profiles can coexist; the frontend
/// picks the active one per command (`cfg` travels with every call).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteProfile {
    /// Display name shown in the profile selector.
    pub name: String,
    pub settings: RemoteSettings,
}

impl Default for RemoteProfile {
    fn default() -> Self {
        Self {
            name: "默认".into(),
            settings: RemoteSettings::default(),
        }
    }
}

/// KeePassHttp browser bridge. The loopback server only runs while `enabled`
/// is true; association keys are session-held (never persisted) and wiped on
/// vault lock, so the bridge serves nothing while locked.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BridgeSettings {
    pub enabled: bool,
}

/// KeePassRPC (Kee 4.x) bridge. Same lifecycle as the KeePassHttp bridge:
/// the loopback server only runs while `enabled`, and SRP keys are
/// session-held (never persisted) and wiped on vault lock.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RpcSettings {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    #[serde(default)]
    pub general: GeneralSettings,
    #[serde(default)]
    pub security: SecuritySettings,
    #[serde(default)]
    pub database: DatabaseDefaults,
    /// Named S3 configurations; the browser and commands use the profile at
    /// `active_remote` (clamped to a valid index on load).
    #[serde(default)]
    pub remote_profiles: Vec<RemoteProfile>,
    #[serde(default)]
    pub active_remote: usize,
    /// Legacy single-profile field from configs written before profiles
    /// existed; migrated into `remote_profiles` on load, never re-serialized.
    #[serde(default, skip_serializing)]
    pub remote: Option<RemoteSettings>,
    #[serde(default)]
    pub bridge: BridgeSettings,
    #[serde(default)]
    pub rpc: RpcSettings,
    #[serde(default)]
    pub keyboard: KeyboardSettings,
    #[serde(default)]
    pub favicon: FaviconSettings,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            general: GeneralSettings::default(),
            security: SecuritySettings::default(),
            database: DatabaseDefaults::default(),
            remote_profiles: vec![RemoteProfile::default()],
            active_remote: 0,
            remote: None,
            bridge: BridgeSettings::default(),
            rpc: RpcSettings::default(),
            keyboard: KeyboardSettings::default(),
            favicon: FaviconSettings::default(),
        }
    }
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

/// Clamp every column's width to 30..=400 px ("title" keeps its `0` auto
/// sentinel) and keep ids/visibility verbatim — unknown ids survive the
/// round-trip because custom-field columns appear dynamically on the frontend.
/// Boundary-clamps (not fallback) to stay idempotent with the frontend
/// `clampInt` in `normalizeEntryColumns`.
fn normalize_entry_columns(columns: Vec<EntryColumnState>) -> Vec<EntryColumnState> {
    columns
        .into_iter()
        .map(|mut c| {
            if c.id == "title" && c.width == 0 {
                return c;
            }
            c.width = c.width.clamp(30, 400);
            c
        })
        .collect()
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

    config.general.entry_columns = normalize_entry_columns(config.general.entry_columns);

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

    config.favicon.concurrency = clamp_i32(config.favicon.concurrency, 1, 16, 8);

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
    config.database.file_extension = normalize_file_extension(&config.database.file_extension);

    config.remote_profiles = if config.remote_profiles.is_empty() {
        match config.remote.take() {
            Some(legacy) => vec![RemoteProfile {
                name: "默认".into(),
                settings: legacy,
            }],
            None => vec![RemoteProfile::default()],
        }
    } else {
        config.remote_profiles
    };
    for profile in &mut config.remote_profiles {
        normalize_remote_settings(&mut profile.settings);
        profile.name = profile.name.trim().to_owned();
        if profile.name.is_empty() {
            profile.name = "默认".into();
        }
    }
    config.active_remote = clamp_i32(
        config.active_remote as i32,
        0,
        config.remote_profiles.len() as i32 - 1,
        0,
    ) as usize;

    // Legacy `general.global_auto_type_shortcut` → `keyboard.auto_type_global`.
    if config.keyboard.auto_type_global.is_empty()
        && !config.general.global_auto_type_shortcut.is_empty()
    {
        config.keyboard.auto_type_global =
            std::mem::take(&mut config.general.global_auto_type_shortcut);
    }
    config.general.global_auto_type_shortcut.clear();

    config
}

/// Lowercase the transport kind and fall back to `"s3"` on anything unknown.
fn normalize_remote_kind(kind: &str) -> String {
    match kind.trim().to_ascii_lowercase().as_str() {
        "webdav" => "webdav".into(),
        _ => "s3".into(),
    }
}

fn normalize_remote_settings(settings: &mut RemoteSettings) {
    settings.kind = normalize_remote_kind(&settings.kind);
    settings.endpoint = settings.endpoint.trim().to_owned();
    settings.region = settings.region.trim().to_owned();
    settings.bucket = settings.bucket.trim().to_owned();
    settings.access_key = settings.access_key.trim().to_owned();
    settings.secret_key = settings.secret_key.trim().to_owned();
    settings.prefix = settings.prefix.trim().to_owned();
    settings.local_dir = settings.local_dir.trim().to_owned();
    if settings.local_dir.is_empty() {
        settings.local_dir = "remote".into();
    }
    settings.backup_count = clamp_i32(settings.backup_count, 0, 10, 3);
    settings.backup_template = settings.backup_template.trim().to_owned();
    if settings.backup_template.is_empty() {
        settings.backup_template = DEFAULT_BACKUP_TEMPLATE.into();
    }
}

/// Sanitize a user-supplied file extension: drop the leading dot, keep only
/// alphanumeric characters, fall back to `kdbx` when nothing remains.
fn normalize_file_extension(ext: &str) -> String {
    let cleaned: String = ext
        .trim()
        .trim_start_matches('.')
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect();
    if cleaned.is_empty() {
        "kdbx".into()
    } else {
        cleaned
    }
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
        let mut value: AppConfig =
            serde_json::from_str(&text).map_err(|e| format!("解析配置失败: {e}"))?;
        for profile in &mut value.remote_profiles {
            profile.settings.access_key = crate::dpapi::decrypt(&profile.settings.access_key);
            profile.settings.secret_key = crate::dpapi::decrypt(&profile.settings.secret_key);
        }
        if let Some(legacy) = &mut value.remote {
            legacy.access_key = crate::dpapi::decrypt(&legacy.access_key);
            legacy.secret_key = crate::dpapi::decrypt(&legacy.secret_key);
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
    let tmp = dir.join("config.json.tmp");
    let mut persisted = config.clone();
    for profile in &mut persisted.remote_profiles {
        profile.settings.access_key = crate::dpapi::encrypt(&profile.settings.access_key)?;
        profile.settings.secret_key = crate::dpapi::encrypt(&profile.settings.secret_key)?;
    }
    let text =
        serde_json::to_string_pretty(&persisted).map_err(|e| format!("序列化配置失败: {e}"))?;
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

    /// Settings of the profile at `index`, clamped to the last valid profile
    /// (mirrors how `active_remote` is normalized). Returns the decrypted
    /// in-memory values — commands resolve profiles here instead of taking
    /// credentials over IPC.
    pub fn remote_settings(&self, index: usize) -> Result<RemoteSettings, String> {
        let guard = self.config.lock().map_err(|_| "配置锁已损坏".to_owned())?;
        let idx = index.min(guard.remote_profiles.len().saturating_sub(1));
        Ok(guard.remote_profiles[idx].settings.clone())
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
    fn remote_profiles_survive_deserialize_write_reload() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let mut config = AppConfig::default();
        config.remote_profiles.push(RemoteProfile {
            name: "Bitiful".into(),
            settings: RemoteSettings {
                kind: "s3".into(),
                endpoint: "http://127.0.0.1:9000".into(),
                region: "cn-east-1".into(),
                bucket: "my-vaults".into(),
                access_key: "AKIA-test".into(),
                secret_key: "s3cret".into(),
                prefix: "vaults/".into(),
                local_dir: "backups".into(),
                backup_count: 5,
                backup_template: "{name}.{timestamp}.{ext}.bak".into(),
            },
        });
        config.active_remote = 1;
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"backupCount\": 5"));
        assert!(text.contains("\"remoteProfiles\""));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let again = reloaded.get().unwrap();
        assert_eq!(again.remote_profiles.len(), 2);
        assert_eq!(again.remote_profiles[1].name, "Bitiful");
        assert_eq!(
            again.remote_profiles[1].settings.endpoint,
            "http://127.0.0.1:9000"
        );
        assert_eq!(again.remote_profiles[1].settings.bucket, "my-vaults");
        assert_eq!(again.remote_profiles[1].settings.secret_key, "s3cret");
        assert_eq!(again.remote_profiles[1].settings.local_dir, "backups");
        assert_eq!(again.remote_profiles[1].settings.backup_count, 5);
        // active index is clamped to the last valid profile
        assert_eq!(again.active_remote, 1);
        // the legacy field must not be re-serialized
        assert!(!text.contains("\"remote\":"));
    }

    #[test]
    fn legacy_single_remote_migrates_into_first_profile() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("conf");
        fs::create_dir_all(&conf).unwrap();
        let legacy = r#"{"remote": {
            "endpoint": "https://s3.bitiful.net", "region": "cn-east-1",
            "bucket": "muuyo", "accessKey": "AK", "secretKey": "SK",
            "prefix": "vaults/", "localDir": "remote", "backupCount": 3
        }}"#;
        fs::write(conf.join("config.json"), legacy).unwrap();

        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let config = store.get().unwrap();
        assert_eq!(config.remote_profiles.len(), 1);
        assert_eq!(config.remote_profiles[0].name, "默认");
        assert_eq!(
            config.remote_profiles[0].settings.endpoint,
            "https://s3.bitiful.net"
        );
        assert_eq!(config.remote_profiles[0].settings.bucket, "muuyo");
        assert_eq!(config.active_remote, 0);
        // re-saving writes the new shape only
        store.set(config.clone()).unwrap();
        let text = std::fs::read_to_string(conf.join("config.json")).unwrap();
        assert!(text.contains("\"remoteProfiles\""));
        assert!(!text.contains("\"accessKey\": \"AK\""));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn remote_secrets_never_persist_in_plaintext() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let mut config = AppConfig::default();
        config.remote_profiles[0].settings.access_key = "AKIA-secret-access".into();
        config.remote_profiles[0].settings.secret_key = "plaintext-s3cret".into();
        store.set(config).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(!text.contains("AKIA-secret-access"));
        assert!(!text.contains("plaintext-s3cret"));
        assert!(text.contains("\"accessKey\": \"dpapi1:"));
        assert!(text.contains("\"secretKey\": \"dpapi1:"));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let again = reloaded.get().unwrap();
        assert_eq!(
            again.remote_profiles[0].settings.access_key,
            "AKIA-secret-access"
        );
        assert_eq!(
            again.remote_profiles[0].settings.secret_key,
            "plaintext-s3cret"
        );
    }

    #[test]
    fn remote_defaults_and_normalization_rules() {
        let mut config = AppConfig::default();
        assert_eq!(
            config.remote_profiles[0].settings.endpoint,
            "https://s3.amazonaws.com"
        );
        assert_eq!(config.remote_profiles[0].settings.local_dir, "remote");
        assert_eq!(config.remote_profiles[0].settings.backup_count, 3);
        assert_eq!(config.remote_profiles[0].settings.kind, "s3");

        config.remote_profiles[0].settings.endpoint = "  https://s3.example.com  ".into();
        config.remote_profiles[0].settings.local_dir = "   ".into();
        config.remote_profiles[0].settings.backup_count = 99;
        config.active_remote = 42;
        let normalized = normalize_config(config);
        assert_eq!(
            normalized.remote_profiles[0].settings.endpoint,
            "https://s3.example.com"
        );
        assert_eq!(normalized.remote_profiles[0].settings.local_dir, "remote");
        assert_eq!(normalized.remote_profiles[0].settings.backup_count, 3);
        assert_eq!(normalized.active_remote, 0);
    }

    #[test]
    fn remote_kind_normalizes_to_s3_or_webdav() {
        let mut config = AppConfig::default();
        config.remote_profiles[0].settings.kind = "  WebDAV ".into();
        assert_eq!(
            normalize_config(config).remote_profiles[0].settings.kind,
            "webdav"
        );

        let mut config = AppConfig::default();
        config.remote_profiles[0].settings.kind = "ftp".into();
        assert_eq!(
            normalize_config(config).remote_profiles[0].settings.kind,
            "s3"
        );

        let mut config = AppConfig::default();
        config.remote_profiles[0].settings.kind = "".into();
        assert_eq!(
            normalize_config(config).remote_profiles[0].settings.kind,
            "s3"
        );
    }

    #[test]
    fn file_extension_and_backup_template_normalize() {
        let mut config = AppConfig::default();
        assert_eq!(config.database.file_extension, "kdbx");
        assert_eq!(
            config.remote_profiles[0].settings.backup_template,
            "{name}.{timestamp}.{ext}.bak"
        );

        config.database.file_extension = "  .kdb!x  ".into();
        config.remote_profiles[0].settings.backup_template = "   ".into();
        let normalized = normalize_config(config);
        assert_eq!(normalized.database.file_extension, "kdbx");

        let mut config = AppConfig::default();
        config.database.file_extension = "txt".into();
        config.remote_profiles[0].settings.backup_template =
            "  {name}-{timestamp}.{ext}.old  ".into();
        let normalized = normalize_config(config);
        assert_eq!(normalized.database.file_extension, "txt");
        assert_eq!(
            normalized.remote_profiles[0].settings.backup_template,
            "{name}-{timestamp}.{ext}.old"
        );
        assert_eq!(normalize_file_extension(""), "kdbx");
        assert_eq!(normalize_file_extension("..."), "kdbx");
        assert_eq!(normalize_file_extension("KDBX"), "KDBX");
    }

    #[test]
    fn remote_settings_resolves_profile_and_clamps_index() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let mut config = AppConfig::default();
        config.remote_profiles.push(RemoteProfile {
            name: "Bitiful".into(),
            settings: RemoteSettings {
                endpoint: "http://127.0.0.1:9000".into(),
                ..RemoteSettings::default()
            },
        });
        config.active_remote = 1;
        store.set(config).unwrap();

        let first = store.remote_settings(0).unwrap();
        assert_eq!(first.endpoint, "https://s3.amazonaws.com");
        let second = store.remote_settings(1).unwrap();
        assert_eq!(second.endpoint, "http://127.0.0.1:9000");
        // out-of-range indices clamp to the last valid profile
        let clamped = store.remote_settings(9).unwrap();
        assert_eq!(clamped.endpoint, "http://127.0.0.1:9000");
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
    fn entry_columns_roundtrip_keeps_ids_and_clamps_widths() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let mut config = AppConfig::default();
        config.general.entry_columns = vec![
            col("title", true, 0),
            col("username", true, 120),
            col("custom:手机号", true, 999),
            col("notes", true, 10),
        ];
        store.set(config.clone()).unwrap();
        let normalized = store.get().unwrap();
        let by_id: std::collections::HashMap<&str, &EntryColumnState> = normalized
            .general
            .entry_columns
            .iter()
            .map(|c| (c.id.as_str(), c))
            .collect();
        // title keeps its 0 flex sentinel
        assert_eq!(by_id["title"].width, 0);
        assert!(by_id["title"].visible);
        // normal widths pass through
        assert_eq!(by_id["username"].width, 120);
        // custom-field ids survive the round-trip
        assert_eq!(by_id["custom:手机号"].width, 400);
        // out-of-range widths clamp
        assert_eq!(by_id["notes"].width, 30);
        assert_eq!(normalized.general.entry_columns.len(), 4);
    }

    #[test]
    fn empty_config_object_gets_default_entry_columns() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("conf");
        fs::create_dir_all(&conf).unwrap();
        fs::write(conf.join("config.json"), "{}").unwrap();

        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let config = store.get().unwrap();
        assert_eq!(
            config.general.entry_columns.len(),
            default_entry_columns().len()
        );
        assert_eq!(config.general.entry_columns[0].id, "title");
        assert_eq!(config.general.entry_columns[0].width, 0);
    }

    #[test]
    fn screen_capture_guard_defaults_off_and_survives_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        assert!(!store.get().unwrap().security.screen_capture_guard);

        let mut config = AppConfig::default();
        config.security.screen_capture_guard = true;
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"screenCaptureGuard\": true"));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        assert!(reloaded.get().unwrap().security.screen_capture_guard);
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
    fn keyboard_auto_type_global_round_trips_and_defaults_empty() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        assert_eq!(store.get().unwrap().keyboard.auto_type_global, "");

        let mut config = AppConfig::default();
        config.keyboard.auto_type_global = "Ctrl+Shift+A".into();
        config
            .keyboard
            .shortcuts
            .insert("save".into(), "Ctrl+S".into());
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"autoTypeGlobal\": \"Ctrl+Shift+A\""));
        assert!(text.contains("\"save\": \"Ctrl+S\""));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let again = reloaded.get().unwrap();
        assert_eq!(again.keyboard.auto_type_global, "Ctrl+Shift+A");
        assert_eq!(
            again.keyboard.shortcuts.get("save"),
            Some(&"Ctrl+S".to_string())
        );
    }

    #[test]
    fn old_config_without_hotkey_field_loads_with_defaults() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("conf");
        std::fs::create_dir_all(&conf).unwrap();
        std::fs::write(conf.join("config.json"), r#"{"general":{"theme":"light"}}"#).unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        assert_eq!(store.get().unwrap().keyboard.auto_type_global, "");
    }

    #[test]
    fn legacy_global_auto_type_shortcut_migrates_into_keyboard_section() {
        let dir = TempDir::new().unwrap();
        let conf = dir.path().join("conf");
        std::fs::create_dir_all(&conf).unwrap();
        std::fs::write(
            conf.join("config.json"),
            r#"{"general":{"globalAutoTypeShortcut":"Ctrl+Shift+A"}}"#,
        )
        .unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        let loaded = store.get().unwrap();
        assert_eq!(loaded.keyboard.auto_type_global, "Ctrl+Shift+A");
        assert_eq!(loaded.general.global_auto_type_shortcut, "");

        // A re-save writes it under `keyboard`, never the legacy field.
        store.set(loaded).unwrap();
        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"keyboard\""));
        assert!(text.contains("\"autoTypeGlobal\": \"Ctrl+Shift+A\""));
        assert!(!text.contains("globalAutoTypeShortcut"));
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
    fn bridge_defaults_off_and_survives_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        assert!(!store.get().unwrap().bridge.enabled);

        let mut config = AppConfig::default();
        config.bridge.enabled = true;
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"bridge\": {"));
        assert!(text.contains("\"enabled\": true"));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        assert!(reloaded.get().unwrap().bridge.enabled);
    }

    #[test]
    fn rpc_defaults_off_and_survives_round_trip() {
        let dir = TempDir::new().unwrap();
        let store = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        assert!(!store.get().unwrap().rpc.enabled);

        let mut config = AppConfig::default();
        config.rpc.enabled = true;
        store.set(config.clone()).unwrap();

        let text = std::fs::read_to_string(dir.path().join("conf").join("config.json")).unwrap();
        assert!(text.contains("\"rpc\": {"));
        assert!(text.contains("\"enabled\": true"));

        let reloaded = ConfigStore::load(dir.path().to_path_buf()).unwrap();
        assert!(reloaded.get().unwrap().rpc.enabled);
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
