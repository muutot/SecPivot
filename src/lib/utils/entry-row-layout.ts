import type { VirtualRange } from "$lib/utils/virtual-list";

/** Prefix offsets include a terminal total; separators use half the entry height. */
export function buildEntryRowOffsets(
  rows: ReadonlyArray<{ kind: string }>,
  entryHeight: number,
): number[] {
  const offsets = [0];
  const height = Math.max(1, entryHeight);
  const separatorHeight = Math.max(14, Math.round(height / 2));
  for (const row of rows)
    offsets.push(offsets[offsets.length - 1] + (row.kind === "group" ? separatorHeight : height));
  return offsets;
}

/** Find the physical row at a pixel offset in logarithmic time. */
export function entryRowAtOffset(offsets: ReadonlyArray<number>, pixel: number): number {
  let low = 0,
    high = offsets.length - 1;
  while (low < high) {
    const mid = Math.ceil((low + high) / 2);
    if (offsets[mid] <= pixel) low = mid;
    else high = mid - 1;
  }
  return Math.max(0, Math.min(offsets.length - 2, low));
}

export function entryRowRange(
  offsets: ReadonlyArray<number>,
  scrollTop: number,
  viewportHeight: number,
  overscan = 6,
): VirtualRange {
  const count = offsets.length - 1;
  if (!count) return { start: 0, end: 0 };
  const viewport = Math.max(1, viewportHeight);
  const top = Math.max(0, Math.min(scrollTop, offsets[count] - viewport));
  const first = entryRowAtOffset(offsets, top);
  const endPixel = top + viewport;
  const last = entryRowAtOffset(offsets, endPixel);
  const end = offsets[last] === endPixel ? last : last + 1;
  return { start: Math.max(0, first - overscan), end: Math.min(count, end + overscan) };
}
