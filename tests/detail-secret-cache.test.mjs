import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";

// Actual read and invalidation bodies with an isolated synthetic service seam.
const source = readFileSync("src/lib/components/EntryDetail.svelte", "utf8");
const reset = source.match(/\$effect\(\(\) => \{\s*void entry;[^]*?\n  \}\);/)[0];
const methods = ["ensurePassword", "ensureCustomField"]
  .map((name) => source.match(new RegExp(`^  async function ${name}\\([^]*?^  \\}`, "m"))[0])
  .join("\n");
function fixture() {
  const pending = [];
  const context = vm.createContext({
    entry: { uuid: "synthetic-entry" },
    notesDraft: "unsaved notes",
    notesDirty: true,
    editing: { kind: "username" },
    editValue: "unsaved username",
    revealPassword: true,
    passwordLoaded: false,
    passwordLoading: false,
    fetchedPassword: "",
    customFieldValues: {},
    customFieldLoaded: {},
    customFieldLoading: {},
    customFieldRevealed: {},
    $effect: (fn) => {
      context.invalidate = fn;
    },
    untrack: (fn) => fn(),
    detailView: { capture: () => ({}), isCurrent: () => true },
    detailSessionId: () => "synthetic-owner",
    vault: {
      callInSession: (_id, run) => run(),
      getEntryPassword: () => new Promise((resolve) => pending.push(resolve)),
      getCustomFieldValue: () => new Promise((resolve) => pending.push(resolve)),
    },
  });
  vm.runInContext(
    ts.transpile(
      `${reset}\n${methods}\nglobalThis.password = ensurePassword; globalThis.field = ensureCustomField;`,
      { target: ts.ScriptTarget.ES2022 },
    ),
    context,
  );
  return { context, pending };
}

test("same-entry snapshot invalidation wipes caches while preserving unsaved drafts", () => {
  const { context: c } = fixture();
  c.fetchedPassword = "synthetic-old";
  c.passwordLoaded = true;
  c.customFieldValues = { alias: "synthetic-old" };
  c.customFieldRevealed = { alias: true };
  c.invalidate();
  assert.equal(c.fetchedPassword, "");
  assert.equal(c.passwordLoaded, false);
  assert.equal(c.revealPassword, false);
  assert.equal(Object.keys(c.customFieldValues).length, 0);
  assert.equal(Object.keys(c.customFieldRevealed).length, 0);
  assert.equal(c.notesDraft, "unsaved notes");
  assert.equal(c.notesDirty, true);
  assert.equal(c.editValue, "unsaved username");
  assert.equal(c.editing.kind, "username");
});

test("obsolete secret results cannot refill or clear a newer same-UUID request", async () => {
  for (const kind of ["password", "field"]) {
    const { context: c, pending } = fixture();
    const old = c[kind]("alias");
    c.entry = { uuid: "synthetic-entry" };
    c.invalidate();
    const current = c[kind]("alias");
    pending[0]("synthetic-old");
    assert.equal(await old, null);
    assert.equal(kind === "password" ? c.passwordLoading : c.customFieldLoading.alias, true);
    assert.equal(
      kind === "password" ? c.fetchedPassword : c.customFieldValues.alias,
      kind === "password" ? "" : undefined,
    );
    pending[1]("synthetic-current");
    assert.equal(await current, "synthetic-current");
    assert.equal(
      kind === "password" ? c.fetchedPassword : c.customFieldValues.alias,
      "synthetic-current",
    );
  }
});
