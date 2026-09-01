//! Panel layout state machine for the main window: group/detail panel widths,
//! detail-panel visibility (with selection auto-open suppression), the mobile
//! navigation drawer, pointer-drag resizing, and persistence ordering.
//!
//! Extracted from `+page.svelte`: the two effects here have documented
//! ordering hazards (settings mirror vs. fresh drag widths) and are easier to
//! reason about as one owned unit. The page supplies read-back closures for
//! the pieces that live elsewhere (entry columns, selected entry).

import { get } from "svelte/store";
import { appSettings } from "$lib/services/settings";
import type { EntryColumnState } from "$lib/types/settings";
import type { VaultEntry } from "$lib/types/vault";

export type PanelLayoutOptions = {
  /** Currently persisted entry columns. `saveLayout` writes them before
   *  `panelWidths` so the settings mirror cannot clobber freshly dragged
   *  column widths (see the ordering note in `saveLayout`). */
  entryColumns: () => EntryColumnState[];
  /** Width fallback for the URL column slot of `panelWidths.urlCol`. */
  urlColWidth: () => number;
  /** Selection whose change auto-opens the detail panel. */
  selectedEntry: () => VaultEntry | null;
  /** Monotonic version that bumps on every selection (even same uuid) to re-trigger auto-open after close. */
  selectionVersion?: () => number;
};

export type PanelLayout = {
  groupWidth: number;
  detailWidth: number;
  detailVisible: boolean;
  mobileNavOpen: boolean;
  /** Suppress the next selection-driven auto-open (right-click menus). */
  suppressNextAutoOpen: () => void;
  startDetailResize: (event: PointerEvent) => void;
  startGroupResize: (event: PointerEvent) => void;
  saveLayout: () => void;
};

export function usePanelLayout(options: PanelLayoutOptions): PanelLayout {
  let groupWidth = $state(get(appSettings).general.panelWidths.group);
  let detailWidth = $state(get(appSettings).general.panelWidths.detail);
  let detailVisible = $state(false);
  let showDetailOnSelect = $state(get(appSettings).general.showDetailOnSelect ?? true);
  /** Set before a selection change that must not auto-open the detail panel
   *  (right-click context menu). Consumed by the effect below. Deliberately
   *  non-reactive: the effect must not track it and re-run when it resets. */
  let suppressDetailAutoOpen = false;
  /** Whether the group tree drawer is open on narrow/mobile layouts. */
  let mobileNavOpen = $state(false);

  $effect(() => {
    // Subscribing (instead of one untracked get()) keeps external settings
    // edits applying immediately, mirroring the previous page-local behavior.
    const unsubscribe = appSettings.subscribe((s) => {
      groupWidth = s.general.panelWidths.group;
      detailWidth = s.general.panelWidths.detail;
      showDetailOnSelect = s.general.showDetailOnSelect ?? true;
    });
    return unsubscribe;
  });

  $effect(() => {
    const entry = options.selectedEntry();
    // also track showDetailOnSelect so toggling the setting re-evaluates
    const autoShow = showDetailOnSelect;
    // track version to re-open same entry after detail was closed
    const version = options.selectionVersion?.() ?? 0;
    void version;
    if (entry) {
      if (suppressDetailAutoOpen) {
        suppressDetailAutoOpen = false;
      } else {
        detailVisible = autoShow;
      }
    } else {
      suppressDetailAutoOpen = false;
      detailVisible = false;
    }
  });

  function beginColumnDrag(
    event: PointerEvent,
    /** Map `(width at drag start, pointer delta)` to the new width. */
    compute: (startWidth: number, deltaX: number) => number,
    read: () => number,
    write: (value: number) => void,
  ): void {
    event.preventDefault();
    event.stopPropagation();
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const startWidth = read();
    document.body.classList.add("resizing-column");
    const onMove = (ev: PointerEvent): void => {
      write(compute(startWidth, ev.clientX - startX));
    };
    const onUp = (ev: PointerEvent): void => {
      if (target.hasPointerCapture(ev.pointerId)) target.releasePointerCapture(ev.pointerId);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      document.body.classList.remove("resizing-column");
      saveLayout();
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function startDetailResize(event: PointerEvent): void {
    beginColumnDrag(
      event,
      (startWidth, deltaX) => Math.min(640, Math.max(260, startWidth - deltaX)),
      () => detailWidth,
      (value) => (detailWidth = value),
    );
  }

  function startGroupResize(event: PointerEvent): void {
    beginColumnDrag(
      event,
      (startWidth, deltaX) => Math.min(320, Math.max(140, startWidth + deltaX)),
      () => groupWidth,
      (value) => (groupWidth = value),
    );
  }

  function saveLayout(): void {
    // Capture fresh values before any store write: the `appSettings`
    // subscription that mirrors `panelWidths`/`entryColumns` back into
    // local `$state` runs synchronously on each `updateGeneral`.
    // Writing `entryColumns` first would reset `groupWidth`/`detailWidth`
    // to the stale in-store `panelWidths` before the second write reads
    // them, reverting a panel drag on release (see PITFALLS).
    const freshColumns = options.entryColumns().map((c) => ({ ...c }));
    const freshGroup = groupWidth;
    const freshDetail = detailWidth;
    const freshUrlCol = options.urlColWidth();
    // Order still matches the column-drag hazard: entryColumns first so the
    // column mirror cannot clobber freshly dragged widths before they are
    // persisted; panel data uses the captured snapshot.
    appSettings.updateGeneral("entryColumns", freshColumns);
    appSettings.updateGeneral("panelWidths", {
      group: freshGroup,
      detail: freshDetail,
      urlCol: freshUrlCol,
    });
  }

  return {
    get groupWidth() {
      return groupWidth;
    },
    set groupWidth(value) {
      groupWidth = value;
    },
    get detailWidth() {
      return detailWidth;
    },
    set detailWidth(value) {
      detailWidth = value;
    },
    get detailVisible() {
      return detailVisible;
    },
    set detailVisible(value) {
      detailVisible = value;
    },
    get mobileNavOpen() {
      return mobileNavOpen;
    },
    set mobileNavOpen(value) {
      mobileNavOpen = value;
    },
    suppressNextAutoOpen: () => {
      suppressDetailAutoOpen = true;
    },
    startDetailResize,
    startGroupResize,
    saveLayout,
  };
}
