import { writable, get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime, appSettings, RECENT_FILES_MAX } from "$lib/services/settings";
import type {
  VaultState,
  VaultGroup,
  VaultEntry,
  EntryInput,
  EntryPatch,
  AttachmentInput,
  EntryFlags,
  EntryAutoTypeConfig,
  GroupInput,
  GroupAutoTypeConfig,
  GroupMeta,
  CreateVaultRequest,
  TotpCode,
  HistoryVersion,
  EntryStorage,
  AttachmentPreview,
  TempAttachmentRef,
  SecurityReport,
  SimilarPasswordGroup,
  HistoryCleanResult,
  ExpiredEntry,
  ChangeTimelineEvent,
  BreachFinding,
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
import {
  commitNewestSessionState,
  resolveListedActiveId,
  SessionSwitchQueue,
  switchSession,
} from "$lib/utils/session-state";

interface VaultStore {
  subscribe: typeof state.subscribe;
  tabs: typeof tabs;
  activeId: typeof activeId;
  get: () => VaultState | null;
  getActiveSessionId: () => string | null;
  callInSession: <T>(sessionId: string, operation: () => Promise<T>) => Promise<T>;
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
  save: (force?: boolean) => Promise<VaultState>;
  /** Download the remote vault's latest bytes and replace the session. */
  refreshRemote: () => Promise<VaultState>;
  /** Merge the remote vault's latest bytes into the session by entry/group
   *  UUID + last-modified (histories preserved, recycle bin excluded). */
  mergeRemote: () => Promise<VaultState>;
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
  /** Update one custom field's value in place, keeping its protected flag and
   *  every other field untouched. Used by the detail panel inline editor. */
  updateCustomFieldValue: (
    uuid: string,
    name: string,
    value: string,
    protectedField: boolean,
  ) => Promise<VaultState>;
  securityReport: () => Promise<SecurityReport>;
  similarPasswords: () => Promise<SimilarPasswordGroup[]>;
  clearAllHistory: () => Promise<number>;
  expiredEntries: () => Promise<ExpiredEntry[]>;
  changeTimeline: () => Promise<ChangeTimelineEvent[]>;
  checkHibp: (uuids?: string[]) => Promise<BreachFinding[]>;
  downloadFavicons: (uuids?: string[]) => Promise<FaviconReport>;
  toggleFavorite: (uuid: string) => Promise<VaultState>;
  autoType: (uuid: string, sequence: string) => Promise<void>;
  saveAttachment: (uuid: string, name: string, dest: string) => Promise<void>;
  previewAttachment: (uuid: string, name: string) => Promise<AttachmentPreview>;
  openAttachmentTemp: (uuid: string, name: string) => Promise<TempAttachmentRef>;
  cleanupAttachmentTemp: (token: string) => Promise<void>;
  importAttachmentFromTemp: (uuid: string, name: string, token: string) => Promise<VaultState>;
  /** Add (or replace by name) new attachments to an entry without rewriting
   *  its fields; used by the detail-pane drag-and-drop add flow. */
  addAttachments: (uuid: string, attachments: AttachmentInput[]) => Promise<VaultState>;
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
/** Authoritative state cached per open backend session. Every renderer invoke
 * captures one id and updates only that session's cache; a late response can
 * never overwrite the currently visible tab. */
const sessionStates = new Map<string, VaultState>();
/** Epoch changes when a session is intentionally replaced from an external
 * source (remote download). A pre-replacement response must be rejected even
 * if its edit revision happens to be numerically higher. */
const sessionEpochs = new Map<string, number>();
/** Tokens of attachments extracted to the temp dir, bound to the session they
 * originated from; discarded on lock/close. */
let tempAttachmentTokens = new Map<string, string>();

/** Database custom icons cached per session, kept across mutation snapshots
 * that omit the image payload. */
const iconCaches = new Map<string, Record<string, string>>();
/** Every backend registry/topology change (open/create/close/switch) and its
 * matching frontend publish completes in invocation order. Ordinary renderer
 * commands do not wait because they address their captured session id. */
const topologyQueue = new SessionSwitchQueue();
/** Temporary override used only while one async service method executes its
 * synchronous pre-await capture. Composite actions must wrap each nested
 * service call separately; a global async context would be unsafe when two
 * sessions run concurrently. */
let invocationSessionId: string | null = null;

function applyBackendState(result: VaultState, sessionId?: string | null): VaultState {
  if (!sessionId) return result;
  if (result.customIcons !== undefined) iconCaches.set(sessionId, result.customIcons);
  result.customIcons = { ...(iconCaches.get(sessionId) ?? {}) };
  return result;
}

function captureSessionId(): string {
  const sessionId = invocationSessionId ?? activeSessionId;
  if (!sessionId) throw new Error("数据库未打开");
  return sessionId;
}

/** Cache one session result and publish it only if that tab is still visible. */
function commitSessionState(sessionId: string, result: VaultState): VaultState {
  const current = sessionStates.get(sessionId);
  if (current && result.revision < current.revision) return current;
  const normalized = applyBackendState(result, sessionId);
  const committed = commitNewestSessionState(sessionStates, sessionId, normalized);
  if (activeSessionId === sessionId) state.set(committed);
  return committed;
}

function captureSessionEpoch(sessionId: string): number {
  return sessionEpochs.get(sessionId) ?? 0;
}

function commitSessionStateAtEpoch(
  sessionId: string,
  epoch: number,
  result: VaultState,
): VaultState {
  if (captureSessionEpoch(sessionId) !== epoch) {
    return sessionStates.get(sessionId) ?? applyBackendState(result, sessionId);
  }
  return commitSessionState(sessionId, result);
}

function replaceSessionState(sessionId: string, result: VaultState): VaultState {
  sessionEpochs.set(sessionId, captureSessionEpoch(sessionId) + 1);
  const normalized = applyBackendState(result, sessionId);
  sessionStates.set(sessionId, normalized);
  if (activeSessionId === sessionId) state.set(normalized);
  return normalized;
}

/** Apply a lightweight backend mutation delta to the current store state and
 *  return the merged `VaultState` (or null when no session is open). */
function applyBackendDelta(sessionId: string, delta: MutationDelta): VaultState | null {
  const current =
    sessionStates.get(sessionId) ?? (activeSessionId === sessionId ? get(state) : null);
  if (!current) return null;
  let entryPatches: Map<string, Partial<VaultEntry>> | null = null;
  let groupPatches: Map<string, Partial<VaultGroup>> | null = null;
  if (delta.kind === "favorite") {
    entryPatches = new Map([[delta.uuid, { favorite: delta.favorite }]]);
  } else if (delta.kind === "groupsExpanded") {
    groupPatches = new Map(
      Object.entries(delta.groups).map(([uuid, expanded]) => [uuid, { isExpanded: expanded }]),
    );
  }
  const next = { ...current, revision: delta.revision };
  next.root = applyTreeDelta(current.root, entryPatches, groupPatches);
  return commitSessionState(sessionId, next);
}

/** Clone only the groups along the paths to the mutated nodes and the mutated
 *  nodes themselves, sharing every untouched subtree. Delta mutations must not
 *  deep-clone the whole vault (full JSON serialize/parse on every favorite
 *  toggle), and committed snapshots are immutable — a stale reference must
 *  never observe a mutation, so nodes are replaced, never edited in place. */
function applyTreeDelta(
  root: VaultGroup,
  entryPatches: Map<string, Partial<VaultEntry>> | null,
  groupPatches: Map<string, Partial<VaultGroup>> | null,
): VaultGroup {
  let entries = root.entries;
  if (entryPatches && root.entries.some((entry) => entryPatches.has(entry.uuid))) {
    entries = root.entries.map((entry) => {
      const patch = entryPatches.get(entry.uuid);
      return patch ? { ...entry, ...patch } : entry;
    });
  }
  let children = root.children;
  for (let i = 0; i < children.length; i += 1) {
    const nextChild = applyTreeDelta(children[i], entryPatches, groupPatches);
    if (nextChild !== children[i]) {
      if (children === root.children) children = [...children];
      children[i] = nextChild;
    }
  }
  const groupPatch = groupPatches?.get(root.uuid);
  const changed =
    groupPatch !== undefined || entries !== root.entries || children !== root.children;
  if (!changed) return root;
  return groupPatch
    ? { ...root, ...groupPatch, entries, children }
    : { ...root, entries, children };
}

function deepClone<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function applyEdit(mutator: (draft: VaultState) => void, markDirty = true): VaultState {
  const current = browserState ?? buildDemoVaultState();
  const next = deepClone(current);
  if (markDirty) {
    next.dirty = true;
    next.modifiedAt = new Date().toISOString();
  }
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

/** Strip secrets (passwords, TOTP seeds, protected custom-field values) from
 * a structural clone so the browser-demo persistence never writes them to
 * localStorage. On reload the entries keep their metadata but secrets are
 * gone. */
function withoutSecrets(value: VaultState): VaultState {
  const clone = deepClone(value);
  const strip = (group: VaultGroup): void => {
    for (const entry of group.entries) {
      delete entry.password;
      delete entry.totp;
      // The demo keeps custom fields inline; protected values must not.
      if (entry.customFields) {
        for (const field of entry.customFields) {
          if (field.protected) field.value = "";
        }
      }
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

async function invokeSession<T>(
  command: string,
  args: Record<string, unknown> = {},
  sessionId = captureSessionId(),
): Promise<T> {
  return backendInvoke<T>(command, { ...args, sessionId });
}

async function invokeSessionState(
  command: string,
  args: Record<string, unknown> = {},
): Promise<VaultState> {
  const sessionId = captureSessionId();
  const epoch = captureSessionEpoch(sessionId);
  const result = await invokeSession<VaultState>(command, args, sessionId);
  return commitSessionStateAtEpoch(sessionId, epoch, result);
}

async function invokeSessionDelta(
  command: string,
  args: Record<string, unknown> = {},
): Promise<VaultState> {
  const sessionId = captureSessionId();
  const epoch = captureSessionEpoch(sessionId);
  const delta = await invokeSession<MutationDelta>(command, args, sessionId);
  if (captureSessionEpoch(sessionId) !== epoch) {
    const current = sessionStates.get(sessionId);
    if (!current) throw new Error("数据库未打开");
    return current;
  }
  const result = applyBackendDelta(sessionId, delta);
  if (!result) throw new Error("数据库未打开");
  return result;
}

async function refreshInternal(sessionId = activeSessionId): Promise<VaultState | null> {
  if (isTauriRuntime()) {
    const epoch = sessionId ? captureSessionEpoch(sessionId) : 0;
    const value = await backendInvoke<VaultState | null>("get_vault_state", {
      sessionId,
    });
    if (value && sessionId) return commitSessionStateAtEpoch(sessionId, epoch, value);
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
    activeSessionId = resolveListedActiveId(activeSessionId, list);
    tabs.set(list);
    activeId.set(activeSessionId);
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

/** Best-effort cleanup of every extracted temp attachment (lock/close path). */
async function discardTempAttachments(): Promise<void> {
  await discardTempAttachmentsForSession();
}

/** Discard only one tab's extracted attachments when closing that tab. */
async function discardTempAttachmentsForSession(sessionId?: string): Promise<void> {
  const tokens = [...tempAttachmentTokens.entries()]
    .filter(([, owner]) => sessionId === undefined || owner === sessionId)
    .map(([token]) => token);
  if (!isTauriRuntime()) {
    for (const token of tokens) tempAttachmentTokens.delete(token);
    return;
  }
  await Promise.all(
    tokens.map(async (token) => {
      try {
        await backendInvoke("cleanup_attachment_temp", { token });
        tempAttachmentTokens.delete(token);
      } catch {
        // Keep the token so a later close/lock cleanup can retry it.
      }
    }),
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

  /** Current vault snapshot (or `null` when closed) without subscribing. */
  get(): VaultState | null {
    return get(state);
  },

  /** Backend session id of the active tab (`null` when closed). */
  getActiveSessionId(): string | null {
    return activeSessionId;
  },

  /** Run an operation with every session-scoped vault call bound to
   *  `sessionId`, even when another tab is active. Async work after the first
   *  await is no longer bound — keep overrides synchronous. */
  callInSession<T>(sessionId: string, operation: () => Promise<T>): Promise<T> {
    const previous = invocationSessionId;
    invocationSessionId = sessionId;
    try {
      // Async functions execute synchronously until their first await; every
      // session-scoped vault method captures the override in that segment.
      return operation();
    } finally {
      invocationSessionId = previous;
    }
  },

  /** Database storage settings (KDF/cipher/compression/history cap) of the
   *  addressed session; `null` outside Tauri. */
  async getDatabaseSettings(): Promise<DatabaseSettings | null> {
    if (!isTauriRuntime()) return null;
    const sessionId = captureSessionId();
    return backendInvoke<DatabaseSettings | null>("get_database_settings", { sessionId });
  },

  /** Patch database storage settings; epoch-guarded commit. */
  async updateDatabaseSettings(patch: DatabaseSettingsPatch): Promise<VaultState> {
    if (isTauriRuntime()) {
      const sessionId = captureSessionId();
      const epoch = captureSessionEpoch(sessionId);
      const result = await backendInvoke<VaultState>("update_database_settings", {
        sessionId,
        patch,
      });
      return commitSessionStateAtEpoch(sessionId, epoch, result);
    }
    throw new Error("浏览器预览不支持数据库设置修改");
  },

  remembered: remembered.subscribe,
  /** Last opened local vault for the lock screen quick-reopen; `null` after a
   *  remote open (a remote session cannot be reopened from the lock screen). */
  getRemembered(): RememberedVault | null {
    return get(remembered);
  },
  clearRemembered(): void {
    remembered.set(null);
  },

  /** Open a local KDBX file; serializes through the topology queue, records
   *  the path for quick reopen and recent files, and switches the tab bar. */
  async open(path, password, keyfile): Promise<VaultState> {
    if (isTauriRuntime()) {
      return topologyQueue.enqueue(async () => {
        const result = await backendInvoke<VaultOpenResult>("open_vault", {
          path,
          password,
          keyfile: keyfile || null,
        });
        activeSessionId = result.sessionId;
        sessionEpochs.set(result.sessionId, 0);
        commitSessionState(result.sessionId, result.state);
        remembered.set({ path: result.state.path, fileName: result.state.fileName });
        rememberRecent(result.state.path);
        await refreshTabs();
        return result.state;
      });
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
      return topologyQueue.enqueue(async () => {
        const result = await backendInvoke<VaultOpenResult>("create_vault", {
          ...request,
          keyfile: request.keyfile || null,
        });
        activeSessionId = result.sessionId;
        sessionEpochs.set(result.sessionId, 0);
        commitSessionState(result.sessionId, result.state);
        remembered.set({ path: result.state.path, fileName: result.state.fileName });
        rememberRecent(result.state.path);
        await refreshTabs();
        return result.state;
      });
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

  /** Close the active session: zeroizes its backend state, discards its temp
   *  attachments, drops the remembered credential when not persisted, and
   *  promotes the next tab (clearing all state when none remains). */
  async close(): Promise<void> {
    if (isTauriRuntime()) {
      const path = get(state)?.path;
      const closingId = captureSessionId();
      return topologyQueue.enqueue(async () => {
        await backendInvoke("close_vault", { sessionId: closingId });
        sessionStates.delete(closingId);
        iconCaches.delete(closingId);
        sessionEpochs.delete(closingId);
        if (path && !get(appSettings).security.rememberPassword) {
          void backendInvoke("clear_saved_credential", { path }).catch(() => undefined);
        }
        const list = await backendInvoke<SessionInfo[]>("list_sessions");
        activeSessionId = list[0]?.sessionId ?? null;
        tabs.set(list);
        activeId.set(activeSessionId);
        const remaining = activeSessionId ? await refreshInternal(activeSessionId) : null;
        if (!remaining) {
          state.set(null);
          sessionStates.clear();
          iconCaches.clear();
          sessionEpochs.clear();
        } else if (remaining.path.startsWith("s3://")) {
          remembered.set(null);
        } else {
          remembered.set({ path: remaining.path, fileName: remaining.fileName });
        }
        await discardTempAttachmentsForSession(closingId);
      });
    }
    browserState = null;
    sessionStates.clear();
    iconCaches.clear();
    sessionEpochs.clear();
    state.set(null);
    tabs.set([]);
  },

  /** Lock-everything path shared by lock/idle/focus-loss: closes all backend
   *  sessions (zeroizing keys server-side) and discards every temp
   *  attachment directory. */
  async closeAll(): Promise<void> {
    if (isTauriRuntime()) {
      return topologyQueue.enqueue(async () => {
        await backendInvoke("close_all_vaults");
        activeSessionId = null;
        await discardTempAttachments();
        browserState = null;
        sessionStates.clear();
        iconCaches.clear();
        sessionEpochs.clear();
        state.set(null);
        tabs.set([]);
        activeId.set(null);
      });
    }
    await discardTempAttachments();
    browserState = null;
    sessionStates.clear();
    iconCaches.clear();
    sessionEpochs.clear();
    state.set(null);
    tabs.set([]);
    activeId.set(null);
  },

  /** List remote vault objects under the active profile's prefix (`.kdbx`
   *  first, key descending). Flushes pending settings so a just-added profile
   *  is visible to the backend. */
  async listRemoteObjects(): Promise<RemoteObject[]> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持远程库");
    await appSettings.flush();
    return backendInvoke<RemoteObject[]>("s3_list_objects", {
      profile: get(appSettings).activeRemote,
    });
  },

  /** Download and open a remote vault. `mode` is `"memory"` (upload back only)
   *  or `"local"` (mirror under Storage/remote/…); clears the remembered local
   *  path since a remote session cannot be reopened from the lock screen. */
  async openRemote(key, password, keyfile, mode): Promise<VaultState> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持远程库");
    return topologyQueue.enqueue(async () => {
      await appSettings.flush();
      const result = await backendInvoke<VaultOpenResult>("open_remote_vault", {
        profile: get(appSettings).activeRemote,
        key,
        password,
        keyfile: keyfile || null,
        mode,
      });
      activeSessionId = result.sessionId;
      sessionEpochs.set(result.sessionId, 0);
      commitSessionState(result.sessionId, result.state);
      // A remote session cannot be reopened from the lock screen; clear the
      // remembered local path so unlocking never silently targets the old vault.
      remembered.set(null);
      rememberRecent(result.state.path);
      await refreshTabs();
      return result.state;
    });
  },

  /** Create a fresh remote vault and open it with the same mode semantics as
   *  `openRemote`. */
  async createRemote(key, password, kdf, cipher, compression, keyfile, mode): Promise<VaultState> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持远程库");
    return topologyQueue.enqueue(async () => {
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
      sessionEpochs.set(result.sessionId, 0);
      commitSessionState(result.sessionId, result.state);
      // See `openRemote`: a remote session is never the lock-screen quick-reopen
      // target, so drop any stale remembered local path.
      remembered.set(null);
      rememberRecent(result.state.path);
      await refreshTabs();
      return result.state;
    });
  },

  /** Persist dirty changes; `force` overwrites a detected remote conflict.
   *  Rejects with the `REMOTE_CHANGED\n` sentinel when remote bytes moved. */
  async save(force = false): Promise<VaultState> {
    if (isTauriRuntime()) {
      const sessionId = captureSessionId();
      const epoch = captureSessionEpoch(sessionId);
      const result = await backendInvoke<VaultState>("save_vault", { sessionId, force });
      const committed = commitSessionStateAtEpoch(sessionId, epoch, result);
      await refreshTabs();
      return committed;
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    const saved = deepClone(current);
    saved.dirty = false;
    browserState = saved;
    state.set(saved);
    await browserPersist(saved);
    return saved;
  },

  /** Re-download the remote bytes and replace session state, discarding local
   *  unsaved edits (the caller confirms that first). */
  async refreshRemote(): Promise<VaultState> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持远程刷新");
    const sessionId = captureSessionId();
    const result = await invokeSession<VaultState>("refresh_remote_vault", {}, sessionId);
    const replaced = replaceSessionState(sessionId, result);
    await refreshTabs();
    return replaced;
  },

  /** Merge the remote vault's latest bytes into the session by entry/group
   *  UUID + last-modified, persisting the merged result back. Remote only. */
  async mergeRemote(): Promise<VaultState> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持远程合并");
    const result = await invokeSessionState("merge_remote_vault");
    await refreshTabs();
    return result;
  },

  /** Save As: persist to a new local path and switch the session target. */
  async saveAs(path: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      const sessionId = captureSessionId();
      const epoch = captureSessionEpoch(sessionId);
      const result = await backendInvoke<VaultState>("save_vault_as", { sessionId, path });
      const committed = commitSessionStateAtEpoch(sessionId, epoch, result);
      await refreshTabs();
      return committed;
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

  /** Re-encrypt the session with a new master password and/or keyfile. */
  async changeMasterKey(password: string, keyfile: string | null): Promise<VaultState> {
    if (isTauriRuntime()) {
      const sessionId = captureSessionId();
      const epoch = captureSessionEpoch(sessionId);
      const result = await backendInvoke<VaultState>("change_master_key", {
        sessionId,
        password,
        keyfile,
      });
      const committed = commitSessionStateAtEpoch(sessionId, epoch, result);
      await refreshTabs();
      return committed;
    }
    return vault.save();
  },

  async addEntry(input: EntryInput): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionState("add_entry", { input });
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
      return invokeSessionState("import_entries", { inputs });
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
      return invokeSessionState("update_entry", { uuid, input });
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
      return invokeSessionState("update_entry_flags", {
        uuid,
        overrideUrl: flags.overrideUrl ?? null,
        qualityCheck: flags.qualityCheck ?? null,
        foregroundColor: flags.foregroundColor ?? null,
      });
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
      return invokeSessionState("update_entries", { uuids, patch });
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

  /** Compute the entry's current TOTP/HOTP/Steam code with its validity window. */
  async totpCode(uuid: string): Promise<TotpCode> {
    if (isTauriRuntime()) {
      return invokeSession<TotpCode>("totp_code", { uuid });
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    const entry = findEntry(current.root, uuid);
    if (!entry) throw new Error("entry not found");
    if (!entry.totp) throw new Error("该条目没有 TOTP 种子");
    return computeTotp(entry.totp);
  },

  /** Resolve the entry's plaintext password server-side; only for explicit
   *  user actions (copy/reveal), never part of list state. */
  async getEntryPassword(uuid: string): Promise<string> {
    if (isTauriRuntime()) {
      return invokeSession<string>("get_entry_password", { uuid });
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    return findEntry(current.root, uuid)?.password ?? "";
  },

  async getEntryTotp(uuid: string): Promise<string | null> {
    if (isTauriRuntime()) {
      return invokeSession<string | null>("get_entry_totp", { uuid });
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    return findEntry(current.root, uuid)?.totp ?? null;
  },

  async getCustomFieldValue(uuid: string, name: string): Promise<string | null> {
    if (isTauriRuntime()) {
      return invokeSession<string | null>("get_custom_field_value", { uuid, name });
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    return findEntry(current.root, uuid)?.customFields?.find((f) => f.name === name)?.value ?? null;
  },

  /** Write back a custom field value, preserving or setting its protected
   *  flag; editing never downgrades an existing protection. */
  async updateCustomFieldValue(
    uuid: string,
    name: string,
    value: string,
    protectedField: boolean,
  ): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionState("update_custom_field", {
        uuid,
        name,
        value,
        protected: protectedField,
      });
    }
    const result = applyEdit((draft) => {
      const groups = collectAllGroups(draft.root);
      for (const group of groups) {
        const entry = group.entries.find((e) => e.uuid === uuid);
        if (entry) {
          const field = entry.customFields?.find((f) => f.name === name);
          if (field) field.value = value;
          return;
        }
      }
      throw new Error("entry not found");
    });
    state.set(applyBackendState(result));
    return result;
  },

  /** Session-wide security audit (reuse, weakness, expiry; no secrets in
   *  results). */
  async securityReport(): Promise<SecurityReport> {
    if (isTauriRuntime()) {
      return invokeSession<SecurityReport>("security_report");
    }
    const current = browserState ?? (await ensureBrowserLoaded());
    return computeSecurityReport(current.root);
  },

  /** Edit-distance clusters of similar passwords (values stay backend-side). */
  async similarPasswords(): Promise<SimilarPasswordGroup[]> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持相似密码检查");
    return invokeSession<SimilarPasswordGroup[]>("similar_passwords");
  },

  async clearAllHistory(): Promise<number> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持历史清理");
    const sessionId = captureSessionId();
    const epoch = captureSessionEpoch(sessionId);
    const result = await invokeSession<HistoryCleanResult>("clear_all_history", {}, sessionId);
    commitSessionStateAtEpoch(sessionId, epoch, result.state);
    return result.cleared;
  },

  async expiredEntries(): Promise<ExpiredEntry[]> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持过期维护");
    return invokeSession<ExpiredEntry[]>("expired_entries");
  },

  async changeTimeline(): Promise<ChangeTimelineEvent[]> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持变更时间线");
    return invokeSession<ChangeTimelineEvent[]>("change_timeline");
  },

  /** Opt-in k-anonymity breach check; `uuids` narrows the run to selected
   *  entries. Only SHA-1 prefixes leave the machine. */
  async checkHibp(uuids?: string[]): Promise<BreachFinding[]> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持 HIBP 检查");
    return invokeSession<BreachFinding[]>("check_hibp", {
      uuids: uuids && uuids.length > 0 ? uuids : undefined,
    });
  },

  /** Fetch site icons for all (or the given) entries; state is refreshed only
   *  when no user edit landed during the network run. */
  async downloadFavicons(uuids?: string[]): Promise<FaviconReport> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持下载图标");
    const sessionId = captureSessionId();
    const epoch = captureSessionEpoch(sessionId);
    const report = await backendInvoke<FaviconReport>("download_favicons", {
      sessionId,
      uuids: uuids && uuids.length > 0 ? uuids : undefined,
    });
    if (captureSessionEpoch(sessionId) === epoch) await refreshInternal(sessionId);
    await refreshTabs();
    return report;
  },

  async toggleFavorite(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionDelta("toggle_favorite", { uuid });
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
    await invokeSession<void>("auto_type", { uuid, sequence });
  },

  async saveAttachment(uuid: string, name: string, dest: string): Promise<void> {
    await invokeSession<void>("save_attachment", { uuid, name, dest });
  },

  async previewAttachment(uuid: string, name: string): Promise<AttachmentPreview> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持附件预览");
    return invokeSession<AttachmentPreview>("preview_attachment", { uuid, name });
  },

  async openAttachmentTemp(uuid: string, name: string): Promise<TempAttachmentRef> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持外部打开附件");
    const ref = await invokeSession<TempAttachmentRef>("open_attachment_temp", { uuid, name });
    tempAttachmentTokens.set(ref.token, ref.sessionId);
    return ref;
  },

  /** Discard a temp extraction directory; failure keeps the token registered
   *  so the session close/lock path can retry it. */
  async cleanupAttachmentTemp(token: string): Promise<void> {
    if (!isTauriRuntime()) {
      tempAttachmentTokens.delete(token);
      return;
    }
    try {
      await backendInvoke("cleanup_attachment_temp", { token });
      tempAttachmentTokens.delete(token);
    } catch {
      // Keep the token so the session close/lock path can retry it.
    }
  },

  /** Import a previously extracted attachment back into the entry from its
   *  temp token (session-bound), then clean the token up. */
  async importAttachmentFromTemp(uuid: string, name: string, token: string): Promise<VaultState> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持附件导入");
    const sessionId = tempAttachmentTokens.get(token);
    if (!sessionId) throw new Error("临时附件已清理或不存在");
    const epoch = captureSessionEpoch(sessionId);
    const result = await invokeSession<VaultState>(
      "import_attachment_from_temp",
      {
        uuid,
        name,
        token,
      },
      sessionId,
    );
    try {
      await backendInvoke("cleanup_attachment_temp", { token });
      tempAttachmentTokens.delete(token);
    } catch {
      // Keep the token so the session close/lock path can retry it.
    }
    return commitSessionStateAtEpoch(sessionId, epoch, result);
  },

  async addAttachments(uuid: string, attachments: AttachmentInput[]): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionState("add_attachments", { uuid, attachments });
    }
    const result = applyEdit((draft) => {
      const groups = collectAllGroups(draft.root);
      for (const group of groups) {
        const entry = group.entries.find((e) => e.uuid === uuid);
        if (entry) {
          const existing = entry.attachments ?? [];
          for (const incoming of attachments) {
            if (!incoming.data) continue;
            // Approximate size from base64 length for the demo list display.
            const size = Math.floor((incoming.data.length * 3) / 4);
            const index = existing.findIndex((a) => a.name === incoming.name);
            if (index >= 0) existing[index] = { name: incoming.name, size };
            else existing.push({ name: incoming.name, size });
          }
          entry.attachments = existing;
          entry.modified = new Date().toISOString();
          return;
        }
      }
      throw new Error("entry not found");
    });
    state.set(applyBackendState(result));
    return result;
  },

  async addGroup(input: GroupInput): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionState("add_group", { input });
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
      return invokeSessionState("rename_group", { uuid, name });
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
      return invokeSessionState("set_group_icon", { uuid, icon });
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
      return invokeSessionState("update_group_meta", {
        uuid,
        notes: meta.notes ?? null,
        tags: meta.tags ?? null,
        enableSearching: meta.enableSearching ?? null,
      });
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
      return invokeSessionDelta("set_group_expanded", {
        uuid,
        expanded,
      });
    }
    const result = applyEdit((draft) => {
      const group = findGroup(draft.root, uuid);
      if (!group) throw new Error("group not found");
      group.isExpanded = expanded;
    }, false);
    state.set(applyBackendState(result));
    return result;
  },

  async setGroupsExpanded(uuids: string[], expanded: boolean): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionDelta("set_groups_expanded", {
        uuids,
        expanded,
      });
    }
    if (uuids.length === 0) {
      const result = deepClone(browserState ?? buildDemoVaultState());
      state.set(applyBackendState(result));
      return result;
    }
    const result = applyEdit((draft) => {
      setGroupsExpandedInTree(draft.root, uuids, expanded);
    }, false);
    state.set(applyBackendState(result));
    return result;
  },

  async updateEntryAutoType(uuid: string, input: EntryAutoTypeConfig): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionState("update_entry_autotype", { uuid, input });
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
      return invokeSessionState("update_group_autotype", { uuid, input });
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
      return invokeSessionState("update_db_meta", args);
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
      return invokeSessionState("delete_entry", { uuid });
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

  /** Delete entries to the recycle bin (multi-select). */
  async deleteEntries(uuids: string[]): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionState("delete_entries", { uuids });
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
      return invokeSessionState("move_entry", { uuid, groupUuid });
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
      return invokeSessionState("restore_entry", { uuid });
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
      return invokeSession<HistoryVersion[]>("get_entry_history", { uuid });
    }
    return [];
  },

  async deleteEntryHistory(uuid: string, index: number): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionState("delete_entry_history", { uuid, index });
    }
    throw new Error("浏览器模式不支持删除历史版本");
  },

  async getEntryStorage(uuid: string): Promise<EntryStorage> {
    if (isTauriRuntime()) {
      return invokeSession<EntryStorage>("get_entry_storage", { uuid });
    }
    return { fields: 0, attachments: 0, history: 0, total: 0 };
  },

  async restoreEntryVersion(uuid: string, index: number): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionState("restore_entry_version", { uuid, index });
    }
    throw new Error("浏览器模式不支持历史版本恢复");
  },

  async deleteGroup(uuid: string): Promise<VaultState> {
    if (isTauriRuntime()) {
      return invokeSessionState("delete_group", { uuid });
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
      return invokeSessionState("restore_group", { uuid });
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
      return invokeSessionState("empty_recycle_bin");
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
    if (isTauriRuntime() && !activeSessionId) await refreshTabs();
    await refreshInternal(activeSessionId);
    await refreshTabs();
  },

  /** Switch the active tab: validates an uncached session before swapping the
   *  backend-active session, epoch-guarding the commit so a failed load never
   *  leaves renderer and backend pointing at different sessions. */
  async setActiveSession(sessionId: string): Promise<VaultState> {
    if (!isTauriRuntime()) throw new Error("浏览器预览不支持多库标签");
    if (sessionId === activeSessionId) {
      const current = sessionStates.get(sessionId);
      if (current) return current;
    }
    const epoch = captureSessionEpoch(sessionId);
    // The whole validation + backend-active swap is serialized. An uncached
    // tab is validated before the backend switch, so a failed snapshot cannot
    // leave the renderer restored to A while the queued backend later moves to
    // B. Later clicks still run strictly after this complete attempt.
    const resolved = await switchSession({
      queue: topologyQueue,
      cached: sessionStates.get(sessionId),
      load: async () =>
        backendInvoke<VaultState | null>("get_vault_state", {
          sessionId,
        }),
      activate: async () => backendInvoke<VaultState>("set_active_session", { sessionId }),
      commit: (incoming) => commitSessionStateAtEpoch(sessionId, epoch, incoming),
      publish: async (committed) => {
        activeSessionId = sessionId;
        activeId.set(sessionId);
        state.set(committed);
        // The lock-screen quick-reopen follows the newly active tab; remote
        // sessions never become a quick-reopen target.
        if (committed.path.startsWith("s3://")) {
          remembered.set(null);
        } else {
          remembered.set({ path: committed.path, fileName: committed.fileName });
        }
        await refreshTabs();
      },
    });
    return resolved;
  },

  /** Close one tab's session (same semantics as `close`, addressed by id) and
   *  promote the next tab. */
  async closeTab(sessionId: string): Promise<void> {
    if (!isTauriRuntime()) return;
    const tab = get(tabs).find((t) => t.sessionId === sessionId);
    return topologyQueue.enqueue(async () => {
      await backendInvoke("close_vault", { sessionId });
      sessionStates.delete(sessionId);
      iconCaches.delete(sessionId);
      sessionEpochs.delete(sessionId);
      await discardTempAttachmentsForSession(sessionId);
      if (tab?.path && !get(appSettings).security.rememberPassword) {
        void backendInvoke("clear_saved_credential", { path: tab.path }).catch(() => undefined);
      }
      const list = await backendInvoke<SessionInfo[]>("list_sessions");
      activeSessionId = list[0]?.sessionId ?? null;
      tabs.set(list);
      activeId.set(activeSessionId);
      const remaining = activeSessionId ? await refreshInternal(activeSessionId) : null;
      if (!remaining) {
        state.set(null);
      } else if (remaining.path.startsWith("s3://")) {
        remembered.set(null);
      } else {
        remembered.set({ path: remaining.path, fileName: remaining.fileName });
      }
    });
  },
};

export { ROOT_GROUP_NAME };
