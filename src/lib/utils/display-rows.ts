import type { VaultEntry } from "$lib/types/vault";
import type { VaultTreeIndex } from "$lib/utils/tree";

/** A rendered row of the entry list: either a group-separator header or an entry. */
export type DisplayRow =
  { kind: "group"; id: string; label: string } | { kind: "entry"; entry: VaultEntry };

/** Minimal shape the builder reads from the page's sorted rows. */
export interface SortedEntryRow {
  entry: VaultEntry;
}

/** Build the entry-list rows from page-sorted entries: flat entry rows unless
 *  several groups are present and separators are enabled, in which case each
 *  contiguous group block is preceded by a separator header labeled with the
 *  group's display path. Pure: reads only its arguments. */
export function buildDisplayRows(
  sortedEntries: ReadonlyArray<SortedEntryRow>,
  treeIndex: VaultTreeIndex | null | undefined,
  showGroupSeparators: boolean,
): DisplayRow[] {
  if (sortedEntries.length === 0 || !treeIndex) {
    return sortedEntries.map((r) => ({ kind: "entry" as const, entry: r.entry }));
  }
  if (!showGroupSeparators) {
    return sortedEntries.map((r) => ({ kind: "entry" as const, entry: r.entry }));
  }
  const distinct = new Set(sortedEntries.map((r) => r.entry.groupUuid));
  if (distinct.size <= 1) {
    return sortedEntries.map((r) => ({ kind: "entry" as const, entry: r.entry }));
  }
  const rows: DisplayRow[] = [];
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
  }
  return rows;
}
