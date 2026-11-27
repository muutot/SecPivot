import assert from "node:assert/strict";
import test from "node:test";
import ts from "typescript";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";

const script = readFileSync(new URL("../src/routes/+page.svelte", import.meta.url), "utf8").match(
  /<script lang="ts">([\s\S]*?)<\/script>/,
)[1];
const ast = ts.createSourceFile("page.ts", script, ts.ScriptTarget.Latest, true);
const declaration = (name) =>
  ast.statements.find(
    (n) =>
      ts.isVariableStatement(n) &&
      n.declarationList.declarations.some((d) => d.name.getText(ast) === name),
  );
const finish = ast.statements.find(
  (n) => ts.isFunctionDeclaration(n) && n.name?.text === "finishBitwardenPrompt",
);
const io = declaration("ioHost").declarationList.declarations[0].initializer;
const request = io.properties.find(
  (n) => n.name.getText(ast) === "requestBitwardenSettings",
).initializer;
const effect = ast.statements.find(
  (n) =>
    ts.isExpressionStatement(n) &&
    ts.isCallExpression(n.expression) &&
    n.expression.expression.getText(ast) === "$effect" &&
    n.getText(ast).includes("const prompt = bitwardenPrompt"),
);
function fixture() {
  let current = true,
    epoch = Symbol();
  const context = {
    $state: (v) => v,
    $effect: (fn) => (context.guard = fn),
    currentVaultEpoch: null,
    sessionView: { isCurrent: () => current },
    vault: { getSessionEpoch: () => epoch },
  };
  const code = `${declaration("bitwardenPrompt").getText(ast)}\n${finish.getText(ast)}\n${effect.getText(ast)}\nglobalThis.request=${request.getText(ast)};globalThis.active=()=>bitwardenPrompt;`;
  runInNewContext(
    ts.transpileModule(code, { compilerOptions: { target: ts.ScriptTarget.ES2022 } }).outputText,
    context,
  );
  return {
    context,
    view: { sessionId: "synthetic" },
    switch: () => (current = false),
    replace: () => (epoch = Symbol()),
  };
}
test("actual page prompt settles cancellation, supersession and stale callbacks independently", async () => {
  const f = fixture(),
    c = f.context;
  const first = c.request(f.view),
    old = c.active();
  const second = c.request(f.view),
    fresh = c.active();
  assert.equal(await first, null);
  c.finishBitwardenPrompt({ defaultMatch: 5, equivalentDomains: [] }, old);
  assert.equal(c.active(), fresh);
  c.finishBitwardenPrompt(null);
  assert.equal(await second, null);
  assert.equal(c.active(), null);
});
test("actual page epoch guard cancels replaced/switched prompts and ignores obsolete requests", async () => {
  for (const invalidate of [(f) => f.switch(), (f) => f.replace()]) {
    const f = fixture();
    const pending = f.context.request(f.view);
    invalidate(f);
    f.context.guard();
    assert.equal(await pending, null);
    assert.equal(f.context.active(), null);
  }
  const f = fixture();
  f.switch();
  assert.equal(await f.context.request(f.view), null);
});
