/** Human-readable byte size (B / KB / MB). */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/** Entry "Size" column text, mirroring the KeePass official client's
 *  `StrUtil.FormatDataSize`: ceiling to whole KB/MB/GB/TB units so the same
 *  record reads identically in both clients ("0 KB" / "1 KB" / "5 KB"…). */
export function formatKeePassSize(bytes: number): string {
  const KB = 1024;
  const MB = KB * KB;
  const GB = MB * KB;
  const TB = GB * KB;
  if (bytes === 0) return "0 KB";
  if (bytes <= KB) return "1 KB";
  if (bytes <= MB) return `${Math.floor((bytes - 1) / KB) + 1} KB`;
  if (bytes <= GB) return `${Math.floor((bytes - 1) / MB) + 1} MB`;
  if (bytes <= TB) return `${Math.floor((bytes - 1) / GB) + 1} GB`;
  return `${Math.floor((bytes - 1) / TB) + 1} TB`;
}

/** Compact entry-description string: the URL with its scheme stripped.
 *  Only shown when a URL exists; otherwise empty. */
export function formatEntryDescription(entry: { url?: string }): string {
  const url = entry.url?.trim();
  return url ? url.replace(/^[a-zA-Z][a-zA-Z0-9+.-]*:\/\//, "") : "";
}
