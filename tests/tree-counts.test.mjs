import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { buildEntryCounts, totalExcludingBin } from "../src/lib/utils/tree.ts";

const leafEntry = (uuid) => ({ uuid });

function group(uuid, entries = [], children = [], extra = {}) {
  return { uuid, entries, children, ...extra };
}

function vault() {
  const bin = group("bin", [leafEntry("b1"), leafEntry("b2")], [], { isRecycleBin: true });
  const child = group("g1", [leafEntry("e2")]);
  // One entry lives directly under the root (outside any subgroup).
  return group("root", [leafEntry("e1")], [child, bin]);
}

describe("buildEntryCounts", () => {
  it("counts root-direct entries plus every subtree", () => {
    const counts = buildEntryCounts(vault());
    assert.equal(counts.get("root"), 4);
    assert.equal(counts.get("g1"), 1);
    assert.equal(counts.get("bin"), 2);
  });
});

describe("totalExcludingBin", () => {
  it("includes root-direct entries and excludes the bin", () => {
    const counts = buildEntryCounts(vault());
    assert.equal(totalExcludingBin(counts, "root", "bin"), 2);
  });

  it("counts everything when no bin exists", () => {
    const counts = buildEntryCounts(
      group("root", [leafEntry("e1")], [group("g1", [leafEntry("e2")])]),
    );
    assert.equal(totalExcludingBin(counts, "root", null), 2);
  });

  it("clamps inconsistent snapshots at zero instead of going negative", () => {
    assert.equal(totalExcludingBin(new Map([["root", 1]]), "root", "bin"), 1);
    assert.equal(totalExcludingBin(new Map(), "root", "bin"), 0);
    assert.equal(totalExcludingBin(new Map([["bin", 5]]), "root", "bin"), 0);
  });
});
