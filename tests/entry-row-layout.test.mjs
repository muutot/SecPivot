import assert from "node:assert/strict";
import test from "node:test";
import {
  buildEntryRowOffsets,
  entryRowAtOffset,
  entryRowRange,
} from "../src/lib/utils/entry-row-layout.ts";

test("directory separators are half-height while entries retain configured density", () => {
  const rows = [
    { kind: "group" },
    { kind: "entry" },
    { kind: "entry" },
    { kind: "group" },
    { kind: "entry" },
  ];
  assert.deepEqual(buildEntryRowOffsets(rows, 30), [0, 15, 45, 75, 90, 120]);
  assert.deepEqual(buildEntryRowOffsets(rows, 36), [0, 18, 54, 90, 108, 144]);
  assert.deepEqual(buildEntryRowOffsets(rows, 20), [0, 14, 34, 54, 68, 88]);
});
test("mixed-height virtual windows cover actual visible rows and exact boundaries", () => {
  const offsets = [0, 15, 45, 75, 90, 120];
  assert.equal(entryRowAtOffset(offsets, 14), 0);
  assert.equal(entryRowAtOffset(offsets, 15), 1);
  assert.deepEqual(entryRowRange(offsets, 0, 45, 0), { start: 0, end: 2 });
  assert.deepEqual(entryRowRange(offsets, 15, 45, 0), { start: 1, end: 3 });
  assert.deepEqual(entryRowRange(offsets, 1000, 30, 0), { start: 4, end: 5 });
  assert.deepEqual(entryRowRange([0], 0, 30), { start: 0, end: 0 });
  assert.deepEqual(entryRowRange(offsets, 75, 30, 1), { start: 2, end: 5 });
});
test("many separators do not accumulate gaps or drop the final entry from the window", () => {
  const rows = Array.from({ length: 1000 }, () => [{ kind: "group" }, { kind: "entry" }]).flat();
  const offsets = buildEntryRowOffsets(rows, 30);
  assert.equal(offsets.at(-1), 45000);
  const range = entryRowRange(offsets, 44850, 150, 6);
  assert.equal(range.end, rows.length);
  assert.ok(range.start <= 1992);
  assert.equal(
    offsets[range.start] +
      (offsets[range.end] - offsets[range.start]) +
      (offsets.at(-1) - offsets[range.end]),
    45000,
  );
});
