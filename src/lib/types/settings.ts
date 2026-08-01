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

export interface SecuritySettings {
  autoLockMinutes: number;
  clipboardClearSeconds: number;
  minimizeToTray: boolean;
  clearOnLock: boolean;
  lockAfterAction: boolean;
  lockOnFocusLoss: boolean;
}

export interface DensitySettings {
  groupGap: number;
  groupPaddingY: number;
  groupIndent: number;
  groupRadius: number;
  showGroupIcon: boolean;
  showGroupChevron: boolean;
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
}

export interface AppSettings {
  general: GeneralSettings;
  security: SecuritySettings;
  database: DatabaseDefaults;
}
