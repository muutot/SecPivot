import type { VaultEntry, VaultGroup } from "$lib/types/vault";

/** Depth-first visit of every group in the tree (root first). */
export function walkGroups(root: VaultGroup, visit: (group: VaultGroup) => void): void {
  visit(root);
  for (const child of root.children) walkGroups(child, visit);
}

/** All groups in the tree, depth-first (root first). */
export function collectGroups(root: VaultGroup): VaultGroup[] {
  const out: VaultGroup[] = [];
  walkGroups(root, (g) => out.push(g));
  return out;
}

/** All entries in the tree, in pre-order group traversal order. */
export function collectEntries(root: VaultGroup): VaultEntry[] {
  const out: VaultEntry[] = [];
  walkGroups(root, (g) => out.push(...g.entries));
  return out;
}

/** Find a group by uuid anywhere in the tree. */
export function findGroup(root: VaultGroup, uuid: string): VaultGroup | null {
  if (root.uuid === uuid) return root;
  for (const child of root.children) {
    const found = findGroup(child, uuid);
    if (found) return found;
  }
  return null;
}

/** Find an entry by uuid anywhere in the tree. */
export function findEntry(root: VaultGroup, uuid: string): VaultEntry | null {
  for (const entry of root.entries) if (entry.uuid === uuid) return entry;
  for (const child of root.children) {
    const found = findEntry(child, uuid);
    if (found) return found;
  }
  return null;
}

/** Direct recycle-bin child of a group, if any. */
export function findBinGroup(root: VaultGroup): VaultGroup | null {
  return root.children.find((c) => c.isRecycleBin) ?? null;
}

/** Total entries in a subtree. */
export function countEntries(root: VaultGroup): number {
  let total = root.entries.length;
  for (const child of root.children) total += countEntries(child);
  return total;
}
