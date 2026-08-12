import type { VaultEntry } from "$lib/types/vault";

export type SearchFieldScope = "all" | "title" | "username" | "url" | "notes" | "tags" | "custom";

export interface AdvancedSearchQuery {
  /** Text to match; empty disables the text rule. */
  text: string;
  field: SearchFieldScope;
  /** Treat `text` as a regular expression (invalid regex matches nothing). */
  regex: boolean;
  /** Invert the text rule (exclude matching entries). */
  exclude: boolean;
  /** Keep only expired entries. */
  onlyExpired?: boolean;
  /** Keep only favorites. */
  onlyFavorites?: boolean;
  /** Require every listed tag (comma/space separated) to be present. */
  tags?: string;
  /** When true, entries with `qualityCheck === false` are excluded. */
  requireQualityCheck?: boolean;
}

export function matchesAdvancedSearch(entry: VaultEntry, query: AdvancedSearchQuery): boolean {
  if (query.onlyExpired && !entry.expired) return false;
  if (query.onlyFavorites && !entry.favorite) return false;
  if (query.requireQualityCheck && entry.qualityCheck === false) return false;

  const requiredTags = splitTags(query.tags);
  if (requiredTags.length > 0) {
    const entryTags = splitTags(entry.tags ?? "");
    if (!requiredTags.every((tag) => entryTags.includes(tag))) return false;
  }

  const text = query.text.trim();
  if (text.length === 0) return true;
  const fieldValue = entryFieldText(entry, query.field);
  let matched: boolean;
  if (query.regex) {
    try {
      matched = new RegExp(text, "i").test(fieldValue);
    } catch {
      matched = false;
    }
  } else {
    matched = fieldValue.toLowerCase().includes(text.toLowerCase());
  }
  return query.exclude ? !matched : matched;
}

function entryFieldText(entry: VaultEntry, scope: SearchFieldScope): string {
  switch (scope) {
    case "title":
      return entry.title ?? "";
    case "username":
      return entry.username ?? "";
    case "url":
      return entry.url ?? "";
    case "notes":
      return entry.notes ?? "";
    case "tags":
      return entry.tags ?? "";
    case "custom":
      return (entry.customFields ?? []).map((f) => `${f.name}:${f.value}`).join(" ");
    case "all":
    default:
      return [
        entry.title,
        entry.username,
        entry.url,
        entry.notes,
        entry.tags,
        ...(entry.customFields ?? []).map((f) => `${f.name}:${f.value}`),
      ]
        .filter(Boolean)
        .join(" ");
  }
}

function splitTags(value: string | undefined): string[] {
  return (value ?? "")
    .split(/[\s,，]+/)
    .map((tag) => tag.trim())
    .filter(Boolean);
}
