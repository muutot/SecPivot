import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import type {
  AppSettings,
  GeneralSettings,
  SecuritySettings,
  DatabaseDefaults,
  RemoteSettings,
  RemoteS3Settings,
  RemoteWebDavSettings,
  RemoteProfile,
  RemoteKind,
  RemoteProfilePath,
  BridgeSettings,
  RpcSettings,
  KeyboardSettings,
  FaviconSettings,
  EntryColumnState,
} from "$lib/types/settings";
import { DARK_THEME_COLORS, LIGHT_THEME_COLORS, type ThemeColors } from "$lib/types/theme";
import { KEYBOARD_ACTIONS } from "$lib/services/keyboard";

export const PERSIST_DEBOUNCE_MS = 120;

export const RECENT_FILES_MAX = 8;

export function isTauriRuntime(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

/** Whether the app is running in a mobile (Android/iOS) web view. Tauri mobile
 * web views expose a mobile platform user-agent; the desktop webview does not
 * match these tokens. Used to hide desktop-only chrome (e.g. window controls)
 * on platforms that have no window concept. */
export function isMobile(): boolean {
  if (typeof navigator === "undefined") return false;
  return /Mobi|Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
}

/** Default entry-table columns. `width: 0` on "title" is an auto sentinel
 *  (the frontend renders the default column width). Mirrors
 *  `default_entry_columns` in config.rs. */
export const DEFAULT_ENTRY_COLUMNS: EntryColumnState[] = [
  { id: "title", visible: true, width: 0 },
  { id: "username", visible: true, width: 120 },
  { id: "password", visible: true, width: 100 },
  { id: "url", visible: true, width: 180 },
  { id: "totp", visible: true, width: 96 },
  { id: "notes", visible: false, width: 160 },
  { id: "tags", visible: false, width: 120 },
  { id: "created", visible: false, width: 140 },
  { id: "modified", visible: false, width: 140 },
  { id: "expires", visible: false, width: 140 },
];

/** Merge a persisted column list over the defaults: unknown ids (custom-field
 *  columns) survive, widths clamp to 30..=400 (title keeps its 0 auto
 *  sentinel). The persisted array order is the display order and is preserved;
 *  fallback columns missing from the source are appended at the end so newer
 *  defaults still appear. Mirrors `normalize_entry_columns` in config.rs. */
export function normalizeEntryColumns(
  source: EntryColumnState[] | undefined,
  fallback: EntryColumnState[] = DEFAULT_ENTRY_COLUMNS,
): EntryColumnState[] {
  if (!Array.isArray(source) || source.length === 0) {
    return fallback.map((c) => ({ ...c }));
  }
  const byId = new Map<string, EntryColumnState>();
  for (const col of source) {
    if (typeof col !== "object" || col === null) continue;
    const width =
      col.id === "title" && col.width === 0
        ? 0
        : clampInt(typeof col.width === "number" ? col.width : 120, 30, 400, 120);
    byId.set(String(col.id), {
      id: String(col.id),
      visible: typeof col.visible === "boolean" ? col.visible : true,
      width,
    });
  }
  for (const col of fallback) {
    if (!byId.has(col.id)) byId.set(col.id, { ...col });
  }
  return Array.from(byId.values());
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
  iconOnlyButtons: false,
  toolbarOverflowMenu: isMobile(),
  entryColumns: DEFAULT_ENTRY_COLUMNS,
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
  fileExtension: "kdbx",
};

export const DEFAULT_S3_REMOTE_SETTINGS: RemoteS3Settings = {
  kind: "s3",
  endpoint: "https://s3.amazonaws.com",
  region: "us-east-1",
  bucket: "",
  accessKey: "",
  secretKey: "",
  prefix: "",
  backupCount: 3,
  backupTemplate: "{name}.{timestamp}.{ext}.bak",
};

export const DEFAULT_WEBDAV_REMOTE_SETTINGS: RemoteWebDavSettings = {
  kind: "webdav",
  endpoint: "",
  accessKey: "",
  secretKey: "",
  prefix: "",
  backupCount: 3,
  backupTemplate: "{name}.{timestamp}.{ext}.bak",
};

export const DEFAULT_REMOTE_PROFILES: RemoteProfile[] = [
  { name: "config_1", settings: DEFAULT_S3_REMOTE_SETTINGS },
  { name: "config_1", settings: DEFAULT_WEBDAV_REMOTE_SETTINGS },
];

export const DEFAULT_BRIDGE_SETTINGS: BridgeSettings = {
  enabled: false,
};

export const DEFAULT_RPC_SETTINGS: RpcSettings = {
  enabled: false,
  keepSessionAfterLock: true,
  matchByRegistrableDomain: false,
};

/** App-window shortcuts start unbound; the panel shows the action's `default`
 *  accelerator until the user records a binding. */
export const DEFAULT_KEYBOARD_SETTINGS: KeyboardSettings = {
  autoTypeGlobal: "",
  shortcuts: {},
};

export const DEFAULT_FAVICON_SETTINGS: FaviconSettings = {
  concurrency: 8,
};

export const DEFAULT_APP_SETTINGS: AppSettings = {
  general: DEFAULT_GENERAL_SETTINGS,
  security: DEFAULT_SECURITY_SETTINGS,
  database: DEFAULT_DATABASE_SETTINGS,
  remoteProfiles: DEFAULT_REMOTE_PROFILES,
  activeRemote: "s3/config_1",
  bridge: DEFAULT_BRIDGE_SETTINGS,
  rpc: DEFAULT_RPC_SETTINGS,
  keyboard: DEFAULT_KEYBOARD_SETTINGS,
  favicon: DEFAULT_FAVICON_SETTINGS,
};

const hexColor = /^#[0-9a-fA-F]{6}$|^#[0-9a-fA-F]{8}$/;

function clampInt(value: number, min: number, max: number, fallback: number): number {
  if (typeof value !== "number" || Number.isNaN(value)) return fallback;
  return Math.min(max, Math.max(min, Math.round(value)));
}

function validHex(value: string, fallback: string): string {
  // An empty string is kept as-is so cleared inputs stay clear while the
  // user types a replacement (only non-empty invalid values fall back).
  return typeof value === "string" && (value === "" || hexColor.test(value)) ? value : fallback;
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
  kind: RemoteKind,
): RemoteSettings {
  const r = (source ?? {}) as Record<string, unknown>;
  const fallback = kind === "webdav" ? DEFAULT_WEBDAV_REMOTE_SETTINGS : DEFAULT_S3_REMOTE_SETTINGS;
  const str = (key: string, fb: string): string => {
    const value = r[key];
    return typeof value === "string" ? value.trim() : fb;
  };
  const common = {
    prefix: str("prefix", fallback.prefix),
    backupCount: clampInt(
      typeof r.backupCount === "number" ? r.backupCount : fallback.backupCount,
      0,
      10,
      3,
    ),
    backupTemplate:
      typeof r.backupTemplate === "string" && r.backupTemplate.trim() !== ""
        ? r.backupTemplate.trim()
        : fallback.backupTemplate,
  };
  if (kind === "webdav") {
    return {
      kind,
      endpoint: str("endpoint", DEFAULT_WEBDAV_REMOTE_SETTINGS.endpoint),
      accessKey: str("accessKey", DEFAULT_WEBDAV_REMOTE_SETTINGS.accessKey),
      secretKey: str("secretKey", DEFAULT_WEBDAV_REMOTE_SETTINGS.secretKey),
      ...common,
    };
  }
  return {
    kind,
    endpoint: str("endpoint", DEFAULT_S3_REMOTE_SETTINGS.endpoint),
    region: str("region", DEFAULT_S3_REMOTE_SETTINGS.region),
    bucket: str("bucket", DEFAULT_S3_REMOTE_SETTINGS.bucket),
    accessKey: str("accessKey", DEFAULT_S3_REMOTE_SETTINGS.accessKey),
    secretKey: str("secretKey", DEFAULT_S3_REMOTE_SETTINGS.secretKey),
    ...common,
  };
}

/** Sanitize a user-supplied file extension: drop the leading dot, keep only
 *  alphanumeric characters, fall back to `kdbx` when nothing remains. */
export function normalizeFileExtension(ext: string): string {
  const cleaned = ext
    .trim()
    .replace(/^\.+/, "")
    .replace(/[^a-zA-Z0-9]/g, "");
  return cleaned === "" ? "kdbx" : cleaned;
}

/** Sanitize a remote profile name into a safe path segment, mirroring
 *  `sanitize_dir_name` in `src-tauri/src/remote/local.rs`: keeps letters/digits
 *  (Unicode-aware, so Chinese names survive) plus `-`/`_`; anything else
 *  becomes `_`. Empty/whitespace → `remote`. */
export function sanitizeDirName(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) return "remote";
  let out = "";
  for (const ch of trimmed) {
    out += /\p{L}|\p{N}|[-_]/u.test(ch) ? ch : "_";
  }
  return out === "" ? "remote" : out;
}

function normalizeRemoteProfileName(name: unknown, fallback: string): string {
  const trimmed = String(name ?? "").trim();
  return trimmed === "" ? fallback : sanitizeDirName(trimmed);
}

export function remoteProfilePath(profile: RemoteProfile): RemoteProfilePath {
  return `${profile.settings.kind}/${profile.name}`;
}

export function remoteProfilesForKind(
  profiles: RemoteProfile[],
  kind: RemoteKind,
): RemoteProfile[] {
  return profiles.filter((profile) => profile.settings.kind === kind);
}

export function findRemoteProfile(
  profiles: RemoteProfile[],
  path: string,
): RemoteProfile | undefined {
  return profiles.find((profile) => remoteProfilePath(profile) === path);
}

export function activeRemoteProfile(settings: AppSettings): RemoteProfile {
  return (
    findRemoteProfile(settings.remoteProfiles, settings.activeRemote) ?? settings.remoteProfiles[0]
  );
}

export function remoteMirrorPath(profile: RemoteProfile): RemoteProfilePath {
  return `${profile.settings.kind}/${sanitizeDirName(profile.name)}`;
}

/** Normalize remote profiles into two transport namespaces. Every namespace
 * always retains at least one configuration, and names are unique only within
 * that namespace (`s3/config_1` and `webdav/config_1` may coexist). */
export function normalizeRemoteProfiles(
  source: Partial<RemoteProfile>[] | undefined,
): RemoteProfile[] {
  const counters: Record<RemoteKind, number> = { s3: 0, webdav: 0 };
  const profiles: RemoteProfile[] = [];
  for (const profile of Array.isArray(source) ? source : []) {
    const kind: RemoteKind = profile?.settings?.kind === "webdav" ? "webdav" : "s3";
    counters[kind] += 1;
    profiles.push({
      name: normalizeRemoteProfileName(profile?.name, `config_${counters[kind]}`),
      settings: normalizeRemoteSettings(profile?.settings, kind),
    });
  }
  for (const kind of ["s3", "webdav"] as const) {
    if (counters[kind] === 0) {
      profiles.push({
        name: "config_1",
        settings: normalizeRemoteSettings(undefined, kind),
      });
    }
  }

  const seen: Record<RemoteKind, Set<string>> = { s3: new Set(), webdav: new Set() };
  for (const profile of profiles) {
    const base = profile.name;
    let candidate = base;
    let n = 2;
    const kindSeen = seen[profile.settings.kind];
    while (kindSeen.has(candidate)) {
      candidate = `${base}_${n}`;
      n += 1;
    }
    kindSeen.add(candidate);
    profile.name = candidate;
  }
  return profiles;
}

function normalizeActiveRemote(
  value: unknown,
  profiles: RemoteProfile[],
  fallback: RemoteProfilePath,
): RemoteProfilePath {
  const normalizePath = (path: unknown): RemoteProfilePath | null => {
    if (typeof path !== "string") return null;
    const slash = path.indexOf("/");
    if (slash <= 0) return null;
    const kind = path.slice(0, slash) as RemoteKind;
    if (kind !== "s3" && kind !== "webdav") return null;
    const name = normalizeRemoteProfileName(path.slice(slash + 1), "");
    return name ? `${kind}/${name}` : null;
  };
  const requested = normalizePath(value);
  if (requested && findRemoteProfile(profiles, requested)) return requested;
  const fallbackPath = normalizePath(fallback);
  if (fallbackPath && findRemoteProfile(profiles, fallbackPath)) return fallbackPath;
  return remoteProfilePath(
    profiles.find((profile) => profile.settings.kind === "s3") ?? profiles[0],
  );
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
    iconOnlyButtons:
      typeof g.iconOnlyButtons === "boolean" ? g.iconOnlyButtons : fallback.general.iconOnlyButtons,
    toolbarOverflowMenu:
      typeof g.toolbarOverflowMenu === "boolean"
        ? g.toolbarOverflowMenu
        : fallback.general.toolbarOverflowMenu,
    entryColumns: normalizeEntryColumns(g.entryColumns, fallback.general.entryColumns),
    recentFiles: normalizeRecentFiles(g.recentFiles),
    language:
      g.language === "en" || g.language === "zh-CN" ? g.language : fallback.general.language,
    windowEffect:
      g.windowEffect === "acrylic" || g.windowEffect === "mica" ? g.windowEffect : "off",
  };

  const s = source.security ?? fallback.security;
  const asBool = (v: unknown, fallback: boolean): boolean =>
    typeof v === "boolean" ? v : fallback;
  const security: SecuritySettings = {
    minimizeToTray: asBool(s.minimizeToTray, fallback.security.minimizeToTray),
    clearOnLock: asBool(s.clearOnLock, fallback.security.clearOnLock),
    lockAfterAction: asBool(s.lockAfterAction, fallback.security.lockAfterAction),
    lockOnFocusLoss: asBool(s.lockOnFocusLoss, fallback.security.lockOnFocusLoss),
    rememberPassword: asBool(s.rememberPassword, fallback.security.rememberPassword),
    screenCaptureGuard: asBool(s.screenCaptureGuard, fallback.security.screenCaptureGuard),
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
    fileExtension: normalizeFileExtension(d.fileExtension ?? fallback.database.fileExtension),
  };

  const remoteProfiles = normalizeRemoteProfiles(source.remoteProfiles ?? fallback.remoteProfiles);
  const activeRemote = normalizeActiveRemote(
    source.activeRemote,
    remoteProfiles,
    fallback.activeRemote,
  );

  const k = source.keyboard ?? fallback.keyboard;
  const legacyGlobal = (
    source.general as unknown as {
      globalAutoTypeShortcut?: unknown;
    }
  )?.globalAutoTypeShortcut;
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
      keepSessionAfterLock:
        typeof source.rpc?.keepSessionAfterLock === "boolean"
          ? source.rpc.keepSessionAfterLock
          : (fallback.rpc?.keepSessionAfterLock ?? true),
      matchByRegistrableDomain:
        typeof source.rpc?.matchByRegistrableDomain === "boolean"
          ? source.rpc.matchByRegistrableDomain
          : (fallback.rpc?.matchByRegistrableDomain ?? false),
    },
    keyboard,
    favicon: {
      concurrency: clampInt(source.favicon?.concurrency ?? fallback.favicon?.concurrency, 1, 16, 8),
    },
  };
}

interface AppSettingsStore {
  subscribe: typeof settings.subscribe;
  initialize: () => Promise<void>;
  updateGeneral: <K extends keyof GeneralSettings>(key: K, value: GeneralSettings[K]) => void;
  updateSecurity: <K extends keyof SecuritySettings>(key: K, value: SecuritySettings[K]) => void;
  updateDatabase: <K extends keyof DatabaseDefaults>(key: K, value: DatabaseDefaults[K]) => void;
  updateRemote: <K extends RemoteUpdateKey>(
    path: RemoteProfilePath,
    key: K,
    value: RemoteUpdateValue<K>,
  ) => void;
  setActiveRemote: (path: RemoteProfilePath) => void;
  addRemoteProfile: (kind: RemoteKind, name: string) => void;
  removeRemoteProfile: (path: RemoteProfilePath) => void;
  renameRemoteProfile: (path: RemoteProfilePath, name: string) => void;
  updateBridge: <K extends keyof BridgeSettings>(key: K, value: BridgeSettings[K]) => void;
  updateRpc: <K extends keyof RpcSettings>(key: K, value: RpcSettings[K]) => void;
  updateKeyboard: <K extends keyof KeyboardSettings>(key: K, value: KeyboardSettings[K]) => void;
  updateFavicon: <K extends keyof FaviconSettings>(key: K, value: FaviconSettings[K]) => void;
  merge: (partial: Partial<AppSettings>) => void;
  flush: () => Promise<void>;
  destroy: () => void;
}

/** Editable fields across the two discriminated remote profile shapes. */
export type RemoteUpdateKey =
  | (keyof Omit<RemoteS3Settings, "kind"> & string)
  | (keyof Omit<RemoteWebDavSettings, "kind"> & string);

export type RemoteUpdateValue<K extends RemoteUpdateKey> = K extends "backupCount"
  ? number
  : string;

function updateRemoteBlock(
  settings: RemoteSettings,
  key: RemoteUpdateKey,
  value: unknown,
): RemoteSettings {
  if (settings.kind === "webdav" && (key === "region" || key === "bucket")) return settings;
  return { ...settings, [key]: value } as RemoteSettings;
}

const settings = writable<AppSettings>(DEFAULT_APP_SETTINGS);

const STORAGE_KEY = "secpivot-settings";

let dirty = false;
let pending: AppSettings | null = null;
/** In-flight single-flight persist chain; `null` when idle. Used to serialize
 * writes and to let `flush()` await the chain that keeps draining newer
 * changes (fixes a lost-update race where a change arriving mid-write was
 * never persisted). */
let persistPromise: Promise<void> | null = null;
let persistTimer: ReturnType<typeof setTimeout> | null = null;
let initialized = false;

/** Strip remote credentials from a settings clone so the browser-preview
 *  persistence never writes `accessKey`/`secretKey` to localStorage (mirrors
 *  `withoutSecrets` in vault.ts for the vault demo). */
function withoutSecrets(value: AppSettings): AppSettings {
  const strip = (settings: RemoteSettings): RemoteSettings =>
    ({
      ...settings,
      accessKey: "",
      secretKey: "",
    }) as RemoteSettings;
  return {
    ...value,
    remoteProfiles: value.remoteProfiles.map((p) => ({ ...p, settings: strip(p.settings) })),
  };
}

/** Write every queued value in order. The `while` loop re-checks `pending`
 * after each write, so changes that land while an earlier write is in flight
 * are drained by the same chain instead of being dropped. */
async function persist(): Promise<void> {
  while (dirty && pending) {
    const value = pending;
    pending = null;
    dirty = false;
    if (isTauriRuntime()) {
      try {
        const saved = await invoke<AppSettings>("set_config", { config: value });
        settings.set(normalizeSettings(saved, value));
      } catch {
        // Re-queue the failed value for a retry only when no newer change has
        // superseded it (a mid-flight edit already replaced `pending`).
        if (pending === null) {
          pending = value;
          dirty = true;
        }
        return;
      }
    } else {
      try {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(withoutSecrets(value)));
      } catch {
        // storage unavailable; keep running without persistence
      }
    }
  }
}

/** Single-flight runner: concurrent callers share one chain, and the chain
 * keeps draining `pending` until it is empty, so the debounce timer can never
 * abandon a queued value. */
async function runPersist(): Promise<void> {
  if (persistPromise) return persistPromise;
  persistPromise = persist().finally(() => {
    persistPromise = null;
  });
  return persistPromise;
}

function schedulePersist(): void {
  dirty = true;
  pending = normalizeSettings(get(settings));
  if (persistTimer) clearTimeout(persistTimer);
  persistTimer = setTimeout(() => {
    persistTimer = null;
    void runPersist();
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

  updateRemote(path, key, value): void {
    settings.update((s) => {
      const remoteProfiles = s.remoteProfiles.map((profile) =>
        remoteProfilePath(profile) === path
          ? { ...profile, settings: updateRemoteBlock(profile.settings, key, value) }
          : profile,
      );
      return { ...s, remoteProfiles };
    });
    schedulePersist();
  },

  setActiveRemote(path): void {
    settings.update((s) => {
      if (!findRemoteProfile(s.remoteProfiles, path)) return s;
      return { ...s, activeRemote: path };
    });
    schedulePersist();
  },

  addRemoteProfile(kind, name): void {
    settings.update((s) => {
      const sameKind = remoteProfilesForKind(s.remoteProfiles, kind);
      const base = normalizeRemoteProfileName(name, `config_${sameKind.length + 1}`);
      const taken = new Set(sameKind.map((profile) => profile.name));
      let candidate = base;
      let n = 2;
      while (taken.has(candidate)) {
        candidate = `${base}_${n}`;
        n += 1;
      }
      const settingsForProfile = normalizeRemoteSettings(undefined, kind);
      const profile: RemoteProfile = { name: candidate, settings: settingsForProfile };
      const remoteProfiles = [...s.remoteProfiles, profile];
      return { ...s, remoteProfiles, activeRemote: remoteProfilePath(profile) };
    });
    schedulePersist();
  },

  removeRemoteProfile(path): void {
    settings.update((s) => {
      const current = findRemoteProfile(s.remoteProfiles, path);
      if (!current) return s;
      const sameKind = remoteProfilesForKind(s.remoteProfiles, current.settings.kind);
      if (sameKind.length <= 1) return s;
      const remoteProfiles = s.remoteProfiles.filter(
        (profile) => remoteProfilePath(profile) !== path,
      );
      const replacement = remoteProfiles.find(
        (profile) => profile.settings.kind === current.settings.kind,
      );
      const activeRemote =
        s.activeRemote === path && replacement ? remoteProfilePath(replacement) : s.activeRemote;
      return { ...s, remoteProfiles, activeRemote };
    });
    schedulePersist();
  },

  renameRemoteProfile(path, name): void {
    settings.update((s) => {
      const current = findRemoteProfile(s.remoteProfiles, path);
      if (!current) return s;
      const nextName = normalizeRemoteProfileName(name, "");
      if (nextName === "") return s;
      const collides = s.remoteProfiles.some(
        (profile) =>
          remoteProfilePath(profile) !== path &&
          profile.settings.kind === current.settings.kind &&
          profile.name === nextName,
      );
      if (collides) return s;
      const remoteProfiles = s.remoteProfiles.map((profile) =>
        remoteProfilePath(profile) === path ? { ...profile, name: nextName } : profile,
      );
      const renamed = remoteProfiles.find(
        (profile) => profile.settings.kind === current.settings.kind && profile.name === nextName,
      );
      const activeRemote =
        s.activeRemote === path && renamed ? remoteProfilePath(renamed) : s.activeRemote;
      return { ...s, remoteProfiles, activeRemote };
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

  updateFavicon(key, value): void {
    settings.update((s) => ({ ...s, favicon: { ...s.favicon, [key]: value } }));
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
      await runPersist();
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
