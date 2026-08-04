import type { VaultEntry, VaultGroup } from "$lib/types/vault";

/** Per-root lookup maps built lazily and invalidated by object identity, so
 *  repeated uuid lookups over a large tree (e.g. re-resolving the selection on
 *  every vault-state change) are O(1) instead of a full depth-first walk. */
const entryIndex = new WeakMap<VaultGroup, Map<string, VaultEntry>>();
const groupIndex = new WeakMap<VaultGroup, Map<string, VaultGroup>>();

function buildEntryIndex(root: VaultGroup): Map<string, VaultEntry> {
  const map = new Map<string, VaultEntry>();
  const visit = (group: VaultGroup): void => {
    for (const entry of group.entries) map.set(entry.uuid, entry);
    for (const child of group.children) visit(child);
  };
  visit(root);
  return map;
}

function buildGroupIndex(root: VaultGroup): Map<string, VaultGroup> {
  const map = new Map<string, VaultGroup>();
  const visit = (group: VaultGroup): void => {
    map.set(group.uuid, group);
    for (const child of group.children) visit(child);
  };
  visit(root);
  return map;
}

/** Search the entry index of `root`, scanning the tree only on first use. */
export function findEntryIn(root: VaultGroup, uuid: string): VaultEntry | null {
  let map = entryIndex.get(root);
  if (!map) {
    map = buildEntryIndex(root);
    entryIndex.set(root, map);
  }
  return map.get(uuid) ?? null;
}

/** Search the group index of `root`, scanning the tree only on first use. */
export function findGroupIn(root: VaultGroup, uuid: string): VaultGroup | null {
  let map = groupIndex.get(root);
  if (!map) {
    map = buildGroupIndex(root);
    groupIndex.set(root, map);
  }
  return map.get(uuid) ?? null;
}

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
