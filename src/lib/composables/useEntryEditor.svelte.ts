//! Entry-editor dialog flow: open modes (create / edit / edit-multi) and the
//! session-guarded save pipeline. Extracted from `+page.svelte` — the page
//! keeps selection ownership and lookup helpers, injected here as closures so
//! every staleness branch and toast behaves exactly as before.

import { vault } from "$lib/services/vault";
import type {
  EntryAutoTypeConfig,
  EntryFlags,
  EntryInput,
  EntryPatch,
  VaultEntry,
  VaultState,
} from "$lib/types/vault";
import { SessionViewGuard } from "$lib/utils/session-state";

export type EntryEditorOptions = {
  /** Shared session-staleness guard owned by the page. */
  sessionView: SessionViewGuard;
  notify: (message: string) => void;
  /** Resolve an entry by uuid inside a fresh snapshot (page helper). */
  findEntry: (state: VaultState | null, uuid: string | null) => VaultEntry | null;
  /** Locate the just-created entry (backend generates the uuid). */
  findNewestInGroup: (state: VaultState, groupUuid: string) => VaultEntry | null;
  /** Rebind the single selection after a successful save (create/edit). */
  setSingleSelection: (entry: VaultEntry | null) => void;
  /** Replace the single-selection highlight without touching the multi-row
   *  set — used by the edit-multi completion, which deliberately keeps every
   *  selected row selected. */
  setSelectedEntry: (entry: VaultEntry | null) => void;
  /** Current single selection (edit-multi rebind source). */
  getSelectedEntry: () => VaultEntry | null;
};

export type EntryEditor = {
  readonly editorOpen: boolean;
  editorMode: "create" | "edit" | "edit-multi";
  readonly editEntry: VaultEntry | null;
  editEntries: VaultEntry[];
  openCreate(): void;
  /** Open the editor for one entry; the multi-select decision stays with the
   *  caller — pass the collected multi rows via `multiEntries`. */
  openEdit(entry: VaultEntry, multiEntries?: VaultEntry[]): void;
  close(): void;
  /** Full teardown used by tab-switch/vault-close resets. */
  reset(): void;
  handleSave(
    input: EntryInput | null,
    patch: EntryPatch | null,
    autotype: EntryAutoTypeConfig | null,
    flags?: EntryFlags | null,
  ): Promise<void>;
};

export function useEntryEditor(options: EntryEditorOptions): EntryEditor {
  let editorOpen = $state(false);
  let editorMode = $state<"create" | "edit" | "edit-multi">("create");
  let editEntry = $state<VaultEntry | null>(null);
  let editEntries = $state<VaultEntry[]>([]);

  function openCreate(): void {
    editorMode = "create";
    editEntry = null;
    editEntries = [];
    editorOpen = true;
  }

  function openEdit(entry: VaultEntry, multiEntries?: VaultEntry[]): void {
    if (multiEntries && multiEntries.length >= 2) {
      editorMode = "edit-multi";
      editEntry = null;
      editEntries = multiEntries;
      editorOpen = true;
      return;
    }
    editorMode = "edit";
    editEntry = entry;
    editEntries = [];
    editorOpen = true;
  }

  function close(): void {
    editorOpen = false;
  }

  function reset(): void {
    editorOpen = false;
    editEntry = null;
    editEntries = [];
  }

  async function handleSave(
    input: EntryInput | null,
    patch: EntryPatch | null,
    autotype: EntryAutoTypeConfig | null,
    flags?: EntryFlags | null,
  ): Promise<void> {
    const view = options.sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    const mode = editorMode;
    const targetEntry = editEntry;
    const targetEntries = [...editEntries];
    try {
      if (mode === "create" && input) {
        let state = await vault.callInSession(sessionId, () => vault.addEntry(input));
        const created = options.findNewestInGroup(state, input.groupUuid);
        if (autotype && created) {
          state = await vault.callInSession(sessionId, () =>
            vault.updateEntryAutoType(created.uuid, autotype),
          );
        }
        if (flags && created) {
          state = await vault.callInSession(sessionId, () =>
            vault.updateEntryFlags(created.uuid, flags),
          );
        }
        if (!options.sessionView.isCurrent(view)) return;
        options.setSingleSelection(options.findEntry(state, created?.uuid ?? null));
        editorOpen = false;
        options.notify("已创建条目");
      } else if (mode === "edit-multi" && patch && targetEntries.length > 0) {
        const uuids = targetEntries.map((e) => e.uuid);
        const state = await vault.callInSession(sessionId, () => vault.updateEntries(uuids, patch));
        if (!options.sessionView.isCurrent(view)) return;
        // Deliberately not a full re-select: multi-edit keeps the whole row
        // selection intact and only rebinds the highlighted entry.
        options.setSelectedEntry(
          options.findEntry(state, options.getSelectedEntry()?.uuid ?? null),
        );
        editorOpen = false;
        options.notify(`已更新 ${uuids.length} 个条目`);
      } else if (mode === "edit" && input && targetEntry) {
        const uuid = targetEntry.uuid;
        let state = await vault.callInSession(sessionId, () => vault.updateEntry(uuid, input));
        if (autotype) {
          state = await vault.callInSession(sessionId, () =>
            vault.updateEntryAutoType(uuid, autotype),
          );
        }
        if (flags) {
          state = await vault.callInSession(sessionId, () => vault.updateEntryFlags(uuid, flags));
        }
        if (!options.sessionView.isCurrent(view)) return;
        options.setSingleSelection(options.findEntry(state, uuid));
        editorOpen = false;
        options.notify("已保存修改");
      }
    } catch (e) {
      if (options.sessionView.isCurrent(view)) options.notify(`操作失败：${e}`);
    }
  }

  return {
    get editorOpen() {
      return editorOpen;
    },
    get editorMode() {
      return editorMode;
    },
    set editorMode(value) {
      editorMode = value;
    },
    get editEntry() {
      return editEntry;
    },
    get editEntries() {
      return editEntries;
    },
    set editEntries(value) {
      editEntries = value;
    },
    openCreate,
    openEdit,
    close,
    reset,
    handleSave,
  };
}
