import type { CustomField, VaultEntry, VaultGroup } from "$lib/types/vault";

/** Resolve every hidden value before a template can replace the editor draft.
 * A missing protected field/OTP is a failed read, never an empty default. */
export async function resolveTemplateSecrets(
  entry: VaultEntry,
  readers: {
    password: () => Promise<string>;
    totp: () => Promise<string | null>;
    field: (name: string) => Promise<string | null>;
  },
): Promise<{ password: string; totp: string; customFields: CustomField[] }> {
  const [password, totp, customFields] = await Promise.all([
    readers.password(),
    entry.hasTotp
      ? readers.totp().then((value) => {
          if (value === null) throw new Error("Template OTP is unavailable");
          return value;
        })
      : Promise.resolve(entry.totp ?? ""),
    Promise.all(
      (entry.customFields ?? []).map(async (field) => {
        if (!field.protected) return { ...field };
        const value = await readers.field(field.name);
        if (value === null) throw new Error("Template protected field is unavailable");
        return { ...field, value };
      }),
    ),
  ]);
  return { password, totp, customFields };
}

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
