import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  GeneralSettings,
  SecuritySettings,
  DatabaseDefaults,
  RemoteSettings,
  RemoteProfile,
  BridgeSettings,
  RpcSettings,
  KeyboardSettings,
} from "$lib/types/settings";
import { DARK_THEME_COLORS, LIGHT_THEME_COLORS, type ThemeColors } from "$lib/types/theme";
import { KEYBOARD_ACTIONS } from "$lib/services/keyboard";

export const PERSIST_DEBOUNCE_MS = 120;

export const RECENT_FILES_MAX = 8;

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

export const DEFAULT_GENERAL_SETTINGS: GeneralSettings = {
  language: "zh-CN",
  theme: "dark",
  themeColors: { ...DARK_THEME_COLORS },
  customPresets: [],
  compactMode: false,
  density: {
    groupGap: 2,
    groupPaddingY: 3,
    groupIndent: 12,
    groupRadius: 6,
    showGroupIcon: true,
    showGroupChevron: true,
  },
  showDescriptions: true,
  fontSizes: { base: 14, secondary: 11, cardTitle: 13, cardPreview: 11 },
  windowEffect: "off",
  windowOpacity: 100,
  rememberLastDatabase: true,
  recentFiles: [],
  windowWidth: 1100,
  windowHeight: 720,
  panelWidths: { group: 200, detail: 300, urlCol: 200 },
};

export const DEFAULT_SECURITY_SETTINGS: SecuritySettings = {
  autoLockMinutes: 5,
  clipboardClearSeconds: 20,
  minimizeToTray: true,
  clearOnLock: true,
  lockAfterAction: false,
  lockOnFocusLoss: false,
  rememberPassword: false,
  screenCaptureGuard: false,
};

export const DEFAULT_DATABASE_SETTINGS: DatabaseDefaults = {
  kdf: "Argon2id",
  cipher: "Aes256",
  compression: "Gzip",
  generator: {
    length: 20,
    includeUpper: true,
    includeLower: true,
    includeDigits: true,
    includeSymbols: true,
    excludeSimilar: false,
    excludeAmbiguous: false,
  },
};

export const DEFAULT_REMOTE_SETTINGS: RemoteSettings = {
  endpoint: "https://s3.amazonaws.com",
  region: "us-east-1",
  bucket: "",
  accessKey: "",
  secretKey: "",
  prefix: "",
  localDir: "remote",
  backupCount: 3,
};

export const DEFAULT_BRIDGE_SETTINGS: BridgeSettings = {
  enabled: false,
};

export const DEFAULT_RPC_SETTINGS: RpcSettings = {
  enabled: false,
};

/** App-window shortcuts start unbound; the panel shows the action's `default`
 *  accelerator until the user records a binding. */
export const DEFAULT_KEYBOARD_SETTINGS: KeyboardSettings = {
  autoTypeGlobal: "",
  shortcuts: {},
};

export const DEFAULT_APP_SETTINGS: AppSettings = {
  general: DEFAULT_GENERAL_SETTINGS,
  security: DEFAULT_SECURITY_SETTINGS,
  database: DEFAULT_DATABASE_SETTINGS,
  remoteProfiles: [{ name: "默认", settings: DEFAULT_REMOTE_SETTINGS }],
  activeRemote: 0,
  remote: DEFAULT_REMOTE_SETTINGS,
  bridge: DEFAULT_BRIDGE_SETTINGS,
  rpc: DEFAULT_RPC_SETTINGS,
  keyboard: DEFAULT_KEYBOARD_SETTINGS,
};

const hexColor = /^#[0-9a-fA-F]{6}$|^#[0-9a-fA-F]{8}$/;

function clampInt(value: number, min: number, max: number, fallback: number): number {
  if (typeof value !== "number" || Number.isNaN(value)) return fallback;
  return Math.min(max, Math.max(min, Math.round(value)));
}

function validHex(value: string, fallback: string): string {
  return typeof value === "string" && hexColor.test(value) ? value : fallback;
}

/** Trim, dedup (keep first occurrence), and cap the recent-files list. */
export function normalizeRecentFiles(files: unknown): string[] {
  if (!Array.isArray(files)) return [];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const file of files) {
    const trimmed = String(file).trim();
    if (!trimmed || seen.has(trimmed)) continue;
    seen.add(trimmed);
    out.push(trimmed);
    if (out.length >= RECENT_FILES_MAX) break;
  }
  return out;
}

export function normalizeThemeColors(value: unknown, fallback: ThemeColors): ThemeColors {
  const base: ThemeColors = { ...fallback };
  if (!value || typeof value !== "object") return base;
  const source = value as Partial<ThemeColors>;
  for (const key of Object.keys(base) as (keyof ThemeColors)[]) {
    base[key] = validHex(String(source[key] ?? base[key]), base[key]);
  }
  return base;
}

export function normalizeRemoteSettings(
  source: Partial<RemoteSettings> | undefined,
  fallback: RemoteSettings = DEFAULT_REMOTE_SETTINGS,
): RemoteSettings {
  const r = source ?? {};
  const str = (key: keyof RemoteSettings, fb: string): string => {
    const value = typeof r[key] === "string" ? String(r[key]).trim() : "";
    return value || fb;
  };
  return {
    endpoint: str("endpoint", fallback.endpoint),
    region: str("region", fallback.region),
    bucket: str("bucket", fallback.bucket),
    accessKey: str("accessKey", fallback.accessKey),
    secretKey: str("secretKey", fallback.secretKey),
    prefix: str("prefix", fallback.prefix),
    localDir: str("localDir", fallback.localDir),
    backupCount: clampInt(
      typeof r.backupCount === "number" ? r.backupCount : fallback.backupCount,
      0,
      10,
      3,
    ),
  };
}

/** Normalize `remoteProfiles` (at least one profile always survives), with a
 * legacy single `remote` object promoted to the first profile. */
export function normalizeRemoteProfiles(
  source: Partial<RemoteProfile>[] | undefined,
  legacy: Partial<RemoteSettings> | undefined,
  fallback: RemoteSettings,
): RemoteProfile[] {
  const profiles = Array.isArray(source)
    ? source.map((p, i) => ({
        name: String(p?.name ?? "").trim() || (i === 0 ? "默认" : `配置 ${i + 1}`),
        settings: normalizeRemoteSettings(p?.settings, fallback),
      }))
    : [];
  if (profiles.length > 0) return profiles;
  return [{ name: "默认", settings: normalizeRemoteSettings(legacy ?? undefined, fallback) }];
}

export function normalizeSettings(
  source: Partial<AppSettings>,
  fallback: AppSettings = DEFAULT_APP_SETTINGS,
): AppSettings {
  const g = source.general ?? fallback.general;
  const general: GeneralSettings = {
    ...fallback.general,
    ...(typeof g === "object" ? g : {}),
    theme:
      g.theme === "light" || g.theme === "custom"
        ? g.theme
        : g.theme === "dark"
          ? "dark"
          : fallback.general.theme,
    themeColors: normalizeThemeColors(g.themeColors, DARK_THEME_COLORS),
    customPresets: Array.isArray(g.customPresets)
      ? g.customPresets.map((p: unknown) => normalizeThemeColors(p, DARK_THEME_COLORS))
      : [],
    fontSizes: {
      ...fallback.general.fontSizes,
      ...(g.fontSizes ?? {}),
      base: clampInt(g.fontSizes?.base ?? fallback.general.fontSizes.base, 11, 20, 14),
      secondary: clampInt(
        g.fontSizes?.secondary ?? fallback.general.fontSizes.secondary,
        9,
        16,
        11,
      ),
      cardTitle: clampInt(
        g.fontSizes?.cardTitle ?? fallback.general.fontSizes.cardTitle,
        11,
        18,
        13,
      ),
      cardPreview: clampInt(
        g.fontSizes?.cardPreview ?? fallback.general.fontSizes.cardPreview,
        9,
        16,
        11,
      ),
    },
    density: {
      ...fallback.general.density,
      ...(g.density ?? {}),
      groupGap: clampInt(g.density?.groupGap ?? fallback.general.density.groupGap, 0, 16, 2),
      groupPaddingY: clampInt(
        g.density?.groupPaddingY ?? fallback.general.density.groupPaddingY,
        0,
        16,
        3,
      ),
      groupIndent: clampInt(
        g.density?.groupIndent ?? fallback.general.density.groupIndent,
        4,
        32,
        12,
      ),
      groupRadius: clampInt(
        g.density?.groupRadius ?? fallback.general.density.groupRadius,
        0,
        12,
        6,
      ),
      showGroupIcon:
        typeof g.density?.showGroupIcon === "boolean"
          ? g.density.showGroupIcon
          : fallback.general.density.showGroupIcon,
      showGroupChevron:
        typeof g.density?.showGroupChevron === "boolean"
          ? g.density.showGroupChevron
          : fallback.general.density.showGroupChevron,
    },
    windowOpacity: clampInt(g.windowOpacity ?? fallback.general.windowOpacity, 40, 100, 100),
    windowWidth: clampInt(g.windowWidth ?? fallback.general.windowWidth, 560, 2560, 1100),
    windowHeight: clampInt(g.windowHeight ?? fallback.general.windowHeight, 420, 1600, 720),
    panelWidths: {
      ...fallback.general.panelWidths,
      ...(g.panelWidths ?? {}),
      group: clampInt(g.panelWidths?.group ?? fallback.general.panelWidths.group, 140, 320, 200),
      detail: clampInt(g.panelWidths?.detail ?? fallback.general.panelWidths.detail, 260, 640, 300),
      urlCol: clampInt(g.panelWidths?.urlCol ?? fallback.general.panelWidths.urlCol, 30, 400, 200),
    },
    recentFiles: normalizeRecentFiles(g.recentFiles),
    language:
      g.language === "en" || g.language === "zh-CN" ? g.language : fallback.general.language,
    windowEffect:
      g.windowEffect === "acrylic" || g.windowEffect === "mica" ? g.windowEffect : "off",
  };

  const s = source.security ?? fallback.security;
  const security: SecuritySettings = {
    ...fallback.security,
    ...(typeof s === "object" ? s : {}),
    autoLockMinutes: clampInt(s.autoLockMinutes ?? fallback.security.autoLockMinutes, 0, 240, 5),
    clipboardClearSeconds: clampInt(
      s.clipboardClearSeconds ?? fallback.security.clipboardClearSeconds,
      0,
      600,
      20,
    ),
  };

  const d = source.database ?? fallback.database;
  const database: DatabaseDefaults = {
    ...fallback.database,
    ...(typeof d === "object" ? d : {}),
    generator: {
      ...fallback.database.generator,
      ...(typeof d.generator === "object" ? d.generator : {}),
      length: clampInt(d.generator?.length ?? fallback.database.generator.length, 8, 128, 20),
    },
  };

  const remoteProfiles = normalizeRemoteProfiles(
    source.remoteProfiles,
    source.remote,
    fallback.remote,
  );
  const activeRemote = clampInt(
    source.activeRemote ?? fallback.activeRemote,
    0,
    remoteProfiles.length - 1,
    0,
  );
  const remote = remoteProfiles[activeRemote].settings;

  const k = source.keyboard ?? fallback.keyboard;
  const legacyGlobal = (source.general as unknown as {
    globalAutoTypeShortcut?: unknown;
  })?.globalAutoTypeShortcut;
  const keyboard: KeyboardSettings = {
    autoTypeGlobal:
      typeof k?.autoTypeGlobal === "string" && k.autoTypeGlobal.trim()
        ? k.autoTypeGlobal.trim()
        : typeof legacyGlobal === "string" && legacyGlobal.trim()
          ? legacyGlobal.trim()
          : (fallback.keyboard?.autoTypeGlobal ?? ""),
    shortcuts: {},
  };
  if (k && typeof k.shortcuts === "object" && k.shortcuts !== null) {
    for (const action of KEYBOARD_ACTIONS) {
      const value = (k.shortcuts as Record<string, unknown>)[action.id];
      if (typeof value === "string" && value.trim()) {
        keyboard.shortcuts[action.id] = value.trim();
      }
    }
  }

  return {
    general,
    security,
    database,
    remoteProfiles,
    activeRemote,
    remote,
    bridge: {
      enabled:
        typeof source.bridge?.enabled === "boolean"
          ? source.bridge.enabled
          : (fallback.bridge?.enabled ?? false),
    },
    rpc: {
      enabled:
        typeof source.rpc?.enabled === "boolean"
          ? source.rpc.enabled
          : (fallback.rpc?.enabled ?? false),
    },
    keyboard,
  };
}

interface AppSettingsStore {
  subscribe: typeof settings.subscribe;
  initialize: () => Promise<void>;
  updateGeneral: <K extends keyof GeneralSettings>(key: K, value: GeneralSettings[K]) => void;
  updateSecurity: <K extends keyof SecuritySettings>(key: K, value: SecuritySettings[K]) => void;
  updateDatabase: <K extends keyof DatabaseDefaults>(key: K, value: DatabaseDefaults[K]) => void;
  /** Update a field of the ACTIVE profile's settings (kept in `remote` too). */
  updateRemote: <K extends keyof RemoteSettings>(key: K, value: RemoteSettings[K]) => void;
  setActiveRemote: (index: number) => void;
  addRemoteProfile: (name: string) => void;
  removeRemoteProfile: (index: number) => void;
  renameRemoteProfile: (index: number, name: string) => void;
  updateBridge: <K extends keyof BridgeSettings>(key: K, value: BridgeSettings[K]) => void;
  updateRpc: <K extends keyof RpcSettings>(key: K, value: RpcSettings[K]) => void;
  updateKeyboard: <K extends keyof KeyboardSettings>(
    key: K,
    value: KeyboardSettings[K],
  ) => void;
  merge: (partial: Partial<AppSettings>) => void;
  flush: () => Promise<void>;
  destroy: () => void;
}

const settings = writable<AppSettings>(DEFAULT_APP_SETTINGS);

const STORAGE_KEY = "keyvault-settings";

let dirty = false;
let pending: AppSettings | null = null;
let inFlight: Promise<void> | null = null;
let persistTimer: ReturnType<typeof setTimeout> | null = null;
let initialized = false;

async function persist(): Promise<void> {
  if (!dirty || !pending) return;
  const value = pending;
  pending = null;
  dirty = false;
  if (isTauriRuntime()) {
    try {
      const saved = await invoke<AppSettings>("set_config", { config: value });
      settings.set(normalizeSettings(saved, value));
    } catch {
      pending = value;
      dirty = true;
    }
  } else {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(value));
    } catch {
      // storage unavailable; keep running without persistence
    }
  }
}

function schedulePersist(): void {
  dirty = true;
  pending = normalizeSettings(get(settings));
  if (persistTimer) clearTimeout(persistTimer);
  persistTimer = setTimeout(() => {
    persistTimer = null;
    if (inFlight) return;
    inFlight = persist().finally(() => {
      inFlight = null;
      if (persistTimer) {
        persistTimer = null;
        void persist();
      }
    });
  }, PERSIST_DEBOUNCE_MS);
}

export const appSettings: AppSettingsStore = {
  subscribe: settings.subscribe,

  async initialize(): Promise<void> {
    if (initialized) return;
    initialized = true;
    if (isTauriRuntime()) {
      try {
        const loaded = await invoke<AppSettings>("get_config");
        settings.set(normalizeSettings(loaded, DEFAULT_APP_SETTINGS));
      } catch {
        settings.set(DEFAULT_APP_SETTINGS);
      }
    } else {
      try {
        const raw = localStorage.getItem(STORAGE_KEY);
        if (raw) {
          const parsed = JSON.parse(raw) as AppSettings;
          settings.set(normalizeSettings(parsed, DEFAULT_APP_SETTINGS));
        }
      } catch {
        settings.set(DEFAULT_APP_SETTINGS);
      }
      window.addEventListener("storage", (event) => {
        if (event.key === STORAGE_KEY && event.newValue) {
          try {
            settings.set(
              normalizeSettings(JSON.parse(event.newValue) as AppSettings, get(settings)),
            );
          } catch {
            // ignore malformed cross-tab update
          }
        }
      });
    }
  },

  updateGeneral(key, value): void {
    settings.update((s) => {
      const next = { ...s, general: { ...s.general, [key]: value } };
      if (key === "theme" && value !== "custom") {
        next.general.themeColors =
          value === "light" ? { ...LIGHT_THEME_COLORS } : { ...DARK_THEME_COLORS };
      }
      return next;
    });
    schedulePersist();
  },

  updateSecurity(key, value): void {
    settings.update((s) => ({ ...s, security: { ...s.security, [key]: value } }));
    schedulePersist();
  },

  updateDatabase(key, value): void {
    settings.update((s) => ({ ...s, database: { ...s.database, [key]: value } }));
    schedulePersist();
  },

  updateRemote(key, value): void {
    settings.update((s) => {
      const remoteProfiles = s.remoteProfiles.map((p, i) =>
        i === s.activeRemote ? { ...p, settings: { ...p.settings, [key]: value } } : p,
      );
      return { ...s, remoteProfiles, remote: remoteProfiles[s.activeRemote].settings };
    });
    schedulePersist();
  },

  setActiveRemote(index): void {
    settings.update((s) => {
      const activeRemote = Math.min(Math.max(0, Math.round(index)), s.remoteProfiles.length - 1);
      return { ...s, activeRemote, remote: s.remoteProfiles[activeRemote].settings };
    });
    schedulePersist();
  },

  addRemoteProfile(name): void {
    settings.update((s) => {
      const remoteProfiles = [
        ...s.remoteProfiles,
        {
          name: name.trim() || `配置 ${s.remoteProfiles.length + 1}`,
          settings: { ...DEFAULT_REMOTE_SETTINGS },
        },
      ];
      const activeRemote = remoteProfiles.length - 1;
      return { ...s, remoteProfiles, activeRemote, remote: remoteProfiles[activeRemote].settings };
    });
    schedulePersist();
  },

  removeRemoteProfile(index): void {
    settings.update((s) => {
      if (s.remoteProfiles.length <= 1) return s;
      const remoteProfiles = s.remoteProfiles.filter((_, i) => i !== index);
      const activeRemote = Math.min(Math.max(0, Math.round(index)), remoteProfiles.length - 1);
      return { ...s, remoteProfiles, activeRemote, remote: remoteProfiles[activeRemote].settings };
    });
    schedulePersist();
  },

  renameRemoteProfile(index, name): void {
    settings.update((s) => {
      const remoteProfiles = s.remoteProfiles.map((p, i) => (i === index ? { ...p, name } : p));
      return { ...s, remoteProfiles, remote: remoteProfiles[s.activeRemote].settings };
    });
    schedulePersist();
  },

  updateBridge(key, value): void {
    settings.update((s) => ({ ...s, bridge: { ...s.bridge, [key]: value } }));
    schedulePersist();
  },

  updateRpc(key, value): void {
    settings.update((s) => ({ ...s, rpc: { ...s.rpc, [key]: value } }));
    schedulePersist();
  },

  updateKeyboard(key, value): void {
    settings.update((s) => ({ ...s, keyboard: { ...s.keyboard, [key]: value } }));
    schedulePersist();
  },

  merge(partial): void {
    settings.update((s) => normalizeSettings({ ...s, ...partial }, s));
    schedulePersist();
  },

  async flush(): Promise<void> {
    if (persistTimer) {
      clearTimeout(persistTimer);
      persistTimer = null;
    }
    if (dirty && pending) {
      await persist();
    }
  },

  destroy(): void {
    if (persistTimer) clearTimeout(persistTimer);
  },
};

export function selectThemeColors(s: AppSettings): ThemeColors {
  if (s.general.theme === "light") return LIGHT_THEME_COLORS;
  if (s.general.theme === "custom") return s.general.themeColors;
  return DARK_THEME_COLORS;
}
