import { describe, it } from "node:test";
import assert from "node:assert/strict";

// Entry templates come from the direct children of the database's configured
// entry-templates group (KeePass convention). Nested groups are not expanded,
// and an unset or dangling group uuid yields no templates so the editor hides
// its selector.
const { collectTemplateEntries, resolveTemplateSecrets } =
  await import("../src/lib/utils/templates.ts");

it("resolves all template secrets atomically without modifying its snapshot", async () => {
  const template = {
    ...entry("t"),
    hasTotp: true,
    customFields: [
      { name: "public", value: "visible" },
      { name: "hidden", value: "", protected: true },
    ],
  };
  const result = await resolveTemplateSecrets(template, {
    password: async () => "synthetic password",
    totp: async () => "synthetic otp",
    field: async () => "synthetic field",
  });
  assert.equal(result.password, "synthetic password");
  assert.equal(result.totp, "synthetic otp");
  assert.equal(result.customFields[1].value, "synthetic field");
  assert.equal(template.customFields[1].value, "");
});

it("rejects partial template reads, including absent protected values", async () => {
  const template = {
    ...entry("t"),
    hasTotp: true,
    customFields: [{ name: "hidden", value: "", protected: true }],
  };
  const readers = {
    password: async () => "synthetic",
    totp: async () => "otp",
    field: async () => "field",
  };
  await assert.rejects(
    resolveTemplateSecrets(template, {
      ...readers,
      password: async () => {
        throw new Error("synthetic failure");
      },
    }),
  );
  await assert.rejects(resolveTemplateSecrets(template, { ...readers, field: async () => null }));
  await assert.rejects(resolveTemplateSecrets(template, { ...readers, totp: async () => null }));
});

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
