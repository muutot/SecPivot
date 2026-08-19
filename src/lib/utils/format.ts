/** Human-readable byte size (B / KB / MB). */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Compact entry-description string: the URL with its scheme stripped.
 *  Falls back to the username when no URL is present. */
export function formatEntryDescription(entry: { url?: string; username?: string }): string {
  const url = entry.url?.trim();
  if (url) return url.replace(/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//, "");
  return entry.username || "";
}
