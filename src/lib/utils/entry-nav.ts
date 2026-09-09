/** Row navigation for the windowed entry table.
 *
 * Group-separator rows are visual only: every walker below skips them so
 * keyboard focus always lands on an entry row. The page-step helpers clamp
 * both the probe and the fallback start to `[0, rows.length - 1]` — a
 * negative or over-long start would otherwise fail every bounds check and
 * return `null`, silently swallowing PageUp/PageDown (the PageUp case fired
 * whenever the first row was a group separator and `index - pageStep < 0`).
 */

export interface NavigableRow {
  kind: "group" | "entry";
}

export function isEntryRow(row: NavigableRow | undefined): boolean {
  return !!row && row.kind === "entry";
}

export function findNextEntryIndex(
  rows: ReadonlyArray<NavigableRow | undefined>,
  from: number,
  step: 1 | -1,
): number | null {
  let idx = from;
  while (idx >= 0 && idx < rows.length) {
    if (isEntryRow(rows[idx])) return idx;
    idx += step;
  }
  return null;
}

export function findNearestEntryIndex(
  rows: ReadonlyArray<NavigableRow | undefined>,
  target: number,
  direction: 1 | -1,
): number | null {
  if (target < 0 || target >= rows.length) return null;
  if (isEntryRow(rows[target])) return target;
  // header: scan in direction, fallback to opposite
  let idx = target + direction;
  while (idx >= 0 && idx < rows.length) {
    if (isEntryRow(rows[idx])) return idx;
    idx += direction;
  }
  idx = target - direction;
  while (idx >= 0 && idx < rows.length) {
    if (isEntryRow(rows[idx])) return idx;
    idx -= direction;
  }
  return null;
}

/** PageUp target tolerant of a leading group separator. */
export function pageUpTargetIndex(
  rows: ReadonlyArray<NavigableRow | undefined>,
  index: number,
  pageStep: number,
): number | null {
  if (rows.length === 0) return null;
  const start = Math.max(0, index - pageStep);
  return findNearestEntryIndex(rows, start, -1) ?? findNextEntryIndex(rows, start, 1);
}

/** PageDown target tolerant of a trailing group separator. */
export function pageDownTargetIndex(
  rows: ReadonlyArray<NavigableRow | undefined>,
  index: number,
  pageStep: number,
): number | null {
  if (rows.length === 0) return null;
  const start = Math.min(rows.length - 1, index + pageStep);
  return findNearestEntryIndex(rows, start, 1) ?? findNextEntryIndex(rows, start, -1);
}
