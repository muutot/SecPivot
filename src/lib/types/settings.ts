import type { ThemeColors } from "./theme";

export type ThemeMode = "dark" | "light" | "custom";
export type WindowEffect = "off" | "acrylic" | "mica";
export type Language = "en" | "zh-CN";
export type Kdf = "Argon2id" | "Argon2" | "Aes";
export type Cipher = "Aes256" | "ChaCha20";
export type Compression = "None" | "Gzip";

export interface PasswordGeneratorSettings {
  length: number;
  includeUpper: boolean;
  includeLower: boolean;
  includeDigits: boolean;
  includeSymbols: boolean;
  excludeSimilar: boolean;
  excludeAmbiguous: boolean;
}

export interface DatabaseDefaults {
  kdf: Kdf;
  cipher: Cipher;
  compression: Compression;
  generator: PasswordGeneratorSettings;
  /** Default file extension (no leading dot) for "另存为" and as the backup fallback. */
  fileExtension: string;
}

/** S3-compatible object-storage connection. Fields are independent of the
 * WebDAV block so switching transports never loses or mixes credentials. */
export interface RemoteS3Settings {
  endpoint: string;
  region: string;
  bucket: string;
  accessKey: string;
  secretKey: string;
}

/** WebDAV connection settings. `endpoint` is the WebDAV base URL; access
 *  credentials are sent as HTTP Basic auth. Independent of the S3 block. */
export interface RemoteWebDavSettings {
  endpoint: string;
  accessKey: string;
  secretKey: string;
}

/** Remote vault settings. `kind` selects which transport block is active
 * (`"s3"` or `"webdav"`); the two blocks hold isolated credentials/URLs so the
 * S3 and WebDAV configs never contaminate each other. Access credentials are
 * secondary credentials, never a vault master password (see `security-model.md`). */
export interface RemoteSettings {
  kind: string;
  /** S3-compatible object-storage connection. Ignored when `kind !== "s3"`. */
  s3: RemoteS3Settings;
  /** WebDAV connection (Basic-auth username/password). Ignored when `kind !== "webdav"`. */
  webdav: RemoteWebDavSettings;
  /** Optional key prefix (folder) used by the remote file browser. */
  prefix: string;
  /** Number of timestamped `.bak` backups kept beside the local file/remote; 0 disables. */
  backupCount: number;
  /** Backup file name template. Placeholders: `{name}` (file stem),
   * `{timestamp}` (`YYYYMMDDHHmmssSSS`), `{ext}` (original extension). */
  backupTemplate: string;
}

/** One named S3 configuration shown in the profile selector. The name must be
 *  unique across profiles — it also names the local mirror folder
 *  (`Storage/remote/<sanitized name>` for "保存到本地" mode). */
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
 *  "notes", "tags", "created", "modified", "expires") or `custom:<field name>`
 *  for entry custom fields. `width` is px; the "title" column uses `0` as an
 *  auto sentinel (frontend resolves it to the default column width). Mirrors
 *  `EntryColumnState` in `src-tauri/src/config.rs`. */
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

export interface GeneralSettings {
  language: Language;
  theme: ThemeMode;
  themeColors: ThemeColors;
  customPresets: ThemeColors[];
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
  /** Collect lower-frequency toolbar actions in a shared More menu. */
  toolbarOverflowMenu: boolean;
  /** Entry-table column layout (visible + px width per column id). */
  entryColumns: EntryColumnState[];
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
}

/** Favicon download behavior ("Download Favicons").
 *  Mirrors `FaviconSettings` in `src-tauri/src/config.rs`. */
export interface FaviconSettings {
  /** How many distinct hosts may be fetched at once (1-16, default 8). */
  concurrency: number;
}

export interface AppSettings {
  general: GeneralSettings;
  security: SecuritySettings;
  database: DatabaseDefaults;
  /** Named S3 configurations; `activeRemote` selects the one commands use. */
  remoteProfiles: RemoteProfile[];
  /** Index into `remoteProfiles` (clamped to a valid index on normalize). */
  activeRemote: number;
  /** Active profile's settings — kept in sync as a convenience surface for
   * the settings UI; sent as `null` by the backend on load. */
  remote: RemoteSettings;
  bridge: BridgeSettings;
  rpc: RpcSettings;
  keyboard: KeyboardSettings;
  favicon: FaviconSettings;
}
