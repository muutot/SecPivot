import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, appSettings, RECENT_FILES_MAX } from "$lib/services/settings";
import type {
  VaultState,
  VaultGroup,
  VaultEntry,
  EntryInput,
  GroupInput,
  CreateVaultRequest,
  TotpCode,
  HistoryVersion,
  SecurityReport,
  RemoteObject,
  RemoteMode,
} from "$lib/types/vault";
import { ROOT_GROUP_NAME } from "$lib/types/vault";
import { buildDemoVaultState } from "$lib/data/demo-vault";
import { computeTotp } from "$lib/utils/totp";
import { computeSecurityReport } from "$lib/utils/security-report";

interface VaultStore {
  subscribe: typeof state.subscribe;
  get: () => VaultState | null;
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
  save: () => Promise<VaultState>;
  changeMasterKey: (password: string, keyfile: string | null) => Promise<VaultState>;
  addEntry: (input: EntryInput) => Promise<VaultState>;
  updateEntry: (uuid: string, input: EntryInput) => Promise<VaultState>;
  deleteEntry: (uuid: string) => Promise<VaultState>;
  deleteEntries: (uuids: string[]) => Promise<VaultState>;
  moveEntry: (uuid: string, groupUuid: string) => Promise<VaultState>;
  restoreEntry: (uuid: string) => Promise<VaultState>;
  getEntryHistory: (uuid: string) => Promise<HistoryVersion[]>;
  restoreEntryVersion: (uuid: string, index: number) => Promise<VaultState>;
  totpCode: (uuid: string) => Promise<TotpCode>;
  getEntryPassword: (uuid: string) => Promise<string>;
  getEntryTotp: (uuid: string) => Promise<string | null>;
  securityReport: () => Promise<SecurityReport>;
  toggleFavorite: (uuid: string) => Promise<VaultState>;
  autoType: (uuid: string, sequence: string) => Promise<void>;
  saveAttachment: (uuid: string, name: string, dest: string) => Promise<void>;
  addGroup: (input: GroupInput) => Promise<VaultState>;
  renameGroup: (uuid: string, name: string) => Promise<VaultState>;
  deleteGroup: (uuid: string) => Promise<VaultState>;
  restoreGroup: (uuid: string) => Promise<VaultState>;
  emptyRecycleBin: () => Promise<VaultState>;
  refresh: () => Promise<void>;
  remembered: typeof remembered.subscribe;
  getRemembered: () => RememberedVault | null;
  clearRemembered: () => void;
}

export interface RememberedVault {
  path: string;
  fileName: string;
}

const state = writable<VaultState | null>(null);

/** Last opened/created vault path, kept across lock so the lock screen can offer a quick reopen. */
const remembered = writable<RememberedVault | null>(null);

const BROWSER_KEY = "keyvault-browser-vault";

let browserState: VaultState | null = null;
let initialized = false;

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

function findGroup(root: VaultGroup, uuid: string): VaultGroup | null {
  if (root.uuid === uuid) return root;
  for (const child of root.children) {
    const found = findGroup(child, uuid);
    if (found) return found;
  }
  return null;
}

function findEntry(root: VaultGroup, uuid: string): VaultEntry | null {
  for (const entry of root.entries) {
    if (entry.uuid === uuid) return entry;
  }
  for (const child of root.children) {
    const found = findEntry(child, uuid);
    if (found) return found;
  }
  return null;
}

function collectGroups(root: VaultGroup, out: VaultGroup[]): void {
  out.push(root);
  for (const child of root.children) collectGroups(child, out);
}

function findBinGroup(root: VaultGroup): VaultGroup | null {
  for (const child of root.children) {
    if (child.isRecycleBin) return child;
  }
  return null;
}

function ensureBinGroup(root: VaultGroup): VaultGroup {
  const existing = findBinGroup(root);
  if (existing) return existing;
  const bin: VaultGroup = {
    uuid: newUuid(),
    parentUuid: root.uuid,
    name: "回收站",
    isRecycleBin: true,
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
    if (raw) return JSON.parse(raw) as VaultState;
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
    const value = await backendInvoke<VaultState | null>("get_vault_state");
    state.set(value);
    return value;
  }
  const value = browserState ? deepClone(browserState) : null;
  state.set(value);
  return value;
}

async function ensureBrowserLoaded(): Promise<VaultState> {
  if (browserState) return browserState;
  browserState = (await browserLoad()) ?? buildDemoVaultState();
  return browserState;
}

export const vault: VaultStore = {
  subscribe: state.subscribe,

  get(): VaultState | null {
    return get(state);
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
      const result = await backendInvoke<VaultState>("open_vault", {
        path,
        password,
        keyfile: keyfile || null,
      });
      state.set(result);
      remembered.set({ path: result.path, fileName: result.fileName });
      rememberRecent(result.path);
      return result;
    }
    browserState = (await browserLoad()) ?? buildDemoVaultState();
    browserState.path = path;
    browserState.fileName = path.split(/[\\/]/).pop() ?? "vault.kdbx";
    const result = deepClone(browserState);
    state.set(result);
    remembered.set({ path: result.path, fileName: result.fileName });
    rememberRecent(result.path);
    return result;
  },

  async create(request: CreateVaultRequest): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("create_vault", {
        ...request,
        keyfile: request.keyfile || null,
      });
      state.set(result);
      remembered.set({ path: result.path, fileName: result.fileName });
      rememberRecent(result.path);
      return result;
    }
    const fresh = buildDemoVaultState();
    fresh.path = request.path;
    fresh.fileName = request.path.split(/[\\/]/).pop() ?? "vault.kdbx";
    fresh.dirty = false;
    browserState = fresh;
    const result = deepClone(fresh);
    state.set(result);
    remembered.set({ path: result.path, fileName: result.fileName });
    rememberRecent(result.path);
    await browserPersist(result);
    return result;
  },

  async close(): Promise<void> {
    if (isTauriRuntime()) {
      const path = get(state)?.path;
      await backendInvoke("close_vault");
      if (path && !get(appSettings).security.rememberPassword) {
        void backendInvoke("clear_saved_credential", { path }).catch(() => undefined);
      }
    }
    browserState = null;
    state.set(null);
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
    const result = await backendInvoke<VaultState>("open_remote_vault", {
      profile: get(appSettings).activeRemote,
      key,
      password,
      keyfile: keyfile || null,
      mode,
    });
    state.set(result);
    rememberRecent(result.path);
    return result;
  },

  async createRemote(key, password, kdf, cipher, compression, keyfile, mode): Promise<VaultState> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持远程库");
    await appSettings.flush();
    const result = await backendInvoke<VaultState>("create_remote_vault", {
      profile: get(appSettings).activeRemote,
      key,
      password,
      kdf,
      cipher,
      compression,
      keyfile: keyfile || null,
      mode,
    });
    state.set(result);
    rememberRecent(result.path);
    return result;
  },

  async save(): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("save_vault");
      state.set(result);
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

  async changeMasterKey(password: string, keyfile: string | null): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("change_master_key", {
        password,
        keyfile,
      });
      state.set(result);
      return result;
    }
    return vault.save();
  },

  async addEntry(input: EntryInput): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("add_entry", { input });
      state.set(result);
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
        hasTotp: Boolean(input.totp),
        totp: input.totp || undefined,
        customFields: input.customFields,
        attachments: input.attachments?.map((a) => ({ name: a.name, size: a.data?.length ?? 0 })),
        created: new Date().toISOString(),
        modified: new Date().toISOString(),
      });
    });
    state.set(result);
    return result;
  },

  async updateEntry(uuid: string, input: EntryInput): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("update_entry", { uuid, input });
      state.set(result);
      return result;
    }
    const result = applyEdit((draft) => {
      const groups: VaultGroup[] = [];
      collectGroups(draft.root, groups);
      for (const group of groups) {
        const entry = group.entries.find((e) => e.uuid === uuid);
        if (entry) {
          Object.assign(entry, input, {
            groupUuid: input.groupUuid,
            hasTotp: Boolean(input.totp),
            modified: new Date().toISOString(),
          });
          return;
        }
      }
      throw new Error("entry not found");
    });
    state.set(result);
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

  async securityReport(): Promise<SecurityReport> {
    if (isTauriRuntime()) {
      return backendInvoke<SecurityReport>("security_report");
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    return computeSecurityReport(current.root);
  },

  async toggleFavorite(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("toggle_favorite", { uuid });
      state.set(result);
      return result;
    }
    const result = applyEdit((draft) => {
      const entry = findEntry(draft.root, uuid);
      if (!entry) throw new Error("entry not found");
      entry.favorite = !entry.favorite;
    });
    state.set(result);
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
      state.set(result);
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
        children: [],
        entries: [],
      });
    });
    state.set(result);
    return result;
  },

  async renameGroup(uuid: string, name: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("rename_group", { uuid, name });
      state.set(result);
      return result;
    }
    const result = applyEdit((draft) => {
      const group = findGroup(draft.root, uuid);
      if (!group) throw new Error("group not found");
      group.name = name;
    });
    state.set(result);
    return result;
  },

  async deleteEntry(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("delete_entry", { uuid });
      state.set(result);
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
    state.set(result);
    return result;
  },

  async deleteEntries(uuids: string[]): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("delete_entries", { uuids });
      state.set(result);
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
    state.set(result);
    return result;
  },

  async moveEntry(uuid: string, groupUuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("move_entry", { uuid, groupUuid });
      state.set(result);
      return result;
    }
    const result = applyEdit((draft) => {
      const entry = findEntry(draft.root, uuid);
      if (!entry) throw new Error("entry not found");
      removeEntryFromGroup(draft.root, uuid);
      entry.groupUuid = groupUuid;
      const target = findGroup(draft.root, groupUuid);
      if (target) target.entries.push(entry);
    });
    state.set(result);
    return result;
  },

  async restoreEntry(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("restore_entry", { uuid });
      state.set(result);
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
    state.set(result);
    return result;
  },

  async getEntryHistory(uuid: string): Promise<HistoryVersion[]> {
    if (isTauriRuntime()) {
      return backendInvoke<HistoryVersion[]>("get_entry_history", { uuid });
    }
    return [];
  },

  async restoreEntryVersion(uuid: string, index: number): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("restore_entry_version", { uuid, index });
      state.set(result);
      return result;
    }
    throw new Error("浏览器模式不支持历史版本恢复");
  },

  async deleteGroup(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("delete_group", { uuid });
      state.set(result);
      return result;
    }
    if (uuid === "root") throw new Error("cannot delete root");
    const result = applyEdit((draft) => {
      const bin = ensureBinGroup(draft.root);
      const groups: VaultGroup[] = [];
      collectGroups(draft.root, groups);
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
    state.set(result);
    return result;
  },

  async restoreGroup(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("restore_group", { uuid });
      state.set(result);
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
    state.set(result);
    return result;
  },

  async emptyRecycleBin(): Promise<VaultState> {
    if (isTauriRuntime()) {
      const result = await backendInvoke<VaultState>("empty_recycle_bin");
      state.set(result);
      return result;
    }
    const result = applyEdit((draft) => {
      const bin = findBinGroup(draft.root);
      if (bin) {
        bin.entries = [];
        bin.children = [];
      }
    });
    state.set(result);
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
  },
};

export function countEntries(root: VaultGroup): number {
  let total = root.entries.length;
  for (const child of root.children) total += countEntries(child);
  return total;
}

export { ROOT_GROUP_NAME };
