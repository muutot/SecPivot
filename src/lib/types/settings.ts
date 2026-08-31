import type { ThemeColors } from "./theme";
import type { AdvancedSearchQuery } from "$lib/utils/entry-search";

export type ThemeMode = "dark" | "light" | "custom";
export type WindowEffect = "off" | "acrylic" | "mica";
export type Language = "en" | "zh-CN";
export type Kdf = "Argon2id" | "Argon2" | "Aes";
export type Cipher = "Aes256" | "ChaCha20";
export type Compression = "None" | "Gzip";
export type RemoteKind = "s3" | "webdav";
export type RemoteProfilePath = `${RemoteKind}/${string}`;

export interface SavedSearch {
  name: string;
  query: AdvancedSearchQuery;
}

export interface PasswordGeneratorSettings {
  /** Profile name; absent on the built-in default. */
  name?: string;
  length: number;
  includeUpper: boolean;
  includeLower: boolean;
  includeDigits: boolean;
  includeSymbols: boolean;
  excludeSimilar: boolean;
  excludeAmbiguous: boolean;
  /** Replaces the built-in character classes entirely when non-empty. */
  customCharset?: string;
  /** Characters removed from every pool. */
  excludeChars?: string;
  /** Each character must appear at least once in the result. */
  requiredChars?: string;
  /** KeePass-style pattern (`u`/`l`/`d`/`s`/`a`, other chars literal). */
  pattern?: string;
}

export interface DatabaseDefaults {
  kdf: Kdf;
  cipher: Cipher;
  compression: Compression;
  generator: PasswordGeneratorSettings;
  /** Named generator profiles selectable for new entries. */
  generatorProfiles: PasswordGeneratorSettings[];
  /** Default file extension (no leading dot) for "另存为" and as the backup fallback. */
  fileExtension: string;
}

interface RemoteCommonSettings {
  /** Optional key prefix (folder) used by the remote file browser. */
  prefix: string;
  /** Number of timestamped `.bak` backups kept beside the local file/remote; 0 disables. */
  backupCount: number;
  /** Backup file name template. Placeholders: `{name}` (file stem),
   * `{timestamp}` (`YYYYMMDDHHmmssSSS`), `{ext}` (original extension). */
  backupTemplate: string;
}

/** One S3-compatible object-storage configuration. */
export interface RemoteS3Settings extends RemoteCommonSettings {
  kind: "s3";
  endpoint: string;
  region: string;
  bucket: string;
  accessKey: string;
  secretKey: string;
}

/** One WebDAV configuration. `endpoint` is the WebDAV base URL; access
 * credentials are sent as HTTP Basic auth. */
export interface RemoteWebDavSettings extends RemoteCommonSettings {
  kind: "webdav";
  endpoint: string;
  accessKey: string;
  secretKey: string;
}

/** A profile stores exactly one transport shape; the `kind` discriminant
 * prevents one configuration from carrying both S3 and WebDAV fields. */
export type RemoteSettings = RemoteS3Settings | RemoteWebDavSettings;

/** One named remote configuration. Names are unique within their transport,
 * producing canonical paths such as `s3/config_1` and `webdav/config_1`. */
export interface RemoteProfile {
  name: string;
  settings: RemoteSettings;
}

export interface SecuritySettings {
  autoLockMinutes: number;
  clipboardClearSeconds: number;
  minimizeToTray: boolean;
  clearOnLock: boolean;
  lockAfterAction: boolean;
  lockOnFocusLoss: boolean;
  /** Keep the master password in the OS credential store for Windows Hello unlock. */
  rememberPassword: boolean;
  /** Exclude the main window from screenshots/recordings while a vault is open (Windows WDA_EXCLUDEFROMCAPTURE). Default off — opt-in from the welcome page. */
  screenCaptureGuard: boolean;
}

export interface DensitySettings {
  groupGap: number;
  groupPaddingY: number;
  groupIndent: number;
  groupRadius: number;
  showGroupIcon: boolean;
  showGroupChevron: boolean;
}

/** One entry-table column's persisted state (KeePass-style list). `id` is a
 *  built-in column id ("title", "username", "password", "url", "totp",
 *  "notes", "tags", "created", "modified", "expires", "size") or
 *  `custom:<field name>` for entry custom fields. `width` is px; the "title"
 *  column uses `0` as an auto sentinel (frontend resolves it to the default
 *  column width). Mirrors `EntryColumnState` in `src-tauri/src/config.rs`. */
export interface EntryColumnState {
  id: string;
  visible: boolean;
  width: number;
}

/** User-resizable pane widths of the main view, remembered across restarts. */
export interface PanelWidths {
  group: number;
  detail: number;
  /** URL column; floor is header chars × 10px + 10px ("网址" → 30).
   *  Kept in sync with the "url" entry column's width for legacy configs. */
  urlCol: number;
}

/** Per-item visibility for secondary toolbar/window actions.
 *  `true` = show directly on the main toolbar (or window chrome);
 *  `false` = collect inside the More menu (or hidden for window buttons). */
export interface ToolbarItemVisibility {
  newEntry: boolean;
  save: boolean;
  saveAs: boolean;
  lock: boolean;
  toggleDetail: boolean;
  securityReport: boolean;
  similarPasswords: boolean;
  hibpCheck: boolean;
  importMenu: boolean;
  exportMenu: boolean;
  expiredEntries: boolean;
  clearHistory: boolean;
  dbSettings: boolean;
  appSettings: boolean;
  windowMinimize: boolean;
  windowMaximize: boolean;
  windowClose: boolean;
}

/** All toolbar button ids that participate in global ordering (moreMenu is always visible, not in visibility map). */
export type ToolbarButtonId =
  | "newEntry"
  | "save"
  | "saveAs"
  | "lock"
  | "toggleDetail"
  | "securityReport"
  | "similarPasswords"
  | "hibpCheck"
  | "expiredEntries"
  | "clearHistory"
  | "importMenu"
  | "exportMenu"
  | "dbSettings"
  | "appSettings"
  | "moreMenu"
  | "windowMinimize"
  | "windowMaximize"
  | "windowClose";

/** Ordered toolbar ids for the configurable right side (legacy alias). */
export type ToolbarRightId =
  | "toggleDetail"
  | "securityReport"
  | "similarPasswords"
  | "hibpCheck"
  | "expiredEntries"
  | "clearHistory"
  | "importMenu"
  | "exportMenu"
  | "dbSettings"
  | "appSettings";

export interface CustomTheme {
  name: string;
  colors: ThemeColors;
}

export interface GeneralSettings {
  language: Language;
  theme: ThemeMode;
  themeColors: ThemeColors;
  customPresets: ThemeColors[];
  customThemes: CustomTheme[];
  compactMode: boolean;
  density: DensitySettings;
  showDescriptions: boolean;
  fontSizes: {
    base: number;
    secondary: number;
    cardTitle: number;
    cardPreview: number;
  };
  windowEffect: WindowEffect;
  windowOpacity: number;
  rememberLastDatabase: boolean;
  recentFiles: string[];
  /** Main-window size remembered from the user's resize; the welcome screen uses a smaller fixed size. */
  windowWidth: number;
  windowHeight: number;
  panelWidths: PanelWidths;
  /** Toolbar control buttons show icons only; button names appear in hover tooltips. */
  iconOnlyButtons: boolean;
  /** Collect lower-frequency toolbar actions in a shared More menu (legacy, kept for migration). */
  toolbarOverflowMenu: boolean;
  /** Per-item visibility for secondary actions (toolbar + window controls). */
  toolbarItems: ToolbarItemVisibility;
  /** Ordered ids for the right toolbar group; controls sort order on the main toolbar (legacy right-only). */
  toolbarOrder: ToolbarRightId[];
  /** Ids after which a vertical divider is rendered on the toolbar (legacy right-only). */
  toolbarSeparators: ToolbarRightId[];
  /** Global ordered ids for all toolbar buttons (including left group and moreMenu); controls full sort order. */
  toolbarFullOrder: ToolbarButtonId[];
  /** Per-button side assignment: 'left' or 'right' (moreMenu follows its position in fullOrder but still renders as More button). */
  toolbarSides: Record<ToolbarButtonId, "left" | "right">;
  /** Global separators: ids after which a vertical divider is rendered (applies within the same side group). */
  toolbarFullSeparators: ToolbarButtonId[];
  /** Whether clicking an entry automatically shows the detail panel. */
  showDetailOnSelect: boolean;
  /** Render the full entry-table column grid on narrow screens too. */
  mobileColumns: boolean;
  /** Entry-table column layout (visible + px width per column id). */
  entryColumns: EntryColumnState[];
  /** Named advanced-search queries. */
  savedSearches: SavedSearch[];
}

/** 快捷键 section: the global auto-type hotkey plus app-window shortcuts
 *  for common actions. Mirrors `KeyboardSettings` in `src-tauri/src/config.rs`. */
export interface KeyboardSettings {
  /** Global auto-type hotkey (accelerator syntax, e.g. "Ctrl+Shift+A"); empty disables it. */
  autoTypeGlobal: string;
  /** App-window shortcuts: action id → accelerator. An absent key or empty value means unbound. */
  shortcuts: Record<string, string>;
}

/** KeePassHttp browser bridge. The loopback server runs only while enabled;
 * association keys are session-held and wiped on vault lock. */
export interface BridgeSettings {
  enabled: boolean;
}

/** KeePassRPC (Kee 4.x) bridge. Loopback-only server; SRP keys are
 * session-held, and the side-channel password is shown once per connection. */
export interface RpcSettings {
  enabled: boolean;
  /** Keep SRP session keys across a vault lock so the extension reconnects
   * without re-authorizing (official KeePassRPC behavior). */
  keepSessionAfterLock: boolean;
  /** Match the Domain tier by registrable domain (PSL) instead of strict
   * host/subdomain. Enables the official KeePassRPC behavior where sibling
   * hosts under one domain (e.g. `account.aliyun.com` / `passport.aliyun.com`
   * under `aliyun.com`) all match an entry. */
  matchByRegistrableDomain: boolean;
  /** SRP session-key lifetime in seconds, counted from the most recent vault
   *  unlock (each unlock resets it). `0` (default) = never expire. */
  sessionTimeoutSecs: number;
}

/** Favicon download behavior ("Download Favicons").
 *  Mirrors `FaviconSettings` in `src-tauri/src/config.rs`. */
export interface FaviconSettings {
  /** How many distinct hosts may be fetched at once (1-16, default 8). */
  concurrency: number;
  /** Persist the database right after applying downloaded icons. Off by
   * default: icons apply to the open session and the vault is left dirty
   * for a manual save. */
  autoSave: boolean;
}

export interface AppSettings {
  general: GeneralSettings;
  security: SecuritySettings;
  database: DatabaseDefaults;
  /** S3 and WebDAV configurations share one ordered collection but remain
   * separated by their discriminated `settings.kind`. */
  remoteProfiles: RemoteProfile[];
  /** Canonical active profile path, e.g. `s3/config_1`. */
  activeRemote: RemoteProfilePath;
  bridge: BridgeSettings;
  rpc: RpcSettings;
  keyboard: KeyboardSettings;
  favicon: FaviconSettings;
}
