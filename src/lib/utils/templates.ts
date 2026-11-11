import type { CustomField, VaultEntry, VaultGroup, WebsiteRuleSet } from "$lib/types/vault";

/** Resolve every hidden value before a template can replace the editor draft.
 * A missing protected field/OTP is a failed read, never an empty default. */
export async function resolveTemplateSecrets(
  entry: VaultEntry,
  readers: {
    password: () => Promise<string>;
    totp: () => Promise<string | null>;
    field: (name: string) => Promise<string | null>;
    websiteRules?: () => Promise<WebsiteRuleSet | null>;
  },
): Promise<{
  password: string;
  totp: string;
  customFields: CustomField[];
  websiteRules: WebsiteRuleSet | null;
}> {
  const [password, totp, customFields, websiteRules] = await Promise.all([
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
    entry.hasWebsiteRules
      ? Promise.resolve().then(async () => {
          if (!readers.websiteRules) throw new Error("Template website rules reader unavailable");
          const value = await readers.websiteRules();
          if (value === null) throw new Error("Template website rules unavailable");
          return value;
        })
      : Promise.resolve(null),
  ]);
  return { password, totp, customFields, websiteRules };
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
