import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import test from "node:test";
import ts from "typescript";
const source = readFileSync(new URL("../src/routes/+page.svelte", import.meta.url), "utf8").match(
  /<script lang="ts">([^]*?)<\/script>/,
)[1];
const ast = ts.createSourceFile("page.ts", source, ts.ScriptTarget.Latest, true);
const handler = ast.statements.find(
  (node) => ts.isFunctionDeclaration(node) && node.name?.text === "moveGroupTo",
);
assert.ok(handler);
const js = ts.transpileModule(handler.getText(ast) + "\nexports.move=moveGroupTo;", {
  compilerOptions: { target: ts.ScriptTarget.ESNext },
}).outputText;
function fixture() {
  let resolve;
  const pending = new Promise((done) => (resolve = done));
  const calls = [],
    tips = [];
  const context = {
    exports: {},
    current: true,
    currentVault: { readOnly: false },
    selectedGroup: "old",
    revealGroupUuid: "old",
    selection: {
      selectedEntry: { uuid: "old-entry" },
      selectedUuids: new Set(["old-entry"]),
      selectionAnchor: "old",
    },
    settings: { general: { language: "en" } },
    Set,
    String,
    sessionView: { capture: () => ({ sessionId: "origin" }), isCurrent: () => context.current },
    vault: {
      callInSession: (sessionId, operation) => {
        assert.equal(sessionId, "origin");
        return operation();
      },
      moveGroup: (...args) => {
        calls.push(args);
        return pending;
      },
    },
    tick: async () => {},
    t: (_, key) => key,
    flash: (...args) => tips.push(args),
  };
  runInNewContext(js, context);
  return { context, resolve, calls, tips };
}
test("successful drop selects/reveals moved group and clears the previous entry selection", async () => {
  const f = fixture();
  const operation = f.context.exports.move({ uuid: "a", parentUuid: "b", beforeUuid: null });
  assert.deepEqual(f.calls, [["a", "b", null]]);
  f.resolve();
  await operation;
  assert.equal(f.context.selectedGroup, "a");
  assert.equal(f.context.revealGroupUuid, "a");
  assert.equal(f.context.selection.selectedEntry, null);
  assert.equal(f.context.selection.selectedUuids.size, 0);
});
test("late or read-only drops cannot change a newer view", async () => {
  const f = fixture();
  const operation = f.context.exports.move({ uuid: "a", parentUuid: "b", beforeUuid: null });
  f.context.current = false;
  f.resolve();
  await operation;
  assert.equal(f.context.selectedGroup, "old");
  assert.equal(f.context.revealGroupUuid, "old");
  const r = fixture();
  r.context.currentVault.readOnly = true;
  await r.context.exports.move({ uuid: "a", parentUuid: "b", beforeUuid: null });
  assert.deepEqual(r.calls, []);
});
