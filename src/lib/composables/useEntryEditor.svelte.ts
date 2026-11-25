//! Entry-editor dialog flow: open modes (create / edit / edit-multi) and the
//! session-guarded save pipeline. Extracted from `+page.svelte` — the page
//! keeps selection ownership and lookup helpers, injected here as closures so
//! every staleness branch and toast behaves exactly as before.

import { get } from "svelte/store";
import { appSettings } from "$lib/services/settings";
import { t } from "$lib/i18n";
import type { TipKind } from "$lib/services/tips";
import { vault } from "$lib/services/vault";
import type {
  EntryAutoTypeConfig,
  EntryFlags,
  EntryInput,
  EntryPatch,
  WebsiteRulesChange,
  VaultEntry,
  VaultState,
} from "$lib/types/vault";

/** Current UI locale for toasts (read lazily: the language can change at
 *  runtime via settings). */
function lang() {
  return get(appSettings).general.language;
}
import { SessionViewGuard } from "$lib/utils/session-state";

export type EntryEditorOptions = {
  /** Shared session-staleness guard owned by the page. */
  sessionView: SessionViewGuard;
  notify: (message: string, kind?: TipKind) => void;
  /** Resolve an entry by uuid inside a fresh snapshot (page helper). */
  findEntry: (state: VaultState | null, uuid: string | null) => VaultEntry | null;
  /** Rebind the single selection after a successful save (create/edit). */
  setSingleSelection: (entry: VaultEntry | null) => void;
  /** Replace the single-selection highlight without touching the multi-row
   *  set — used by the edit-multi completion, which deliberately keeps every
   *  selected row selected. */
  setSelectedEntry: (entry: VaultEntry | null) => void;
  /** Current single selection (edit-multi rebind source). */
  getSelectedEntry: () => VaultEntry | null;
  /** KeePass "auto-save after modifying an entry". The page owns the guarded
   *  `vault.save()` call (session view + busy token); the editor only decides
   *  whether to trigger it when the setting is enabled. */
  persistAfterEdit?: () => Promise<void>;
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
    websiteRules?: WebsiteRulesChange | null,
  ): Promise<void>;
};

export function useEntryEditor(options: EntryEditorOptions): EntryEditor {
  let editorOpen = $state(false);
  let editorMode = $state<"create" | "edit" | "edit-multi">("create");
  let editEntry = $state<VaultEntry | null>(null);
  let editEntries = $state<VaultEntry[]>([]);

  /** Persist the session when KeePass auto-save-after-edit is enabled.
   *  Best-effort: a failed save must not roll back the in-memory edit or
   *  discard the success notice — the vault stays dirty for a manual retry. */
  async function autoPersist(view: ReturnType<SessionViewGuard["capture"]>): Promise<void> {
    if (!get(appSettings).security.autoSaveAfterEdit || !options.persistAfterEdit) return;
    if (!view || !options.sessionView.isCurrent(view)) return;
    try {
      await options.persistAfterEdit();
    } catch {
      // The edit already succeeded in-session; keep it dirty for manual save.
    }
  }

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
    websiteRules?: WebsiteRulesChange | null,
  ): Promise<void> {
    const view = options.sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    const mode = editorMode;
    const targetEntry = editEntry;
    const targetEntries = [...editEntries];
    try {
      if (mode === "create" && input) {
        const { state, uuid } = await vault.callInSession(sessionId, () =>
          vault.saveEntry({
            target: { mode: "create" },
            input,
            autoType: autotype,
            flags,
            websiteRules,
          }),
        );
        if (!options.sessionView.isCurrent(view)) return;
        options.setSingleSelection(options.findEntry(state, uuid));
        editorOpen = false;
        options.notify(t(lang(), "editor.createdEntry"));
        await autoPersist(view);
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
        options.notify(t(lang(), "editor.updatedEntries", { n: uuids.length }));
        await autoPersist(view);
      } else if (mode === "edit" && input && targetEntry) {
        const uuid = targetEntry.uuid;
        const { state } = await vault.callInSession(sessionId, () =>
          vault.saveEntry({
            target: { mode: "edit", uuid },
            input,
            autoType: autotype,
            flags,
            websiteRules,
          }),
        );
        if (!options.sessionView.isCurrent(view)) return;
        options.setSingleSelection(options.findEntry(state, uuid));
        editorOpen = false;
        options.notify(t(lang(), "editor.savedChanges"));
        await autoPersist(view);
      }
    } catch (e) {
      if (options.sessionView.isCurrent(view))
        options.notify(t(lang(), "editor.opFailed", { e: String(e) }), "error");
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
