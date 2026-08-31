//! Entry-selection model for the main window: single select, shift-range,
//! ctrl/cmd toggle, select-all over the visible (sorted) rows, plus the
//! anchor bookkeeping the range logic needs. Extracted from `+page.svelte`;
//! the page keeps flows that react to selection changes (detail auto-open,
//! favorite rebind after save) and drives this model through its accessors.

import type { VaultEntry } from "$lib/types/vault";

export type VaultSelectionOptions = {
  /** Uuids of the currently visible (sorted) rows, top to bottom — the basis
   *  for shift-range selection and select-all. */
  visibleUuids: () => string[];
};

export type VaultSelection = {
  selectedEntry: VaultEntry | null;
  selectedUuids: Set<string>;
  selectionAnchor: string | null;
  selectionVersion: number;
  setSingleSelection(entry: VaultEntry | null): void;
  handleRowClick(event: MouseEvent, entry: VaultEntry): void;
};

export function useVaultSelection(options: VaultSelectionOptions): VaultSelection {
  let selectedEntry = $state<VaultEntry | null>(null);
  let selectedUuids = $state<Set<string>>(new Set());
  let selectionAnchor = $state<string | null>(null);
  let selectionVersion = $state(0);

  function setSingleSelection(entry: VaultEntry | null): void {
    selectedUuids = entry ? new Set([entry.uuid]) : new Set();
    selectionAnchor = entry?.uuid ?? null;
    selectedEntry = entry;
    selectionVersion++;
  }

  function handleRowClick(event: MouseEvent, entry: VaultEntry): void {
    if (event.shiftKey && selectionAnchor) {
      const uuids = options.visibleUuids();
      const start = uuids.indexOf(selectionAnchor);
      const end = uuids.indexOf(entry.uuid);
      if (start !== -1 && end !== -1) {
        const [lo, hi] = start <= end ? [start, end] : [end, start];
        selectedUuids = new Set(uuids.slice(lo, hi + 1));
        selectionAnchor = entry.uuid;
        selectedEntry = entry;
        selectionVersion++;
        return;
      }
    }
    if (event.ctrlKey || event.metaKey) {
      const next = new Set(selectedUuids);
      if (next.has(entry.uuid)) {
        next.delete(entry.uuid);
      } else {
        next.add(entry.uuid);
      }
      selectedUuids = next;
      selectionAnchor = entry.uuid;
      selectedEntry = entry;
      selectionVersion++;
      return;
    }
    setSingleSelection(entry);
  }

  return {
    get selectedEntry() {
      return selectedEntry;
    },
    set selectedEntry(value) {
      selectedEntry = value;
    },
    get selectedUuids() {
      return selectedUuids;
    },
    set selectedUuids(value) {
      selectedUuids = value;
    },
    get selectionAnchor() {
      return selectionAnchor;
    },
    set selectionAnchor(value) {
      selectionAnchor = value;
    },
    get selectionVersion() {
      return selectionVersion;
    },
    set selectionVersion(value) {
      selectionVersion = value;
    },
    setSingleSelection,
    handleRowClick,
  };
}
