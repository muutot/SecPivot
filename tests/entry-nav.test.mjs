import { describe, it } from "node:test";
import assert from "node:assert/strict";

import {
  entryPositions,
  findNearestEntryIndex,
  findNextEntryIndex,
  pageDownTargetIndex,
  pageUpTargetIndex,
} from "../src/lib/utils/entry-nav.ts";

const group = (id) => ({ kind: "group", id, label: id });
const entry = (id) => ({ kind: "entry", uuid: id });

describe("findNextEntryIndex", () => {
  it("skips leading group separators for Home", () => {
    const rows = [group("g1"), entry("e1"), entry("e2")];
    assert.equal(findNextEntryIndex(rows, 0, 1), 1);
  });

  it("walks backward across separators for ArrowUp", () => {
    const rows = [entry("e1"), group("g2"), entry("e2")];
    assert.equal(findNextEntryIndex(rows, 1, -1), 0);
  });

  it("returns null when no entry exists in the walk direction", () => {
    assert.equal(findNextEntryIndex([group("g1")], 0, 1), null);
    assert.equal(findNextEntryIndex([entry("e1")], -1, -1), null);
    assert.equal(findNextEntryIndex([], 0, 1), null);
  });
});

describe("findNearestEntryIndex", () => {
  it("prefers the probed entry and scans both directions", () => {
    const rows = [entry("e1"), group("g2"), entry("e2")];
    assert.equal(findNearestEntryIndex(rows, 1, -1), 0);
    assert.equal(findNearestEntryIndex(rows, 1, 1), 2);
    assert.equal(findNearestEntryIndex(rows, 2, 1), 2);
  });

  it("returns null for out-of-range probes", () => {
    assert.equal(findNearestEntryIndex([entry("e1")], -1, 1), null);
    assert.equal(findNearestEntryIndex([entry("e1")], 1, -1), null);
  });
});

describe("pageUpTargetIndex", () => {
  it("lands on the first entry when a group separator leads the rows", () => {
    const rows = [group("g1"), entry("e1"), entry("e2"), group("g2"), entry("e3")];
    assert.equal(pageUpTargetIndex(rows, 1, 2), 1);
    assert.equal(pageUpTargetIndex(rows, 2, 4), 1);
  });

  it("moves up by roughly one page on separator-free rows", () => {
    const rows = Array.from({ length: 10 }, (_, i) => entry(`e${i}`));
    assert.equal(pageUpTargetIndex(rows, 9, 5), 4);
  });

  it("returns null only when no entry exists", () => {
    assert.equal(pageUpTargetIndex([], 0, 3), null);
    assert.equal(pageUpTargetIndex([group("g1")], 0, 3), null);
  });
});

describe("pageDownTargetIndex", () => {
  it("clamps past the end and lands on the last entry", () => {
    const rows = [entry("e1"), entry("e2"), entry("e3")];
    assert.equal(pageDownTargetIndex(rows, 0, 10), 2);
  });

  it("tolerates a trailing group separator", () => {
    const rows = [entry("e1"), group("g1")];
    assert.equal(pageDownTargetIndex(rows, 0, 5), 0);
  });

  it("returns null only when no entry exists", () => {
    assert.equal(pageDownTargetIndex([], 0, 3), null);
  });
});

describe("entryPositions", () => {
  const idOf = (row) => (row.kind === "entry" ? row.uuid : null);

  it("numbers entries 1-based while skipping separators", () => {
    const rows = [group("g1"), entry("e1"), entry("e2"), group("g2"), entry("e3")];
    const positions = entryPositions(rows, idOf);
    assert.equal(positions.get("e1"), 1);
    assert.equal(positions.get("e2"), 2);
    assert.equal(positions.get("e3"), 3);
    assert.equal(positions.size, 3);
  });

  it("is empty when no entries exist", () => {
    assert.equal(entryPositions([], idOf).size, 0);
    assert.equal(entryPositions([group("g1")], idOf).size, 0);
  });

  it("skips rows without an id", () => {
    const rows = [entry("e1"), { kind: "entry" }];
    const positions = entryPositions(rows, idOf);
    assert.equal(positions.get("e1"), 1);
    assert.equal(positions.size, 1);
  });
});
