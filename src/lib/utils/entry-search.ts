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
  return prepareAdvancedSearch(query).test(entry);
}

/** Pre-compiled form of an advanced-search query: compile once, test many
 *  entries. `regex` compiles a single case-insensitive pattern (an invalid
 *  pattern matches nothing); tag lists and the lowered text are prepared up
 *  front so per-entry work stays proportional to the entry, not the query.
 *  Without this, every search keystroke recompiles the pattern once per
 *  entry — the dominant cost on large vaults now that sort keys and search
 *  text are both memoized. */
export interface PreparedAdvancedSearch {
  test(entry: VaultEntry): boolean;
}

export function prepareAdvancedSearch(query: AdvancedSearchQuery): PreparedAdvancedSearch {
  const requiredTags = splitTags(query.tags);
  const text = query.text.trim();
  let pattern: RegExp | null = null;
  let patternInvalid = false;
  if (query.regex && text.length > 0) {
    try {
      pattern = new RegExp(text, "i");
    } catch {
      patternInvalid = true;
    }
  }
  const loweredText = text.toLowerCase();
  return {
    test(entry: VaultEntry): boolean {
      if (query.onlyExpired && !entry.expired) return false;
      if (query.onlyFavorites && !entry.favorite) return false;
      if (query.requireQualityCheck && entry.qualityCheck === false) return false;
      if (requiredTags.length > 0) {
        const entryTags = splitTags(entry.tags ?? "");
        if (!requiredTags.every((tag) => entryTags.includes(tag))) return false;
      }
      if (text.length === 0) return true;
      if (patternInvalid) return query.exclude;
      const fieldValue = entryFieldText(entry, query.field);
      const matched = pattern
        ? pattern.test(fieldValue)
        : fieldValue.toLowerCase().includes(loweredText);
      return query.exclude ? !matched : matched;
    },
  };
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
