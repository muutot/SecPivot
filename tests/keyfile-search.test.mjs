import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import { get } from "svelte/store";
import test from "node:test";
import ts from "typescript";
import { KeyfileSearchController } from "../src/lib/services/keyfile-search.ts";
const flush = () => new Promise((resolve) => setImmediate(resolve));
const result = { paths: ["X:\\synthetic.keyx"], incomplete: false, supported: true };
function fixture() {
  const calls = [];
  const controller = new KeyfileSearchController(
    () => new Promise((resolve, reject) => calls.push({ resolve, reject })),
  );
  return { controller, calls, state: () => get(controller.state) };
}
test("discovery offers candidates without choosing them; explicit choices are confined to current results", async () => {
  const f = fixture();
  f.controller.activate({ scope: "synthetic-vault", removable: false });
  assert.equal(f.state().busy, true);
  assert.equal(f.controller.allows(result.paths[0]), false);
  f.calls[0].resolve(result);
  await flush();
  assert.equal(f.controller.allows(result.paths[0]), true);
  assert.equal(f.controller.allows("X:\\unlisted.key"), false);
  f.controller.activate({ scope: "synthetic-vault", removable: false });
  assert.equal(f.calls.length, 1);
  f.controller.activate(null);
  assert.equal(f.controller.allows(result.paths[0]), false);
});
test("prompt/settings changes and disposal discard late replies; failed search preserves manual browsing", async () => {
  for (const next of [
    null,
    { scope: "other", removable: false },
    { scope: "same", removable: true },
  ]) {
    const f = fixture();
    f.controller.activate({ scope: "same", removable: false });
    f.controller.activate(next);
    f.calls[0].resolve(result);
    await flush();
    assert.equal(f.state().result, null);
    assert.equal(f.controller.allows(result.paths[0]), false);
    f.controller.dispose();
    f.calls.at(-1).resolve(result);
    await flush();
    assert.equal(f.state().result, null);
  }
  const f = fixture();
  f.controller.activate({ scope: "same", removable: false });
  f.calls[0].reject(new Error("synthetic unavailable"));
  await flush();
  assert.equal(f.state().failed, true);
  assert.equal(f.state().busy, false);
});
test("actual suggestion host flushes config before native discovery and never selects while busy or after detachment", async () => {
  const script = readFileSync("src/lib/components/KeyfileSuggestionList.svelte", "utf8").match(
    /<script lang="ts">([^]*?)<\/script>/,
  )[1];
  const ast = ts.createSourceFile("list.ts", script, ts.ScriptTarget.Latest, true);
  const body = ast.statements
    .filter((node) => !ts.isImportDeclaration(node))
    .map((node) => node.getText(ast))
    .join("\n");
  let effect, destroy, finish, releaseFlush;
  const calls = [],
    selected = [];
  const settings = {
    general: { language: "en" },
    integration: { searchKeyFiles: true, searchKeyFilesOnRemovableMedia: false },
  };
  const context = {
    exports: {},
    $props: () => ({ scope: "synthetic", onselect: (path) => selected.push(path) }),
    $derived: (x) => x,
    $effect: (fn) => (effect = fn),
    onDestroy: (fn) => (destroy = fn),
    $appSettings: settings,
    KeyfileSearchController,
    isTauriRuntime: () => true,
    isMobile: () => false,
    appSettings: { flush: () => new Promise((resolve) => (releaseFlush = resolve)) },
    invoke: (command) => {
      calls.push(command);
      return new Promise((resolve) => (finish = resolve));
    },
  };
  runInNewContext(
    ts.transpileModule(body + "\nexports.choose=choose;exports.busy=(value)=>busy=value;", {
      compilerOptions: { target: ts.ScriptTarget.ES2022 },
    }).outputText,
    context,
  );
  effect();
  assert.deepEqual(calls, []);
  releaseFlush();
  await flush();
  assert.deepEqual(calls, ["search_key_files"]);
  finish(result);
  await flush();
  assert.deepEqual(selected, []);
  context.exports.busy(true);
  context.exports.choose(result.paths[0]);
  assert.deepEqual(selected, []);
  context.exports.busy(false);
  context.exports.choose(result.paths[0]);
  assert.deepEqual(selected, result.paths);
  settings.integration.searchKeyFiles = false;
  effect();
  context.exports.choose(result.paths[0]);
  assert.equal(selected.length, 1);
  destroy();
});
