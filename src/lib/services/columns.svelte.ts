//! Entry-table column configuration: persisted visibility/width/order state,
//! built-in + custom-field column resolution, grid template, memoized sort
//! keys, and cell display text. Extracted from `+page.svelte` so the table
//! concerns live in one owned unit; the page supplies the entry snapshot via
//! a closure and keeps selection/sorting direction itself.

import { get } from "svelte/store";
import { appSettings } from "$lib/services/settings";
import type { EntryColumnState } from "$lib/types/settings";
import type { EntryTableColumn } from "$lib/components/EntryTable.svelte";
import { formatDateOnly } from "$lib/utils/date";
import { formatKeePassSize } from "$lib/utils/format";
import type { VaultEntry } from "$lib/types/vault";

/** Title column width when `width` is `0` (auto sentinel, see settings.ts). */
export const COL_TITLE_DEFAULT = 200;

/** Built-in entry-table columns, in default display order. */
export const BUILTIN_COLUMNS: { id: string; label: string; sortable?: boolean }[] = [
  { id: "title", label: "标题" },
  { id: "username", label: "用户名" },
  { id: "password", label: "密码", sortable: false },
  { id: "url", label: "网址" },
  { id: "totp", label: "验证码", sortable: false },
  { id: "notes", label: "备注" },
  { id: "tags", label: "标签" },
  { id: "created", label: "创建时间" },
  { id: "modified", label: "修改时间" },
  { id: "expires", label: "过期时间" },
  { id: "size", label: "大小" },
];

export type EntryColumns = {
  /** Persisted column state array (display order; drag to reorder). */
  readonly entryColumns: EntryColumnState[];
  customColumnNames: string[];
  visibleCols: EntryTableColumn[];
  entryGridCols: string;
  colState(id: string): EntryColumnState;
  toggleColumn(id: string): void;
  resizeEntryColumn(colId: string, width: number): void;
  applyColumnReorder(colId: string, toIndex: number): void;
  sortKeyFor(entry: VaultEntry, col: string): string;
  columnText(entry: VaultEntry, colId: string): string;
};

export function useEntryColumns(allEntries: () => VaultEntry[]): EntryColumns {
  let entryColumns = $state<EntryColumnState[]>([]);
  $effect(() => {
    // `get(appSettings)` inside an effect is untracked and would freeze the
    // mirror on the initial value; subscribe instead so edits in the settings
    // window re-apply immediately.
    const unsubscribe = appSettings.subscribe((s) => {
      entryColumns = s.general.entryColumns.map((c) => ({ ...c }));
    });
    return unsubscribe;
  });

  /** Custom-field column names present in the vault, most frequent first. */
  const customColumnNames = $derived.by(() => {
    const names = new Map<string, number>();
    for (const e of allEntries()) {
      for (const f of e.customFields ?? []) {
        names.set(f.name, (names.get(f.name) ?? 0) + 1);
      }
    }
    return [...names.entries()].sort((a, b) => b[1] - a[1]).map(([name]) => name);
  });

  /** Persisted state of a column id (built-in or `custom:<name>`). */
  function colState(id: string): EntryColumnState {
    return (
      entryColumns.find((c) => c.id === id) ?? {
        id,
        visible: false,
        width: id === "title" ? 0 : 140,
      }
    );
  }

  /** Visible columns in render order (icon/actions columns excluded). The
   *  `entryColumns` array order is the persisted display order (drag to
   *  reorder); built-ins/customs not yet present in the array are appended in
   *  default order as a legacy fallback. */
  const visibleCols = $derived.by(() => {
    const out: EntryTableColumn[] = [];
    const seen = new Set<string>();
    for (const st of entryColumns) {
      if (!st.visible || seen.has(st.id)) continue;
      seen.add(st.id);
      const def = BUILTIN_COLUMNS.find((b) => b.id === st.id);
      if (def) {
        out.push({
          id: def.id,
          label: def.label,
          width: st.width,
          sortable: def.sortable !== false,
        });
      } else if (st.id.startsWith("custom:")) {
        out.push({
          id: st.id,
          label: st.id.slice("custom:".length),
          width: st.width,
          sortable: true,
        });
      }
    }
    for (const def of BUILTIN_COLUMNS) {
      if (seen.has(def.id)) continue;
      const st = colState(def.id);
      if (st.visible) {
        seen.add(def.id);
        out.push({
          id: def.id,
          label: def.label,
          width: st.width,
          sortable: def.sortable !== false,
        });
      }
    }
    for (const name of customColumnNames) {
      const id = `custom:${name}`;
      if (seen.has(id)) continue;
      const st = colState(id);
      if (st.visible) {
        seen.add(id);
        out.push({ id, label: name, width: st.width, sortable: true });
      }
    }
    return out;
  });

  /** CSS grid template for the entry table (icon + visible columns + actions). */
  const entryGridCols = $derived.by(() => {
    const cols = ["34px"];
    for (const c of visibleCols) {
      const w = c.id === "title" && c.width <= 0 ? COL_TITLE_DEFAULT : c.width;
      cols.push(`${w}px`);
    }
    cols.push("70px");
    return cols.join(" ");
  });

  function resizeEntryColumn(colId: string, width: number): void {
    entryColumns = entryColumns.map((column) =>
      column.id === colId ? { ...column, width } : column,
    );
  }

  function toggleColumn(id: string): void {
    const existing = entryColumns.find((c) => c.id === id);
    if (existing) {
      entryColumns = entryColumns.map((c) => (c.id === id ? { ...c, visible: !c.visible } : c));
    } else {
      entryColumns = [...entryColumns, { id, visible: true, width: id === "title" ? 0 : 140 }];
    }
    appSettings.updateGeneral(
      "entryColumns",
      entryColumns.map((c) => ({ ...c })),
    );
  }

  /** Move a column so it renders at the given insertion index of the visible
   *  order. Hidden columns keep their relative array positions. */
  function applyColumnReorder(colId: string, toIndex: number): void {
    const cols = entryColumns.map((c) => ({ ...c }));
    const from = cols.findIndex((c) => c.id === colId);
    if (from === -1) return;
    // Dropping before itself or the column right after it is a no-op.
    if (toIndex === from || toIndex === from + 1) return;
    const [moved] = cols.splice(from, 1);
    const anchorId = toIndex < visibleCols.length ? visibleCols[toIndex].id : null;
    if (anchorId === null) {
      cols.push(moved);
    } else {
      const anchor = cols.findIndex((c) => c.id === anchorId);
      cols.splice(anchor === -1 ? cols.length : anchor, 0, moved);
    }
    entryColumns = cols;
  }

  /** Display text of an entry cell for a column id. */
  function columnText(entry: VaultEntry, colId: string): string {
    if (colId.startsWith("custom:")) {
      const name = colId.slice("custom:".length);
      const field = entry.customFields?.find((f) => f.name === name);
      if (!field) return "";
      // Protected values never reach the snapshot; show the masked marker.
      return field.protected ? "••••••" : field.value;
    }
    if (colId === "created" || colId === "modified" || colId === "expires") {
      return formatDateOnly(entry[colId] as string | undefined);
    }
    if (colId === "password") return entry.hasPassword || entry.password ? "••••••" : "";
    if (colId === "size") return entry.size == null ? "" : formatKeePassSize(entry.size);
    return String(entry[colId as keyof VaultEntry] ?? "");
  }

  /** Sort key of an entry for a column id (password is never sortable). */
  function sortValue(entry: VaultEntry, colId: string): string {
    if (colId === "totp") return entry.hasTotp ? "1" : "0";
    // Size sorts numerically: zero-pad the byte count so lexicographic
    // string comparison over the memoized sort keys matches numeric order.
    if (colId === "size") return String(entry.size ?? 0).padStart(15, "0");
    return columnText(entry, colId);
  }

  /** Entries are immutable within a vault snapshot, so sort keys are memoized
   *  per (entry, column) exactly like the search-text cache: recomputing them
   *  for every filtered row on each search keystroke is pure waste. Replaced
   *  snapshots release the old entries naturally (WeakMap). */
  const entrySortKeyCache = new WeakMap<VaultEntry, Map<string, string>>();
  function sortKeyFor(entry: VaultEntry, col: string): string {
    let perCol = entrySortKeyCache.get(entry);
    if (!perCol) {
      perCol = new Map();
      entrySortKeyCache.set(entry, perCol);
    }
    const cached = perCol.get(col);
    if (cached !== undefined) return cached;
    const key = sortValue(entry, col);
    perCol.set(col, key);
    return key;
  }

  return {
    get entryColumns() {
      return entryColumns;
    },
    customColumnNames,
    visibleCols,
    entryGridCols,
    colState,
    toggleColumn,
    resizeEntryColumn,
    applyColumnReorder,
    sortKeyFor,
    columnText,
  };
}
