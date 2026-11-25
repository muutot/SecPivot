//! Search/filter pipeline for the main window's entry list: lazy search text
//! memoization per snapshot entry and group-subtree filtering with
//! advanced-query support. Sorting (keys via `columns.svelte.ts`, direction
//! state, favorite-first collation) stays page-side; this composable owns
//! only what turns a subtree + query into the filtered row list.
//!
//! Every input arrives as a getter closure so the `$derived` computation here
//! tracks the page's runes. Extracted from `+page.svelte`.

import type { AdvancedSearchQuery } from "$lib/utils/entry-search";
import { prepareAdvancedSearch } from "$lib/utils/entry-search";
import type { VaultEntry, VaultGroup, VaultState } from "$lib/types/vault";

export type EntryFilterOptions = {
  /** Active vault snapshot (or `null` when closed). */
  currentVault: () => VaultState | null;
  /** Groups whose entries contribute to the result (selected subtree). */
  selectedSubtree: () => VaultGroup[];
  /** Free-text query (trimmed downstream). */
  search: () => string;
  /** Structured advanced-search filter, or `null`. */
  advancedQuery: () => AdvancedSearchQuery | null;
};

export type EntryFilter = {
  readonly filteredEntries: { entry: VaultEntry }[];
};

export function useEntryFilter(options: EntryFilterOptions): EntryFilter {
  /** Entry objects are immutable within a vault snapshot, so search text can
   *  be normalized lazily on the first non-empty query and reused for each
   *  following keystroke. Replaced snapshots naturally release old entries. */
  const entrySearchTextCache = new WeakMap<VaultEntry, string>();
  function searchTextFor(entry: VaultEntry): string {
    const cached = entrySearchTextCache.get(entry);
    if (cached !== undefined) return cached;
    const text = [entry.title, entry.username, entry.url, entry.notes, entry.tags]
      .join(" ")
      .toLowerCase();
    entrySearchTextCache.set(entry, text);
    return text;
  }

  const filteredEntries = $derived.by((): { entry: VaultEntry }[] => {
    if (!options.currentVault()) return [];
    const query = options.search().trim().toLowerCase();
    const advancedQuery = options.advancedQuery();
    const searching = Boolean(query || advancedQuery);
    const subtree = options.selectedSubtree();
    const result: { entry: VaultEntry }[] = [];
    // Compile the advanced query once per derivation, not once per entry.
    const preparedAdvanced = advancedQuery ? prepareAdvancedSearch(advancedQuery) : null;
    for (const group of subtree) {
      // The snapshot resolves inheritance over the whole tree. Scope only
      // selects candidate groups and must not reinterpret explicit settings.
      if (searching && !group.enableSearching) continue;
      for (const entry of group.entries) {
        if (query && !searchTextFor(entry).includes(query)) continue;
        if (preparedAdvanced && !preparedAdvanced.test(entry)) continue;
        result.push({ entry });
      }
    }
    return result;
  });

  return {
    get filteredEntries() {
      return filteredEntries;
    },
  };
}
