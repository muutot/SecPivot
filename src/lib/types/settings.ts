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
}

/** S3-compatible remote vault settings. Access keys are encrypted at rest
 * with Windows DPAPI (`dpapi1:` prefix in `config.json`) — a secondary
 * credential, never a vault master password (see `security-model.md`). */
export interface RemoteSettings {
  endpoint: string;
  region: string;
  bucket: string;
  accessKey: string;
  secretKey: string;
  /** Optional key prefix (folder) used by the remote file browser. */
  prefix: string;
  /** Subdirectory under `Storage/remote/` for local copies ("保存到本地" mode). */
  localDir: string;
  /** Number of timestamped `.bak` backups kept beside the local copy; 0 disables. */
  backupCount: number;
}

/** One named S3 configuration shown in the profile selector. */
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

/** User-resizable pane widths of the main view, remembered across restarts. */
export interface PanelWidths {
  group: number;
  detail: number;
  /** URL column; floor is header chars × 10px + 10px ("网址" → 30). */
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
  /** Global auto-type hotkey (accelerator syntax, e.g. "Ctrl+Shift+A"); empty disables it. */
  globalAutoTypeShortcut: string;
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
}
