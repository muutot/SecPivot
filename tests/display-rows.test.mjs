import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { buildDisplayRows } from "../src/lib/utils/display-rows.ts";

const entry = (uuid, groupUuid) => ({ entry: { uuid, groupUuid } });

function treeIndex() {
  return {
    groupByUuid: new Map([
      ["g1", { name: "Alpha" }],
      ["g2", { name: "Beta" }],
    ]),
    pathByGroupUuid: new Map([
      ["g1", "Alpha"],
      ["g2", "Alpha / Beta"],
    ]),
  };
}

describe("buildDisplayRows", () => {
  it("returns no rows for empty input", () => {
    assert.deepEqual(buildDisplayRows([], treeIndex(), true), []);
  });

  it("stays flat without a tree index", () => {
    const rows = buildDisplayRows([entry("e1", "g1"), entry("e2", "g2")], null, true);
    assert.deepEqual(
      rows.map((r) => r.kind),
      ["entry", "entry"],
    );
  });

  it("stays flat when separators are disabled", () => {
    const rows = buildDisplayRows([entry("e1", "g1"), entry("e2", "g2")], treeIndex(), false);
    assert.deepEqual(
      rows.map((r) => r.kind),
      ["entry", "entry"],
    );
  });

  it("stays flat for a single distinct group", () => {
    const rows = buildDisplayRows([entry("e1", "g1"), entry("e2", "g1")], treeIndex(), true);
    assert.deepEqual(
      rows.map((r) => r.kind),
      ["entry", "entry"],
    );
  });

  it("precedes each group block with a path-labeled separator", () => {
    const rows = buildDisplayRows(
      [entry("e1", "g1"), entry("e2", "g2"), entry("e3", "g2")],
      treeIndex(),
      true,
    );
    assert.deepEqual(
      rows.map((r) => (r.kind === "group" ? `g:${r.label}` : `e:${r.entry.uuid}`)),
      ["g:Alpha", "e:e1", "g:Alpha → Beta", "e:e2", "e:e3"],
    );
    assert.equal(rows[0].id, "g1");
    assert.equal(rows[2].id, "g2");
  });

  it("emits a separator per contiguous block for interleaved groups", () => {
    const rows = buildDisplayRows(
      [entry("e1", "g1"), entry("e2", "g2"), entry("e3", "g1")],
      treeIndex(),
      true,
    );
    assert.deepEqual(
      rows.map((r) => (r.kind === "group" ? `g:${r.id}` : `e:${r.entry.uuid}`)),
      ["g:g1", "e:e1", "g:g2", "e:e2", "g:g1", "e:e3"],
    );
  });

  it("falls back to the raw uuid for unknown groups", () => {
    const rows = buildDisplayRows([entry("e1", "g1"), entry("e2", "gx")], treeIndex(), true);
    const separator = rows[2];
    assert.equal(separator.kind, "group");
    assert.equal(separator.id, "gx");
    assert.equal(separator.label, "gx");
  });
});
