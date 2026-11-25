import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
import { runInNewContext } from "node:vm";

const require = createRequire(import.meta.url);
const ts = require("typescript");
const component = readFileSync(
  new URL("../src/lib/components/GroupMetaDialog.svelte", import.meta.url),
  "utf8",
);
const script = component.match(/<script lang="ts">([^]*?)<\/script>/)[1];
const ast = ts.createSourceFile("GroupMetaDialog.ts", script, ts.ScriptTarget.Latest, true);
const names = new Set(["notes", "tags", "initialSearching", "enableSearching", "saving"]);
const statements = ast.statements.filter(
  (node) =>
    (ts.isVariableStatement(node) &&
      node.declarationList.declarations.some((declaration) =>
        names.has(declaration.name.getText(ast)),
      )) ||
    (ts.isFunctionDeclaration(node) && node.name?.text === "submit"),
);
assert.equal(
  statements.length,
  6,
  "extract all five production state declarations and complete submit",
);
const body = statements.map((node) => node.getText(ast)).join("\n");
// Execute complete production submit and initialization, with a plain $state
// seam. This checks request omission/retry behavior, not rendered UI or effects.
const js = ts.transpileModule(
  body +
    "\nexports.submit = submit; exports.toggle = () => enableSearching = !enableSearching; exports.note = value => notes = value;",
  { compilerOptions: { target: ts.ScriptTarget.ESNext } },
).outputText;
function fixture(enabled, saved = async () => true) {
  const calls = [];
  let closed = 0;
  const exports = {};
  runInNewContext(js, {
    exports,
    $state: (value) => value,
    group: { enableSearching: enabled, searchingSetting: null, notes: "", tags: "" },
    onsaved: async (meta) => {
      calls.push({ ...meta });
      return saved();
    },
    onclose: () => closed++,
  });
  return { ...exports, calls, closed: () => closed };
}
test("saving group notes preserves inheritance for both effective values", async () => {
  for (const enabled of [false, true]) {
    const f = fixture(enabled);
    f.note("Synthetic note");
    await f.submit();
    assert.equal(f.calls[0].notes, "Synthetic note");
    assert.equal(Object.hasOwn(f.calls[0], "enableSearching"), false);
    assert.equal(f.closed(), 1);
  }
});
test("changing the search toggle sends an explicit boolean; reverting preserves inherit", async () => {
  for (const enabled of [false, true]) {
    const changed = fixture(enabled);
    changed.toggle();
    await changed.submit();
    assert.equal(changed.calls[0].enableSearching, !enabled);
    const reverted = fixture(enabled);
    reverted.toggle();
    reverted.toggle();
    await reverted.submit();
    assert.equal(Object.hasOwn(reverted.calls[0], "enableSearching"), false);
  }
});
test("failed metadata submission remains retryable without persisting a search override", async () => {
  let succeeds = false;
  const f = fixture(false, async () => succeeds);
  await f.submit();
  assert.equal(f.closed(), 0);
  succeeds = true;
  await f.submit();
  assert.equal(f.closed(), 1);
  assert.equal(f.calls.length, 2);
  assert.equal(
    f.calls.every((meta) => !Object.hasOwn(meta, "enableSearching")),
    true,
  );
});
