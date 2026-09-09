import type { VaultEntry } from "$lib/types/vault";
import type { VaultTreeIndex } from "$lib/utils/tree";

/** A rendered row of the entry list: either a group-separator header or an entry. */
export type DisplayRow =
  { kind: "group"; id: string; label: string } | { kind: "entry"; entry: VaultEntry };

/** Minimal shape the builder reads from the page's sorted rows. */
export interface SortedEntryRow {
  entry: VaultEntry;
}

/** Result of {@link buildDisplayRows}: the rendered rows plus the entry-only
 *  total, counted in the same single pass (no extra filter over `rows`). */
export interface BuiltDisplayRows {
  rows: DisplayRow[];
  entryCount: number;
}

/** Build the entry-list rows from page-sorted entries: flat entry rows unless
 *  several groups are present and separators are enabled, in which case each
 *  contiguous group block is preceded by a separator header labeled with the
 *  group's display path. Pure: reads only its arguments.
 *
 *  Key contract: a separator's `id` is its group uuid, used verbatim as the
 *  keyed-`each` key (`g-${id}`), so callers must pass DFS-blocked rows with
 *  one block per group — the page's per-group sort guarantees this, keeping
 *  separator keys both unique and stable across virtualization shifts. */
export function buildDisplayRows(
  sortedEntries: ReadonlyArray<SortedEntryRow>,
  treeIndex: VaultTreeIndex | null | undefined,
  showGroupSeparators: boolean,
): BuiltDisplayRows {
  if (sortedEntries.length === 0 || !treeIndex || !showGroupSeparators) {
    return {
      rows: sortedEntries.map((r) => ({ kind: "entry" as const, entry: r.entry })),
      entryCount: sortedEntries.length,
    };
  }
  const distinct = new Set(sortedEntries.map((r) => r.entry.groupUuid));
  if (distinct.size <= 1) {
    return {
      rows: sortedEntries.map((r) => ({ kind: "entry" as const, entry: r.entry })),
      entryCount: sortedEntries.length,
    };
  }
  const rows: DisplayRow[] = [];
  let entryCount = 0;
  let cur: string | null = null;
  for (const row of sortedEntries) {
    const g = row.entry.groupUuid;
    if (g !== cur) {
      cur = g;
      const grp = treeIndex.groupByUuid.get(g);
      const path = treeIndex.pathByGroupUuid.get(g) ?? grp?.name ?? g;
      const raw = path || grp?.name || g;
      const label = raw.replaceAll(" / ", " → ");
      rows.push({ kind: "group", id: g, label });
    }
    rows.push({ kind: "entry", entry: row.entry });
    entryCount += 1;
  }
  return { rows, entryCount };
}
