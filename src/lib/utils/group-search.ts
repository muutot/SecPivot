import type { VaultGroup } from "$lib/types/vault";

/** Update only demo search metadata. Native snapshots already resolve the
 * complete tree. Legacy demo booleans are retained as explicit settings because
 * their original inheritance state cannot be reconstructed. */
export function resolveDemoGroupSearching(group: VaultGroup, inherited = true): void {
  if (group.searchingSetting === undefined) group.searchingSetting = group.enableSearching;
  group.enableSearching = group.searchingSetting ?? inherited;
  for (const child of group.children) resolveDemoGroupSearching(child, group.enableSearching);
}
