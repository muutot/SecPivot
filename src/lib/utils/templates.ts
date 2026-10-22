import type { VaultEntry, VaultGroup } from "$lib/types/vault";

/**
 * Entries offered as new-entry templates: the direct children of the
 * database's configured entry-templates group (KeePass convention — nested
 * groups are not expanded). Returns `[]` when no templates group is set or
 * the group does not exist in the tree.
 */
export function collectTemplateEntries(
  root: VaultGroup | null | undefined,
  templatesGroupUuid: string | null | undefined,
): VaultEntry[] {
  if (!root || !templatesGroupUuid) return [];
  const group = findGroup(root, templatesGroupUuid);
  return group ? group.entries : [];
}

function findGroup(group: VaultGroup, uuid: string): VaultGroup | null {
  if (group.uuid === uuid) return group;
  for (const child of group.children) {
    const found = findGroup(child, uuid);
    if (found) return found;
  }
  return null;
}
