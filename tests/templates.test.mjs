import { describe, it } from "node:test";
import assert from "node:assert/strict";

// Entry templates come from the direct children of the database's configured
// entry-templates group (KeePass convention). Nested groups are not expanded,
// and an unset or dangling group uuid yields no templates so the editor hides
// its selector.
const { collectTemplateEntries } = await import("../src/lib/utils/templates.ts");

function group(uuid, entries = [], children = []) {
  return { uuid, name: uuid, isRecycleBin: false, isExpanded: true, entries, children };
}

function entry(uuid) {
  return { uuid, groupUuid: "parent", title: `entry-${uuid}` };
}

describe("collectTemplateEntries", () => {
  it("returns the direct entries of the configured templates group", () => {
    const root = group("root", [entry("top")], [group("tpl", [entry("t1"), entry("t2")])]);
    assert.deepEqual(
      collectTemplateEntries(root, "tpl").map((e) => e.uuid),
      ["t1", "t2"],
    );
  });

  it("finds the templates group at any depth", () => {
    const root = group("root", [], [group("a", [], [group("tpl", [entry("deep")])])]);
    assert.deepEqual(
      collectTemplateEntries(root, "tpl").map((e) => e.uuid),
      ["deep"],
    );
  });

  it("returns no nested-group entries (only direct children)", () => {
    const root = group(
      "root",
      [],
      [group("tpl", [entry("direct")], [group("sub", [entry("nested")])])],
    );
    assert.deepEqual(
      collectTemplateEntries(root, "tpl").map((e) => e.uuid),
      ["direct"],
    );
  });

  it("returns an empty list when no templates group is set", () => {
    const root = group("root", [], [group("tpl", [entry("t1")])]);
    assert.deepEqual(collectTemplateEntries(root, undefined), []);
    assert.deepEqual(collectTemplateEntries(root, null), []);
  });

  it("returns an empty list for a dangling group uuid", () => {
    const root = group("root", [], [group("tpl", [entry("t1")])]);
    assert.deepEqual(collectTemplateEntries(root, "gone"), []);
  });

  it("returns an empty list without a vault state", () => {
    assert.deepEqual(collectTemplateEntries(null, "tpl"), []);
    assert.deepEqual(collectTemplateEntries(undefined, "tpl"), []);
  });
});
