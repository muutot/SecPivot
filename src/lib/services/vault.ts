import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, appSettings, RECENT_FILES_MAX } from "$lib/services/settings";
import type {
  VaultState,
  VaultGroup,
  VaultEntry,
  EntryInput,
  EntryPatch,
  EntryFlags,
  EntryAutoTypeConfig,
  GroupInput,
  GroupAutoTypeConfig,
  GroupMeta,
  CreateVaultRequest,
  TotpCode,
  HistoryVersion,
  EntryStorage,
  SecurityReport,
  FaviconReport,
  MutationDelta,
  DatabaseSettings,
  DatabaseSettingsPatch,
  VaultOpenResult,
  SessionInfo,
  RemoteObject,
  RemoteMode,
} from "$lib/types/vault";
import { ROOT_GROUP_NAME } from "$lib/types/vault";
import { buildDemoVaultState } from "$lib/data/demo-vault";
import { computeTotp } from "$lib/utils/totp";
import { computeSecurityReport } from "$lib/utils/security-report";
import {
  collectGroups as collectAllGroups,
  findBinGroup,
  findEntry,
  findGroup,
  setGroupsExpandedInTree,
} from "$lib/utils/tree";

interface VaultStore {
  subscribe: typeof state.subscribe;
  tabs: typeof tabs;
  activeId: typeof activeId;
  get: () => VaultState | null;
  getDatabaseSettings: () => Promise<DatabaseSettings | null>;
  updateDatabaseSettings: (patch: DatabaseSettingsPatch) => Promise<VaultState>;
  open: (path: string, password: string, keyfile?: string) => Promise<VaultState>;
  create: (request: CreateVaultRequest) => Promise<VaultState>;
  listRemoteObjects: () => Promise<RemoteObject[]>;
  openRemote: (
    key: string,
    password: string,
    keyfile: string | undefined,
    mode: RemoteMode,
  ) => Promise<VaultState>;
  createRemote: (
    key: string,
    password: string,
    kdf: string,
    cipher: string,
    compression: string,
    keyfile: string | undefined,
    mode: RemoteMode,
  ) => Promise<VaultState>;
  close: () => Promise<void>;
  /** Close every open session (lock path); `remembered` stays for quick-reopen. */
  closeAll: () => Promise<void>;
  save: () => Promise<VaultState>;
  saveAs: (path: string) => Promise<VaultState>;
  changeMasterKey: (password: string, keyfile: string | null) => Promise<VaultState>;
  addEntry: (input: EntryInput) => Promise<VaultState>;
  addEntries: (inputs: EntryInput[]) => Promise<VaultState>;
  updateEntry: (uuid: string, input: EntryInput) => Promise<VaultState>;
  updateEntryFlags: (uuid: string, flags: EntryFlags) => Promise<VaultState>;
  updateEntries: (uuids: string[], patch: EntryPatch) => Promise<VaultState>;
  deleteEntry: (uuid: string) => Promise<VaultState>;
  deleteEntries: (uuids: string[]) => Promise<VaultState>;
  moveEntry: (uuid: string, groupUuid: string) => Promise<VaultState>;
  restoreEntry: (uuid: string) => Promise<VaultState>;
  getEntryHistory: (uuid: string) => Promise<HistoryVersion[]>;
  deleteEntryHistory: (uuid: string, index: number) => Promise<VaultState>;
  getEntryStorage: (uuid: string) => Promise<EntryStorage>;
  restoreEntryVersion: (uuid: string, index: number) => Promise<VaultState>;
  totpCode: (uuid: string) => Promise<TotpCode>;
  getEntryPassword: (uuid: string) => Promise<string>;
  getEntryTotp: (uuid: string) => Promise<string | null>;
  getCustomFieldValue: (uuid: string, name: string) => Promise<string | null>;
  securityReport: () => Promise<SecurityReport>;
  downloadFavicons: (uuids?: string[]) => Promise<FaviconReport>;
  toggleFavorite: (uuid: string) => Promise<VaultState>;
  autoType: (uuid: string, sequence: string) => Promise<void>;
  saveAttachment: (uuid: string, name: string, dest: string) => Promise<void>;
  addGroup: (input: GroupInput) => Promise<VaultState>;
  renameGroup: (uuid: string, name: string) => Promise<VaultState>;
  setGroupIcon: (uuid: string, icon: number | null) => Promise<VaultState>;
  updateGroupMeta: (uuid: string, meta: GroupMeta) => Promise<VaultState>;
  setGroupExpanded: (uuid: string, expanded: boolean) => Promise<VaultState>;
  setGroupsExpanded: (uuids: string[], expanded: boolean) => Promise<VaultState>;
  updateEntryAutoType: (uuid: string, input: EntryAutoTypeConfig) => Promise<VaultState>;
  updateGroupAutoType: (uuid: string, input: GroupAutoTypeConfig) => Promise<VaultState>;
  updateDbMeta: (name?: string, description?: string) => Promise<VaultState>;
  deleteGroup: (uuid: string) => Promise<VaultState>;
  restoreGroup: (uuid: string) => Promise<VaultState>;
  emptyRecycleBin: () => Promise<VaultState>;
  refresh: () => Promise<void>;
  /** Switch the active backend session (multi-database tabs). */
  setActiveSession: (sessionId: string) => Promise<VaultState>;
  /** Close one tab's backend session (active close promotes the next one). */
  closeTab: (sessionId: string) => Promise<void>;
  remembered: typeof remembered.subscribe;
  getRemembered: () => RememberedVault | null;
  clearRemembered: () => void;
}

export interface RememberedVault {
  path: string;
  fileName: string;
}

const state = writable<VaultState | null>(null);
/** Open sessions for the tab bar (active first, then parked). */
const tabs = writable<SessionInfo[]>([]);
/** Id of the active backend session (kept in sync with `tabs`). */
const activeId = writable<string | null>(null);

/** Last opened/created vault path, kept across lock so the lock screen can offer a quick reopen. */
const remembered = writable<RememberedVault | null>(null);

const BROWSER_KEY = "secpivot-browser-vault";

let browserState: VaultState | null = null;
let initialized = false;
/** Registry id of the active backend session (multi-database tabs). `null`
 *  in the browser demo, which has no backend sessions. */
let activeSessionId: string | null = null;

/** Cache of database custom icons, kept across mutation snapshots that omit
 *  the image payload; replaced whenever an authoritative snapshot arrives. */
let iconCache: Record<string, string> = {};

function applyBackendState(result: VaultState): VaultState {
  if (result.customIcons !== undefined) iconCache = result.customIcons;
  result.customIcons = { ...iconCache };
  return result;
}

/** Apply a lightweight backend mutation delta to the current store state and
 *  return the merged `VaultState` (or null when no session is open). */
function applyBackendDelta(delta: MutationDelta): VaultState | null {
  const current = get(state);
  if (!current) return null;
  const next = deepClone(current);
  next.revision = delta.revision;
  if (delta.kind === "favorite") {
    const entry = findEntry(next.root, delta.uuid);
    if (entry) entry.favorite = delta.favorite;
  } else if (delta.kind === "groupsExpanded") {
    for (const [uuid, expanded] of Object.entries(delta.groups)) {
      const group = findGroup(next.root, uuid);
      if (group) group.isExpanded = expanded;
    }
  }
  const result = applyBackendState(next);
  state.set(result);
  return result;
}

function deepClone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function applyEdit(mutator: (draft: VaultState) => void): VaultState {
  const current = browserState ?? buildDemoVaultState();
  const next = deepClone(current);
  next.dirty = true;
  next.modifiedAt = new Date().toISOString();
  mutator(next);
  browserState = next;
  return deepClone(next);
}

function ensureBinGroup(root: VaultGroup): VaultGroup {
  const existing = findBinGroup(root);
  if (existing) return existing;
  const bin: VaultGroup = {
    uuid: newUuid(),
    parentUuid: root.uuid,
    name: "回收站",
    isRecycleBin: true,
    enableSearching: true,
    isExpanded: true,
    children: [],
    entries: [],
  };
  root.children.push(bin);
  return bin;
}

function removeEntryFromGroup(root: VaultGroup, uuid: string): void {
  const index = root.entries.findIndex((e) => e.uuid === uuid);
  if (index >= 0) {
    root.entries.splice(index, 1);
    return;
  }
  for (const child of root.children) removeEntryFromGroup(child, uuid);
}

function moveEntriesToRoot(draft: VaultState, group: VaultGroup): void {
  for (const entry of group.entries) {
    entry.groupUuid = draft.root.uuid;
    draft.root.entries.push(entry);
  }
  for (const child of group.children) moveEntriesToRoot(draft, child);
}

function pushEntry(group: VaultGroup, entry: VaultEntry): void {
  group.entries.push(entry);
}

async function browserLoad(): Promise<VaultState | null> {
  try {
    const raw = localStorage.getItem(BROWSER_KEY);
    if (raw) {
      const parsed = JSON.parse(raw) as VaultState;
      parsed.revision ??= 0;
      return parsed;
    }
  } catch {
    // ignore corrupted demo persistence
  }
  return null;
}

/** Strip secrets (passwords, TOTP seeds) from a structural clone so the
 * browser-demo persistence never writes them to localStorage. On reload the
 * entries keep their metadata but passwords/tokens are gone. */
function withoutSecrets(value: VaultState): VaultState {
  const clone = deepClone(value);
  const strip = (group: VaultGroup): void => {
    for (const entry of group.entries) {
      delete entry.password;
      delete entry.totp;
    }
    for (const child of group.children) strip(child);
  };
  strip(clone.root);
  return clone;
}

async function browserPersist(value: VaultState): Promise<void> {
  try {
    localStorage.setItem(BROWSER_KEY, JSON.stringify(withoutSecrets(value)));
  } catch {
    // storage unavailable; in-memory only
  }
}

function newUuid(): string {
  return crypto.randomUUID();
}

function removeGroupFromTree(group: VaultGroup, uuid: string): boolean {
  const index = group.children.findIndex((c) => c.uuid === uuid);
  if (index >= 0) {
    group.children.splice(index, 1);
    return true;
  }
  for (const child of group.children) {
    if (removeGroupFromTree(child, uuid)) return true;
  }
  return false;
}

/** Whether `uuid`'s group already lives inside the recycle-bin subtree. */
function groupInBin(root: VaultGroup, uuid: string): boolean {
  const bin = findBinGroup(root);
  if (!bin) return false;
  if (uuid === bin.uuid) return true;
  const byUuid = new Map(collectAllGroups(root).map((g) => [g.uuid, g]));
  const target = byUuid.get(uuid);
  if (!target) return false;
  let cursor = byUuid.get(target.parentUuid ?? "");
  while (cursor) {
    if (cursor.uuid === bin.uuid) return true;
    cursor = byUuid.get(cursor.parentUuid ?? "");
  }
  return false;
}

/** Move a successfully opened/created vault path to the front of the recent list.
 * Remote (`s3://`) paths are excluded — the local open flow cannot reopen them. */
function rememberRecent(path: string): void {
  if (!path || path.startsWith("s3://")) return;
  const current = get(appSettings).general.recentFiles;
  const next = [path, ...current.filter((p) => p !== path)].slice(0, RECENT_FILES_MAX);
  if (next.length === current.length && next.every((p, i) => p === current[i])) return;
  appSettings.updateGeneral("recentFiles", next);
}

async function backendInvoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  return invoke<T>(command, args);
}

async function refreshInternal(): Promise<VaultState | null> {
  if (isTauriRuntime()) {
    const value = await backendInvoke<VaultState | null>("get_vault_state", {
      sessionId: activeSessionId,
    });
    if (value) state.set(applyBackendState(value));
    return value;
  }
  const value = browserState ? deepClone(browserState) : null;
  if (value) state.set(applyBackendState(value));
  return value;
}

/** Refresh the tab-bar session list from the backend. */
async function refreshTabs(): Promise<void> {
  if (isTauriRuntime()) {
    const list = await backendInvoke<SessionInfo[]>("list_sessions");
    tabs.set(list);
    activeId.set(list[0]?.sessionId ?? null);
    return;
  }
  const current = browserState ?? get(state);
  activeId.set(current ? "browser" : null);
  tabs.set(
    current
      ? [
          {
            sessionId: "browser",
            fileName: current.fileName,
            path: current.path,
            dirty: current.dirty,
          },
        ]
      : [],
  );
}

async function ensureBrowserLoaded(): Promise<VaultState> {
  if (browserState) return browserState;
  browserState = (await browserLoad()) ?? buildDemoVaultState();
  return browserState;
}

export const vault: VaultStore = {
  subscribe: state.subscribe,
  tabs,
  activeId,

  get(): VaultState | null {
    return get(state);
  },

  async getDatabaseSettings(): Promise<DatabaseSettings | null> {
    if (!isTauriRuntime()) return null;
    return backendInvoke<DatabaseSettings | null>("get_database_settings");
  },

  async updateDatabaseSettings(patch: DatabaseSettingsPatch): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("update_database_settings", { patch });
      state.set(applyBackendState(result));
      return result;
    }
    throw new Error("浏览器预览不支持数据库设置修改");
  },

  remembered: remembered.subscribe,
  getRemembered(): RememberedVault | null {
    return get(remembered);
  },
  clearRemembered(): void {
    remembered.set(null);
  },

  async open(path, password, keyfile): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultOpenResult>("open_vault", {
        path,
        password,
        keyfile: keyfile || null,
      });
      activeSessionId = result.sessionId;
      state.set(applyBackendState(result.state));
      remembered.set({ path: result.state.path, fileName: result.state.fileName });
      rememberRecent(result.state.path);
      await refreshTabs();
      return result.state;
    }
    browserState = (await browserLoad()) ?? buildDemoVaultState();
    browserState.path = path;
    browserState.fileName = path.split(/[\\/]/).pop() ?? "vault.kdbx";
    const result = deepClone(browserState);
    state.set(applyBackendState(result));
    remembered.set({ path: result.path, fileName: result.fileName });
    rememberRecent(result.path);
    await refreshTabs();
    return result;
  },

  async create(request: CreateVaultRequest): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultOpenResult>("create_vault", {
        ...request,
        keyfile: request.keyfile || null,
      });
      activeSessionId = result.sessionId;
      state.set(applyBackendState(result.state));
      remembered.set({ path: result.state.path, fileName: result.state.fileName });
      rememberRecent(result.state.path);
      await refreshTabs();
      return result.state;
    }
    const fresh = buildDemoVaultState();
    fresh.path = request.path;
    fresh.fileName = request.path.split(/[\\/]/).pop() ?? "vault.kdbx";
    fresh.dirty = false;
    browserState = fresh;
    const result = deepClone(fresh);
    state.set(applyBackendState(result));
    remembered.set({ path: result.path, fileName: result.fileName });
    rememberRecent(result.path);
    await browserPersist(result);
    await refreshTabs();
    return result;
  },

  async close(): Promise<void> {
    if (isTauriRuntime()) {
      const path = get(state)?.path;
      await backendInvoke("close_vault", { sessionId: activeSessionId });
      activeSessionId = null;
      if (path && !get(appSettings).security.rememberPassword) {
        void backendInvoke("clear_saved_credential", { path }).catch(() => undefined);
      }
      const remaining = await refreshInternal();
      if (!remaining) {
        state.set(null);
        iconCache = {};
      } else if (remaining.path.startsWith("s3://")) {
        remembered.set(null);
      } else {
        remembered.set({ path: remaining.path, fileName: remaining.fileName });
      }
      await refreshTabs();
      return;
    }
    browserState = null;
    iconCache = {};
    state.set(null);
    tabs.set([]);
  },

  async closeAll(): Promise<void> {
    if (isTauriRuntime()) {
      await backendInvoke("close_all_vaults");
      activeSessionId = null;
    }
    browserState = null;
    iconCache = {};
    state.set(null);
    tabs.set([]);
    activeId.set(null);
  },

  async listRemoteObjects(): Promise<RemoteObject[]> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持远程库");
    await appSettings.flush();
    return backendInvoke<RemoteObject[]>("s3_list_objects", {
      profile: get(appSettings).activeRemote,
    });
  },

  async openRemote(key, password, keyfile, mode): Promise<VaultState> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持远程库");
    await appSettings.flush();
    const result = await backendInvoke<VaultOpenResult>("open_remote_vault", {
      profile: get(appSettings).activeRemote,
      key,
      password,
      keyfile: keyfile || null,
      mode,
    });
    activeSessionId = result.sessionId;
    state.set(applyBackendState(result.state));
    // A remote session cannot be reopened from the lock screen; clear the
    // remembered local path so unlocking never silently targets the old vault.
    remembered.set(null);
    rememberRecent(result.state.path);
    await refreshTabs();
    return result.state;
  },

  async createRemote(key, password, kdf, cipher, compression, keyfile, mode): Promise<VaultState> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持远程库");
    await appSettings.flush();
    const result = await backendInvoke<VaultOpenResult>("create_remote_vault", {
      profile: get(appSettings).activeRemote,
      key,
      password,
      kdf,
      cipher,
      compression,
      keyfile: keyfile || null,
      mode,
    });
    activeSessionId = result.sessionId;
    state.set(applyBackendState(result.state));
    // See `openRemote`: a remote session is never the lock-screen quick-reopen
    // target, so drop any stale remembered local path.
    remembered.set(null);
    rememberRecent(result.state.path);
    await refreshTabs();
    return result.state;
  },

  async save(): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("save_vault");
      state.set(applyBackendState(result));
      await refreshTabs();
      return result;
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    const saved = deepClone(current);
    saved.dirty = false;
    browserState = saved;
    state.set(saved);
    await browserPersist(saved);
    return saved;
  },

  /** Save As: persist to a new local path and switch the session target. */
  async saveAs(path: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("save_vault_as", { path });
      state.set(applyBackendState(result));
      await refreshTabs();
      return result;
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    const saved = deepClone(current);
    saved.path = path;
    saved.fileName = path.split(/[\\/]/).pop() ?? "vault.kdbx";
    saved.dirty = false;
    browserState = saved;
    state.set(saved);
    remembered.set({ path: saved.path, fileName: saved.fileName });
    rememberRecent(path);
    await browserPersist(saved);
    return saved;
  },

  async changeMasterKey(password: string, keyfile: string | null): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("change_master_key", {
        password,
        keyfile,
      });
      state.set(applyBackendState(result));
      return result;
    }
    return vault.save();
  },

  async addEntry(input: EntryInput): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("add_entry", { input });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const group = findGroup(draft.root, input.groupUuid);
      if (!group) throw new Error("target group not found");
      pushEntry(group, {
        uuid: newUuid(),
        groupUuid: input.groupUuid,
        title: input.title,
        username: input.username,
        password: input.password,
        url: input.url,
        notes: input.notes,
        tags: input.tags,
        hasTotp: Boolean(input.totp),
        totp: input.totp || undefined,
        customFields: input.customFields,
        attachments: input.attachments?.map((a) => ({ name: a.name, size: a.data?.length ?? 0 })),
        created: new Date().toISOString(),
        modified: new Date().toISOString(),
      });
    });
    state.set(applyBackendState(result));
    return result;
  },

  async addEntries(inputs: EntryInput[]): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("import_entries", { inputs });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      for (const input of inputs) {
        const group = findGroup(draft.root, input.groupUuid);
        if (!group) throw new Error("target group not found");
        pushEntry(group, {
          uuid: newUuid(),
          groupUuid: input.groupUuid,
          title: input.title,
          username: input.username,
          password: input.password,
          url: input.url,
          notes: input.notes,
          tags: input.tags,
          hasTotp: Boolean(input.totp),
          totp: input.totp || undefined,
          customFields: input.customFields,
          attachments: input.attachments?.map((a) => ({ name: a.name, size: a.data?.length ?? 0 })),
          created: new Date().toISOString(),
          modified: new Date().toISOString(),
        });
      }
    });
    state.set(applyBackendState(result));
    return result;
  },

  async updateEntry(uuid: string, input: EntryInput): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("update_entry", { uuid, input });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const groups = collectAllGroups(draft.root);
      for (const group of groups) {
        const entry = group.entries.find((e) => e.uuid === uuid);
        if (entry) {
          // `icon` follows the backend contract: absent keeps the current
          // icon (custom favicons survive content-only edits), `null` clears
          // it. `Object.assign` would copy `icon: undefined` over the entry,
          // so the icon key is excluded from the merge and applied explicitly.
          const { icon, ...rest } = input;
          Object.assign(entry, rest, {
            groupUuid: input.groupUuid,
            hasTotp: Boolean(input.totp),
            modified: new Date().toISOString(),
          });
          if (icon === null) entry.icon = undefined;
          else if (icon !== undefined) entry.icon = icon;
          return;
        }
      }
      throw new Error("entry not found");
    });
    state.set(applyBackendState(result));
    return result;
  },

  async updateEntryFlags(uuid: string, flags: EntryFlags): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("update_entry_flags", {
        uuid,
        overrideUrl: flags.overrideUrl ?? null,
        qualityCheck: flags.qualityCheck ?? null,
        foregroundColor: flags.foregroundColor ?? null,
      });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const groups = collectAllGroups(draft.root);
      for (const group of groups) {
        const entry = group.entries.find((e) => e.uuid === uuid);
        if (entry) {
          if (flags.overrideUrl !== undefined) {
            const value = flags.overrideUrl.trim();
            if (value) entry.overrideUrl = value;
            else delete entry.overrideUrl;
          }
          if (flags.qualityCheck !== undefined) entry.qualityCheck = flags.qualityCheck;
          if (flags.foregroundColor !== undefined) {
            const value = flags.foregroundColor.trim();
            if (value) entry.foregroundColor = value;
            else delete entry.foregroundColor;
          }
          return;
        }
      }
      throw new Error("entry not found");
    });
    state.set(applyBackendState(result));
    return result;
  },

  async updateEntries(uuids: string[], patch: EntryPatch): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("update_entries", { uuids, patch });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const groups = collectAllGroups(draft.root);
      for (const group of groups) {
        for (const entry of group.entries) {
          if (!uuids.includes(entry.uuid)) continue;
          if (patch.title !== undefined) entry.title = patch.title;
          if (patch.username !== undefined) entry.username = patch.username;
          if (patch.password !== undefined) entry.password = patch.password;
          if (patch.url !== undefined) entry.url = patch.url;
          if (patch.notes !== undefined) entry.notes = patch.notes;
          if (patch.totp !== undefined) {
            entry.totp = patch.totp || undefined;
            entry.hasTotp = Boolean(patch.totp);
          }
          if (patch.clearExpires) entry.expires = undefined;
          else if (patch.expires !== undefined) entry.expires = patch.expires || undefined;
          if (patch.clearIcon) entry.icon = undefined;
          else if (patch.icon !== undefined) entry.icon = patch.icon;
          if (patch.clearColor) entry.color = undefined;
          else if (patch.color !== undefined) entry.color = patch.color || undefined;
          if (patch.tags !== undefined) entry.tags = patch.tags || undefined;
          entry.modified = new Date().toISOString();
        }
      }
    });
    state.set(applyBackendState(result));
    return result;
  },

  async totpCode(uuid: string): Promise<TotpCode> {
    if (isTauriRuntime()) {
      return backendInvoke<TotpCode>("totp_code", { uuid });
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    const entry = findEntry(current.root, uuid);
    if (!entry) throw new Error("entry not found");
    if (!entry.totp) throw new Error("该条目没有 TOTP 种子");
    return computeTotp(entry.totp);
  },

  async getEntryPassword(uuid: string): Promise<string> {
    if (isTauriRuntime()) {
      return backendInvoke<string>("get_entry_password", { uuid });
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    return findEntry(current.root, uuid)?.password ?? "";
  },

  async getEntryTotp(uuid: string): Promise<string | null> {
    if (isTauriRuntime()) {
      return backendInvoke<string | null>("get_entry_totp", { uuid });
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    return findEntry(current.root, uuid)?.totp ?? null;
  },

  async getCustomFieldValue(uuid: string, name: string): Promise<string | null> {
    if (isTauriRuntime()) {
      return backendInvoke<string | null>("get_custom_field_value", { uuid, name });
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    return findEntry(current.root, uuid)?.customFields?.find((f) => f.name === name)?.value ?? null;
  },

  async securityReport(): Promise<SecurityReport> {
    if (isTauriRuntime()) {
      return backendInvoke<SecurityReport>("security_report");
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    return computeSecurityReport(current.root);
  },

  async downloadFavicons(uuids?: string[]): Promise<FaviconReport> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持下载图标");
    const report = await backendInvoke<FaviconReport>("download_favicons", {
      uuids: uuids && uuids.length > 0 ? uuids : undefined,
    });
    await refreshInternal();
    return report;
  },

  async toggleFavorite(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const delta = await backendInvoke<MutationDelta>("toggle_favorite", { uuid });
      const result = applyBackendDelta(delta);
      if (!result) throw new Error("数据库未打开");
      return result;
    }
    const result = applyEdit((draft) => {
      const entry = findEntry(draft.root, uuid);
      if (!entry) throw new Error("entry not found");
      entry.favorite = !entry.favorite;
    });
    state.set(applyBackendState(result));
    return result;
  },

  async autoType(uuid: string, sequence: string): Promise<void> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持自动填充");
    await backendInvoke<void>("auto_type", { uuid, sequence });
  },

  async saveAttachment(uuid: string, name: string, dest: string): Promise<void> {
    await backendInvoke<void>("save_attachment", { uuid, name, dest });
  },

  async addGroup(input: GroupInput): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("add_group", { input });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const parent = input.parentUuid ? findGroup(draft.root, input.parentUuid) : draft.root;
      if (!parent) throw new Error("parent group not found");
      parent.children.push({
        uuid: newUuid(),
        parentUuid: parent.uuid,
        name: input.name,
        isRecycleBin: false,
        enableSearching: true,
        isExpanded: true,
        children: [],
        entries: [],
      });
    });
    state.set(applyBackendState(result));
    return result;
  },

  async renameGroup(uuid: string, name: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("rename_group", { uuid, name });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const group = findGroup(draft.root, uuid);
      if (!group) throw new Error("group not found");
      group.name = name;
    });
    state.set(applyBackendState(result));
    return result;
  },

  async setGroupIcon(uuid: string, icon: number | null): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("set_group_icon", { uuid, icon });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const group = findGroup(draft.root, uuid);
      if (!group) throw new Error("group not found");
      if (icon === null) group.icon = undefined;
      else group.icon = icon;
    });
    state.set(applyBackendState(result));
    return result;
  },

  async updateGroupMeta(uuid: string, meta: GroupMeta): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("update_group_meta", {
        uuid,
        notes: meta.notes ?? null,
        tags: meta.tags ?? null,
        enableSearching: meta.enableSearching ?? null,
      });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const group = findGroup(draft.root, uuid);
      if (!group) throw new Error("group not found");
      if (meta.notes !== undefined) group.notes = meta.notes.trim() || undefined;
      if (meta.tags !== undefined) group.tags = meta.tags.trim() || undefined;
      if (meta.enableSearching !== undefined) group.enableSearching = meta.enableSearching;
    });
    state.set(applyBackendState(result));
    return result;
  },

  async setGroupExpanded(uuid: string, expanded: boolean): Promise<VaultState> {
    if (isTauriRuntime()) {
      const delta = await backendInvoke<MutationDelta>("set_group_expanded", {
        uuid,
        expanded,
      });
      const result = applyBackendDelta(delta);
      if (!result) throw new Error("数据库未打开");
      return result;
    }
    const result = applyEdit((draft) => {
      const group = findGroup(draft.root, uuid);
      if (!group) throw new Error("group not found");
      group.isExpanded = expanded;
    });
    state.set(applyBackendState(result));
    return result;
  },

  async setGroupsExpanded(uuids: string[], expanded: boolean): Promise<VaultState> {
    if (isTauriRuntime()) {
      const delta = await backendInvoke<MutationDelta>("set_groups_expanded", {
        uuids,
        expanded,
      });
      const result = applyBackendDelta(delta);
      if (!result) throw new Error("数据库未打开");
      return result;
    }
    if (uuids.length === 0) {
      const result = deepClone(browserState ?? buildDemoVaultState());
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      setGroupsExpandedInTree(draft.root, uuids, expanded);
    });
    state.set(applyBackendState(result));
    return result;
  },

  async updateEntryAutoType(uuid: string, input: EntryAutoTypeConfig): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("update_entry_autotype", { uuid, input });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const entry = findEntry(draft.root, uuid);
      if (!entry) throw new Error("entry not found");
      entry.autoType = {
        enabled: input.enabled,
        defaultSequence: input.defaultSequence,
        associations: input.associations,
      };
    });
    state.set(applyBackendState(result));
    return result;
  },

  async updateGroupAutoType(uuid: string, input: GroupAutoTypeConfig): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("update_group_autotype", { uuid, input });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const group = findGroup(draft.root, uuid);
      if (!group) throw new Error("group not found");
      group.autoType = {
        enabled: input.enabled,
        defaultSequence: input.defaultSequence,
      };
    });
    state.set(applyBackendState(result));
    return result;
  },

  async updateDbMeta(name?: string, description?: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const args: Record<string, string> = {};
      if (name !== undefined) args.name = name;
      if (description !== undefined) args.description = description;
      const result = await backendInvoke<VaultState>("update_db_meta", args);
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      if (name !== undefined) draft.databaseName = name ? name : undefined;
      if (description !== undefined)
        draft.databaseDescription = description ? description : undefined;
    });
    state.set(applyBackendState(result));
    return result;
  },

  async deleteEntry(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("delete_entry", { uuid });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const entry = findEntry(draft.root, uuid);
      if (!entry) throw new Error("entry not found");
      removeEntryFromGroup(draft.root, uuid);
      const bin = ensureBinGroup(draft.root);
      entry.groupUuid = bin.uuid;
      bin.entries.push(entry);
    });
    state.set(applyBackendState(result));
    return result;
  },

  async deleteEntries(uuids: string[]): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("delete_entries", { uuids });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const bin = ensureBinGroup(draft.root);
      for (const uuid of uuids) {
        const entry = findEntry(draft.root, uuid);
        if (!entry) continue;
        removeEntryFromGroup(draft.root, uuid);
        entry.groupUuid = bin.uuid;
        bin.entries.push(entry);
      }
    });
    state.set(applyBackendState(result));
    return result;
  },

  async moveEntry(uuid: string, groupUuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("move_entry", { uuid, groupUuid });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const entry = findEntry(draft.root, uuid);
      if (!entry) throw new Error("entry not found");
      // Validate the target group before touching the tree, so an invalid
      // `groupUuid` fails atomically instead of silently dropping the entry.
      const target = findGroup(draft.root, groupUuid);
      if (!target) throw new Error("target group not found");
      removeEntryFromGroup(draft.root, uuid);
      entry.groupUuid = groupUuid;
      target.entries.push(entry);
    });
    state.set(applyBackendState(result));
    return result;
  },

  async restoreEntry(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("restore_entry", { uuid });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const bin = findBinGroup(draft.root);
      if (!bin) throw new Error("recycle bin not found");
      const index = bin.entries.findIndex((e) => e.uuid === uuid);
      if (index < 0) throw new Error("entry not in recycle bin");
      const [entry] = bin.entries.splice(index, 1);
      entry.groupUuid = draft.root.uuid;
      draft.root.entries.push(entry);
    });
    state.set(applyBackendState(result));
    return result;
  },

  async getEntryHistory(uuid: string): Promise<HistoryVersion[]> {
    if (isTauriRuntime()) {
      return backendInvoke<HistoryVersion[]>("get_entry_history", { uuid });
    }
    return [];
  },

  async deleteEntryHistory(uuid: string, index: number): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("delete_entry_history", { uuid, index });
      state.set(applyBackendState(result));
      return result;
    }
    throw new Error("浏览器模式不支持删除历史版本");
  },

  async getEntryStorage(uuid: string): Promise<EntryStorage> {
    if (isTauriRuntime()) {
      return backendInvoke<EntryStorage>("get_entry_storage", { uuid });
    }
    return { fields: 0, attachments: 0, history: 0, total: 0 };
  },

  async restoreEntryVersion(uuid: string, index: number): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("restore_entry_version", { uuid, index });
      state.set(applyBackendState(result));
      return result;
    }
    throw new Error("浏览器模式不支持历史版本恢复");
  },

  async deleteGroup(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("delete_group", { uuid });
      state.set(applyBackendState(result));
      return result;
    }
    if (uuid === "root") throw new Error("cannot delete root");
    const result = applyEdit((draft) => {
      // A group already inside the recycle bin is permanently deleted; any
      // other group moves (with its contents) into the bin.
      if (groupInBin(draft.root, uuid)) {
        if (!removeGroupFromTree(draft.root, uuid)) throw new Error("group not found");
        return;
      }
      const bin = ensureBinGroup(draft.root);
      const groups = collectAllGroups(draft.root);
      for (const group of groups) {
        const index = group.children.findIndex((c) => c.uuid === uuid);
        if (index >= 0) {
          const [removed] = group.children.splice(index, 1);
          removed.parentUuid = bin.uuid;
          bin.children.push(removed);
          return;
        }
      }
      throw new Error("group not found");
    });
    state.set(applyBackendState(result));
    return result;
  },

  async restoreGroup(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("restore_group", { uuid });
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const bin = findBinGroup(draft.root);
      if (!bin) throw new Error("recycle bin not found");
      const index = bin.children.findIndex((c) => c.uuid === uuid);
      if (index < 0) throw new Error("group not in recycle bin");
      const [removed] = bin.children.splice(index, 1);
      removed.parentUuid = draft.root.uuid;
      draft.root.children.push(removed);
    });
    state.set(applyBackendState(result));
    return result;
  },

  async emptyRecycleBin(): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("empty_recycle_bin");
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      const bin = findBinGroup(draft.root);
      if (bin) {
        bin.entries = [];
        bin.children = [];
      }
    });
    state.set(applyBackendState(result));
    return result;
  },

  async refresh(): Promise<void> {
    if (!initialized) {
      initialized = true;
      if (!isTauriRuntime()) {
        browserState = await browserLoad();
      }
    }
    await refreshInternal();
    await refreshTabs();
  },

  async setActiveSession(sessionId: string): Promise<VaultState> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持多库标签");
    const result = await backendInvoke<VaultState>("set_active_session", { sessionId });
    activeSessionId = sessionId;
    state.set(applyBackendState(result));
    // The lock-screen quick-reopen follows the newly active tab; remote
    // sessions never become a quick-reopen target.
    if (result.path.startsWith("s3://")) {
      remembered.set(null);
    } else {
      remembered.set({ path: result.path, fileName: result.fileName });
    }
    await refreshTabs();
    return result;
  },

  async closeTab(sessionId: string): Promise<void> {
    if (sessionId === activeSessionId) {
      await vault.close();
      return;
    }
    if (!isTauriRuntime()) return;
    const tab = get(tabs).find((t) => t.sessionId === sessionId);
    await backendInvoke("close_vault", { sessionId });
    if (tab?.path && !get(appSettings).security.rememberPassword) {
      void backendInvoke("clear_saved_credential", { path: tab.path }).catch(() => undefined);
    }
    await refreshTabs();
  },
};

export { ROOT_GROUP_NAME };
