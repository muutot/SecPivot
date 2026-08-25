//! App settings serde shapes (mirrors `src/lib/types/settings.ts`) plus
//! defaults and normalization (extracted from config.rs).

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::RECENT_FILES_MAX;

/// Default backup file name template. `{name}` = file stem, `{timestamp}` =
/// `YYYYMMDDHHmmssSSS`, `{ext}` = original extension.
pub(crate) const DEFAULT_BACKUP_TEMPLATE: &str = "{name}.{timestamp}.{ext}.bak";
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
    pub link_color: String,
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
            accent: "#8bc34a".into(),
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
            selection_color: "#808080".into(),
            link_color: "#1e90ff".into(),
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
            accent: "#689f38".into(),
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
            selection_color: "#757575".into(),
            link_color: "#0066cc".into(),
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
/// "notes", "tags", "created", "modified", "expires", "size") or
/// `custom:<field name>` for entry custom fields. `width` is px; the "title"
/// column uses `0` as an auto sentinel (the frontend resolves it to the
/// default column width).
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

pub(crate) fn col(id: &str, visible: bool, width: i32) -> EntryColumnState {
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
        col("size", false, 80),
    ]
}

/// One named advanced-search query (mirrors `AdvancedSearchQuery` in
/// `src/lib/utils/entry-search.ts`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AdvancedSearchQuery {
    pub text: String,
    pub field: String,
    pub regex: bool,
    pub exclude: bool,
    pub only_expired: bool,
    pub only_favorites: bool,
    pub tags: String,
    pub require_quality_check: bool,
}

impl Default for AdvancedSearchQuery {
    fn default() -> Self {
        Self {
            text: String::new(),
            field: "all".into(),
            regex: false,
            exclude: false,
            only_expired: false,
            only_favorites: false,
            tags: String::new(),
            require_quality_check: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SavedSearch {
    pub name: String,
    pub query: AdvancedSearchQuery,
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
    /// Collect lower-frequency toolbar actions in a shared More menu.
    pub toolbar_overflow_menu: bool,
    /// Legacy global auto-type hotkey from configs written before the
    /// `keyboard` section existed; migrated into `keyboard.auto_type_global`
    /// on load, never re-serialized.
    #[serde(default, skip_serializing)]
    pub global_auto_type_shortcut: String,
    /// Entry-table column layout (visible + px width per column id).
    pub entry_columns: Vec<EntryColumnState>,
    /// Named advanced-search queries.
    pub saved_searches: Vec<SavedSearch>,
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
            toolbar_overflow_menu: cfg!(any(target_os = "android", target_os = "ios")),
            global_auto_type_shortcut: String::new(),
            entry_columns: default_entry_columns(),
            saved_searches: Vec::new(),
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
    /// Profile name; absent on the built-in default.
    pub name: Option<String>,
    pub length: i32,
    pub include_upper: bool,
    pub include_lower: bool,
    pub include_digits: bool,
    pub include_symbols: bool,
    pub exclude_similar: bool,
    pub exclude_ambiguous: bool,
    /// Replaces the built-in character classes entirely when non-empty.
    pub custom_charset: Option<String>,
    /// Characters removed from every pool.
    pub exclude_chars: Option<String>,
    /// Each character must appear at least once in the result.
    pub required_chars: Option<String>,
    /// KeePass-style pattern (`u`/`l`/`d`/`s`/`a`, other chars literal).
    pub pattern: Option<String>,
}

impl Default for PasswordGeneratorSettings {
    fn default() -> Self {
        Self {
            name: None,
            length: 20,
            include_upper: true,
            include_lower: true,
            include_digits: true,
            include_symbols: true,
            exclude_similar: false,
            exclude_ambiguous: false,
            custom_charset: None,
            exclude_chars: None,
            required_chars: None,
            pattern: None,
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
    /// Named generator profiles selectable for new entries.
    pub generator_profiles: Vec<PasswordGeneratorSettings>,
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
            generator_profiles: Vec::new(),
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
    /// Persist the database immediately after applying downloaded icons.
    /// Off by default: icons are applied to the open session and the vault
    /// is left dirty for a manual save, so downloads never write the disk
    /// behind the user's back.
    pub auto_save: bool,
}

impl Default for FaviconSettings {
    fn default() -> Self {
        Self {
            concurrency: 8,
            auto_save: false,
        }
    }
}

/// One remote configuration. `kind` discriminates the only valid shape:
/// S3 uses region/bucket, while WebDAV uses endpoint + Basic-auth credentials.
/// WebDAV normalization clears region/bucket so one profile never carries two
/// transports' effective configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteSettings {
    pub kind: String,
    pub endpoint: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub region: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub bucket: String,
    pub access_key: String,
    pub secret_key: String,
    /// Optional key prefix (folder) used by the remote file browser.
    pub prefix: String,
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
            backup_count: 3,
            backup_template: DEFAULT_BACKUP_TEMPLATE.into(),
        }
    }
}

impl RemoteSettings {
    pub fn webdav_default() -> Self {
        Self {
            kind: "webdav".into(),
            endpoint: String::new(),
            region: String::new(),
            bucket: String::new(),
            access_key: String::new(),
            secret_key: String::new(),
            prefix: String::new(),
            backup_count: 3,
            backup_template: DEFAULT_BACKUP_TEMPLATE.into(),
        }
    }
}

/// One named remote configuration. Names are unique within a transport, so
/// `s3/config_1` and `webdav/config_1` are distinct canonical paths. The same
/// path is used for command resolution and local mirror directory hierarchy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RemoteProfile {
    pub name: String,
    pub settings: RemoteSettings,
}

impl Default for RemoteProfile {
    fn default() -> Self {
        Self {
            name: "config_1".into(),
            settings: RemoteSettings::default(),
        }
    }
}

impl RemoteProfile {
    pub fn webdav_default() -> Self {
        Self {
            name: "config_1".into(),
            settings: RemoteSettings::webdav_default(),
        }
    }

    pub fn path(&self) -> String {
        format!("{}/{}", self.settings.kind, self.name)
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

/// KeePassRPC (Kee 4.x) bridge. The loopback server only runs while
/// `enabled`; SRP keys are session-held (never persisted). When
/// `keep_session_after_lock` is set (default), locking the vault keeps the
/// SRP session keys in memory so a freshly unlocked vault is reused by the
/// Kee extension without a new side-channel password — matching the official
/// KeePassRPC behavior. The loopback server serves nothing sensitive while
/// locked regardless (data requests still answer "locked").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RpcSettings {
    pub enabled: bool,
    /// Keep SRP session keys across a vault lock so the extension reconnects
    /// without re-authorizing (official KeePassRPC behavior).
    #[serde(default = "default_true")]
    pub keep_session_after_lock: bool,
    /// Match the Domain tier by registrable domain (PSL) instead of strict
    /// host/subdomain. `false` (default) keeps SecPivot's strict behavior;
    /// `true` mirrors KeePassRPC, so `account.aliyun.com` and
    /// `passport.aliyun.com` both match an entry filed under `aliyun.com`.
    #[serde(default)]
    pub match_by_registrable_domain: bool,
    /// Lifetime of the SRP session keys in seconds, counted from the most
    /// recent vault unlock (every unlock resets the countdown). `0` (default)
    /// keeps them until the app exits or the user closes the database. When
    /// the timeout lapses the keys are wiped and the Kee extension must
    /// re-authorize with a fresh side-channel password.
    #[serde(default)]
    pub session_timeout_secs: u64,
}

impl Default for RpcSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            keep_session_after_lock: true,
            match_by_registrable_domain: false,
            session_timeout_secs: 0,
        }
    }
}

fn default_true() -> bool {
    true
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
    /// S3 and WebDAV profiles. Each profile path is `<kind>/<name>`.
    #[serde(default)]
    pub remote_profiles: Vec<RemoteProfile>,
    #[serde(default)]
    pub active_remote: String,
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
            remote_profiles: vec![RemoteProfile::default(), RemoteProfile::webdav_default()],
            active_remote: "s3/config_1".into(),
            bridge: BridgeSettings::default(),
            rpc: RpcSettings::default(),
            keyboard: KeyboardSettings::default(),
            favicon: FaviconSettings::default(),
        }
    }
}

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
    // An empty string is kept as-is so cleared custom-color inputs stay clear
    // while the user types a replacement (mirrors `validHex` in settings.ts);
    // only non-empty invalid values fall back.
    if value.is_empty() {
        return value.to_owned();
    }
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
    colors.link_color = valid_hex(&colors.link_color, &defaults.link_color);
    colors.success_color = valid_hex(&colors.success_color, &defaults.success_color);
    colors.danger_color = valid_hex(&colors.danger_color, &defaults.danger_color);
    colors.warning_color = valid_hex(&colors.warning_color, &defaults.warning_color);
    colors.scrollbar_color = valid_hex(&colors.scrollbar_color, &defaults.scrollbar_color);
    colors
}

/// Keeps saved-search names non-empty and unique (mirrors
/// `normalizeSavedSearches` in `src/lib/services/settings.ts`).
fn normalize_saved_searches(searches: Vec<SavedSearch>) -> Vec<SavedSearch> {
    let mut seen = HashSet::new();
    searches
        .into_iter()
        .map(|mut saved| {
            let base = saved.name.trim();
            let base = if base.is_empty() {
                "未命名搜索"
            } else {
                base
            };
            let mut name = base.to_owned();
            let mut suffix = 2;
            while !seen.insert(name.clone()) {
                name = format!("{base} {suffix}");
                suffix += 1;
            }
            saved.name = name;
            match saved.query.field.as_str() {
                "all" | "title" | "username" | "url" | "notes" | "tags" | "custom" => {}
                _ => saved.query.field = "all".into(),
            }
            saved
        })
        .collect()
}

/// Keep generator-profile names non-empty and unique, and clamp lengths to
/// the same 8..=128 boundary used by the frontend normalizer.
fn normalize_generator_profiles(
    profiles: Vec<PasswordGeneratorSettings>,
) -> Vec<PasswordGeneratorSettings> {
    let mut seen = HashSet::new();
    profiles
        .into_iter()
        .enumerate()
        .map(|(index, mut profile)| {
            let fallback = format!("配置 {}", index + 1);
            let base = profile
                .name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .unwrap_or(&fallback)
                .to_owned();
            let mut name = base.clone();
            let mut suffix = 2;
            while !seen.insert(name.clone()) {
                name = format!("{base} {suffix}");
                suffix += 1;
            }
            profile.name = Some(name);
            profile.length = profile.length.clamp(8, 128);
            profile
        })
        .collect()
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
    config.general.saved_searches = normalize_saved_searches(config.general.saved_searches);

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
    config.database.generator_profiles =
        normalize_generator_profiles(config.database.generator_profiles);
    config.database.file_extension = normalize_file_extension(&config.database.file_extension);

    if config.remote_profiles.is_empty() {
        config.remote_profiles = vec![RemoteProfile::default(), RemoteProfile::webdav_default()];
    }
    let mut kind_counts: HashMap<String, usize> = HashMap::new();
    for profile in &mut config.remote_profiles {
        normalize_remote_settings(&mut profile.settings);
        let count = kind_counts
            .entry(profile.settings.kind.clone())
            .and_modify(|value| *value += 1)
            .or_insert(1);
        profile.name = normalize_remote_profile_name(&profile.name, &format!("config_{}", *count));
    }
    if !kind_counts.contains_key("s3") {
        config.remote_profiles.push(RemoteProfile::default());
    }
    if !kind_counts.contains_key("webdav") {
        config.remote_profiles.push(RemoteProfile::webdav_default());
    }

    // Names only need to be unique within a transport namespace. This permits
    // both `s3/config_1` and `webdav/config_1` while keeping every canonical
    // path and local mirror directory unambiguous.
    let mut seen_names: HashMap<String, HashSet<String>> = HashMap::new();
    for profile in &mut config.remote_profiles {
        let base = profile.name.clone();
        let mut candidate = base.clone();
        let mut n = 2;
        let kind_seen = seen_names.entry(profile.settings.kind.clone()).or_default();
        while !kind_seen.insert(candidate.clone()) {
            candidate = format!("{base}_{n}");
            n += 1;
        }
        profile.name = candidate;
    }
    let requested_path = normalize_remote_profile_path(&config.active_remote);
    config.active_remote = requested_path
        .filter(|path| {
            config
                .remote_profiles
                .iter()
                .any(|profile| profile.path() == *path)
        })
        .unwrap_or_else(|| {
            config
                .remote_profiles
                .iter()
                .find(|profile| profile.settings.kind == "s3")
                .unwrap_or(&config.remote_profiles[0])
                .path()
        });

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
    settings.backup_count = clamp_i32(settings.backup_count, 0, 10, 3);
    settings.backup_template = settings.backup_template.trim().to_owned();
    if settings.backup_template.is_empty() {
        settings.backup_template = DEFAULT_BACKUP_TEMPLATE.into();
    }
    if settings.kind == "webdav" {
        settings.region.clear();
        settings.bucket.clear();
    }
}

fn normalize_remote_profile_name(name: &str, fallback: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return fallback.to_owned();
    }
    let normalized: String = trimmed
        .chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if normalized.is_empty() {
        fallback.to_owned()
    } else {
        normalized
    }
}

fn normalize_remote_profile_path(path: &str) -> Option<String> {
    let (kind, name) = path.trim().split_once('/')?;
    let kind = match kind.trim().to_ascii_lowercase().as_str() {
        "s3" => "s3",
        "webdav" => "webdav",
        _ => return None,
    };
    let name = normalize_remote_profile_name(name, "");
    if name.is_empty() {
        None
    } else {
        Some(format!("{kind}/{name}"))
    }
}

/// Sanitize a user-supplied file extension: drop the leading dot, keep only
/// alphanumeric characters, fall back to `kdbx` when nothing remains.
pub(crate) fn normalize_file_extension(ext: &str) -> String {
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
