import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
import {
  LinkedFieldsDraft,
  copyLinkedFields,
  linkedFieldValue,
} from "../src/lib/utils/linked-field-draft.ts";

const initial = () => ({
  version: 1,
  links: [
    { name: "alias", target: { kind: "field", name: "Password" } },
    {
      name: "full",
      target: { kind: "bitwardenFullName", fields: ["title", "first", "middle", "last"] },
    },
  ],
});

test("editor cannot freeze or rename unresolved values and detaches the current resolved value", () => {
  const source = readFileSync("src/lib/components/EntryEditorDialog.svelte", "utf8");
  const methods = ["linkedTarget", "currentFieldValue", "unlinkCustomField", "updateCustomField"]
    .map((name) => source.match(new RegExp(`^  function ${name}\\([^]*?^  \\}`, "m"))[0])
    .join("\n");
  const context = vm.createContext({
    protectedFieldsReady: false,
    passwordReady: false,
    templateLoading: false,
    customFields: [{ name: "alias", value: "synthetic-stored", protected: true }],
    username: "synthetic-user",
    password: "synthetic-current",
    linksDraft: new LinkedFieldsDraft({ version: 1, links: [initial().links[0]] }),
    linkedFieldValue,
  });
  context.linkedFields = context.linksDraft.value;
  vm.runInContext(ts.transpile(methods, { target: ts.ScriptTarget.ES2022 }), context);
  const c = context;
  for (const [ready, password, loading] of [
    [false, true, false],
    [true, false, false],
    [true, true, true],
  ]) {
    c.protectedFieldsReady = ready;
    c.passwordReady = password;
    c.templateLoading = loading;
    c.unlinkCustomField(0);
    c.updateCustomField(0, { protected: false });
    c.updateCustomField(0, { name: "renamed" });
    assert.equal(c.customFields[0].name, "alias");
    assert.equal(c.customFields[0].value, "synthetic-stored");
    assert.equal(c.linkedFields.links.length, 1);
  }
  c.templateLoading = false;
  c.updateCustomField(0, { protected: false });
  assert.equal(c.customFields[0].value, "synthetic-current");
  assert.equal(c.customFields[0].protected, false);
  assert.equal(c.linkedFields.links.length, 0);
});

test("drafts and template replacements have independent nested targets", () => {
  const source = initial();
  const a = new LinkedFieldsDraft(source),
    b = new LinkedFieldsDraft(source);
  a.edit({ name: "first" }, { name: "given" });
  assert.equal(a.value.links[1].target.fields[1], "given");
  assert.equal(source.links[1].target.fields[1], "first");
  assert.equal(b.value.links[1].target.fields[1], "first");
  const copy = a.value;
  copy.links[1].target.fields[0] = "other";
  assert.equal(a.value.links[1].target.fields[0], "title");
  a.replace(null);
  assert.deepEqual(a.change(), { mode: "clear" });
  assert.equal(new LinkedFieldsDraft().change(), null);
});

test("source edits keep bindings while literal alias edits, unprotection and deletion detach", () => {
  for (const patch of [{ value: "literal" }, { protected: false }]) {
    const draft = new LinkedFieldsDraft(initial());
    draft.edit({ name: "first", value: "old" }, { value: "new" });
    assert.equal(draft.value.links.length, 2);
    draft.edit({ name: "alias", protected: true }, patch);
    assert.deepEqual(
      draft.change().fields.links.map((l) => l.name),
      ["full"],
    );
    draft.unlink("full");
    assert.deepEqual(draft.change(), { mode: "clear" });
  }
});

test("rename transitions remain editable and invalid final cycles cannot be saved", () => {
  const draft = new LinkedFieldsDraft(initial());
  draft.edit({ name: "alias" }, { name: "renamed " });
  assert.equal(draft.value.links[0].name, "renamed");
  draft.edit({ name: "first" }, { name: "full" });
  assert.equal(draft.value.links[1].target.fields[1], "full");
  assert.throws(() => draft.change(), /Invalid linked fields/);
  draft.edit({ name: "full" }, { name: "fixed" });
  // Renaming a colliding source and alias cannot silently create a chain.
  assert.throws(() => draft.change(), /Invalid linked fields/);
  draft.unlink("fixed");
  assert.doesNotThrow(() => draft.change());
});

test("current local target values stay literal and official full-name whitespace is retained", () => {
  assert.equal(
    linkedFieldValue(
      { kind: "field", name: "Password" },
      () => "{REF:P@I:synthetic}{CMD:synthetic}",
    ),
    "{REF:P@I:synthetic}{CMD:synthetic}",
  );
  const values = {
    title: "\ufeff Dr ",
    first: "  Synthetic  ",
    middle: "\u0085",
    last: "Person \ufeff",
  };
  assert.equal(
    linkedFieldValue(initial().links[1].target, (name) => values[name]),
    "Dr    Synthetic   \u0085 Person",
  );
  assert.equal(
    linkedFieldValue(initial().links[1].target, () => "\ufeff\t"),
    "",
  );
});

test("bounded field metadata rejects reserved aliases, cycles, control text and excessive names", () => {
  for (const name of [
    "Password",
    "otp",
    "",
    " alias",
    "alias\u0085",
    "alias\ufeff",
    "a\u0000",
    "a\ud800",
    "x".repeat(1025),
  ]) {
    const value = initial();
    value.links[0].name = name;
    assert.throws(() => copyLinkedFields(value), /Invalid linked fields/);
  }
  for (const target of [
    { kind: "field", name: "full" },
    { kind: "field", name: "URL" },
    { kind: "bitwardenFullName", fields: ["a", "a", "b", "c"] },
  ]) {
    const value = initial();
    value.links[0].target = target;
    assert.throws(() => copyLinkedFields(value));
  }
  assert.throws(() =>
    copyLinkedFields({
      version: 1,
      links: Array.from({ length: 4097 }, (_, i) => ({
        name: `a${i}`,
        target: { kind: "field", name: "Password" },
      })),
    }),
  );
});
