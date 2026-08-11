import type { VaultEntry, VaultGroup } from "$lib/types/vault";

export interface VaultTreeIndex {
  groups: VaultGroup[];
  entries: VaultEntry[];
  groupByUuid: Map<string, VaultGroup>;
  entryByUuid: Map<string, VaultEntry>;
  pathByGroupUuid: Map<string, string>;
  recycleBinUuids: Set<string>;
}

export type GroupPathIndex = Map<string, Map<string, string>>;

/** Per-root structural index built lazily and invalidated by object identity.
 *  A single depth-first walk supplies lookup maps, display paths, recycle-bin
 *  membership, and flat render lists so consumers do not rescan the same vault
 *  tree for each derived value or search keystroke. */
const vaultTreeIndexes = new WeakMap<VaultGroup, VaultTreeIndex>();

export function buildVaultTreeIndex(root: VaultGroup): VaultTreeIndex {
  const cached = vaultTreeIndexes.get(root);
  if (cached) return cached;

  const groups: VaultGroup[] = [];
  const entries: VaultEntry[] = [];
  const groupByUuid = new Map<string, VaultGroup>();
  const entryByUuid = new Map<string, VaultEntry>();
  const pathByGroupUuid = new Map<string, string>();
  const recycleBinUuids = new Set<string>();

  const visit = (
    group: VaultGroup,
    parent: VaultGroup | null,
    parentPath: string,
    parentInRecycleBin: boolean,
  ): void => {
    const path = parent ? (parentPath ? `${parentPath} / ${group.name}` : group.name) : "";
    const inRecycleBin = parentInRecycleBin || group.isRecycleBin;

    groups.push(group);
    groupByUuid.set(group.uuid, group);
    pathByGroupUuid.set(group.uuid, path);
    if (inRecycleBin) recycleBinUuids.add(group.uuid);

    for (const entry of group.entries) {
      entries.push(entry);
      entryByUuid.set(entry.uuid, entry);
    }
    for (const child of group.children) visit(child, group, path, inRecycleBin);
  };

  visit(root, null, "", false);
  const index = {
    groups,
    entries,
    groupByUuid,
    entryByUuid,
    pathByGroupUuid,
    recycleBinUuids,
  };
  vaultTreeIndexes.set(root, index);
  return index;
}

/** Search the entry index of `root`, scanning the tree only on first use. */
export function findEntryIn(root: VaultGroup, uuid: string): VaultEntry | null {
  return buildVaultTreeIndex(root).entryByUuid.get(uuid) ?? null;
}

/** Search the group index of `root`, scanning the tree only on first use. */
export function findGroupIn(root: VaultGroup, uuid: string): VaultGroup | null {
  return buildVaultTreeIndex(root).groupByUuid.get(uuid) ?? null;
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

/** Direct-child lookup used while resolving imported `A / B` group paths.
 *  Building it once avoids rescanning the whole vault for every path segment.
 *  Duplicate sibling names preserve the first depth-first match, matching the
 *  previous `collectGroups(...).find(...)` behavior. */
export function buildGroupPathIndex(root: VaultGroup): GroupPathIndex {
  const index: GroupPathIndex = new Map();
  walkGroups(root, (group) => {
    if (group.uuid === root.uuid) return;
    const parentUuid = group.parentUuid ?? root.uuid;
    let children = index.get(parentUuid);
    if (!children) {
      children = new Map();
      index.set(parentUuid, children);
    }
    if (!children.has(group.name)) children.set(group.name, group.uuid);
  });
  return index;
}

/** Atomically update several group expansion flags in a browser snapshot.
 *  Every uuid is resolved before the first write so an unknown group leaves
 *  the draft untouched, matching the backend batch command. */
export function setGroupsExpandedInTree(
  root: VaultGroup,
  uuids: readonly string[],
  expanded: boolean,
): void {
  const groupsByUuid = buildVaultTreeIndex(root).groupByUuid;
  const groups = uuids.map((uuid) => {
    const group = groupsByUuid.get(uuid);
    if (!group || group.uuid === root.uuid) throw new Error("group not found");
    return group;
  });
  for (const group of groups) group.isExpanded = expanded;
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

/** Subtree entry counts keyed by group uuid, computed bottom-up in one walk
 *  (the O(N) counterpart to calling `countEntries` per node, which is O(N²)). */
export function buildEntryCounts(root: VaultGroup): Map<string, number> {
  const counts = new Map<string, number>();
  const visit = (group: VaultGroup): number => {
    let total = group.entries.length;
    for (const child of group.children) total += visit(child);
    counts.set(group.uuid, total);
    return total;
  };
  visit(root);
  return counts;
}
