import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";

// Execute the production component script, with only its environment replaced.
// This exercises event/IPC/timer interleavings without a native Tauri runtime.
async function prompt() {
  const source = await readFile(
    new URL("../src/lib/components/BridgeApprovalPrompt.svelte", import.meta.url),
    "utf8",
  );
  const script = source.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const { outputText } = ts.transpileModule(script, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ESNext },
    transformers: {
      before: [
        () => (file) =>
          ts.factory.updateSourceFile(
            file,
            file.statements.filter((statement) => !ts.isImportDeclaration(statement)),
          ),
      ],
    },
  });
  const listeners = new Map();
  const timers = new Map();
  const requests = [];
  let cleanup;
  let timerId = 0;
  const context = vm.createContext({
    $state: (value) => value,
    $effect: (effect) => effect(),
    $derived: (value) => value,
    $appSettings: { general: { language: "en" } },
    appSettings: {
      subscribe: (callback) => {
        callback({ general: { language: "en" } });
        return () => {};
      },
    },
    isTauriRuntime: () => true,
    onMount: (mount) => {
      cleanup = mount();
    },
    listen: async (name, callback) => {
      listeners.set(name, callback);
      return () => listeners.delete(name);
    },
    invoke: (command, args) =>
      new Promise((resolve, reject) => requests.push({ command, args, resolve, reject })),
    t: (_, key) => key,
    setTimeout: (callback) => {
      timers.set(++timerId, callback);
      return timerId;
    },
    clearTimeout: (id) => timers.delete(id),
  });
  vm.runInContext(
    `${outputText.replace(/^export \{\};\s*$/m, "")}\nglobalThis.inspect = () => ({ pending, busy, outcome, outcomeMsg }); globalThis.answer = decide;`,
    context,
  );
  await Promise.resolve();
  return {
    emit: (payload) => listeners.get("bridge-associate-request")({ payload }),
    answer: (allowed) => context.answer(allowed),
    inspect: () => context.inspect(),
    requests,
    timers,
    cleanup: () => cleanup(),
  };
}

test("old approval success cannot mark or dismiss the replacement prompt", async () => {
  const ui = await prompt();
  ui.emit({ token: "a", id: "client-a" });
  const old = ui.answer(true);
  ui.emit({ token: "b", id: "client-b" });
  ui.requests[0].resolve();
  await old;
  assert.equal(ui.inspect().pending.token, "b");
  assert.equal(ui.inspect().outcome, null);
  assert.equal(ui.timers.size, 0);
  ui.cleanup();
});

test("old approval failure cannot release a replacement decision's busy state", async () => {
  const ui = await prompt();
  ui.emit({ token: "a", id: "client-a" });
  const old = ui.answer(true);
  ui.emit({ token: "b", id: "client-b" });
  const current = ui.answer(false);
  ui.requests[0].reject(new Error("synthetic expired approval"));
  await old;
  assert.equal(ui.inspect().busy, true);
  assert.equal(ui.inspect().outcome, null);
  assert.equal(ui.timers.size, 0);
  ui.requests[1].resolve();
  await current;
  assert.equal(ui.inspect().busy, false);
  assert.equal(ui.inspect().outcomeMsg, "approval.denied");
  ui.cleanup();
});

test("unmounted approval does not install a late dismissal timer", async () => {
  const ui = await prompt();
  ui.emit({ token: "a", id: "client-a" });
  const answer = ui.answer(true);
  ui.cleanup();
  ui.requests[0].resolve();
  await answer;
  assert.equal(ui.timers.size, 0);
});

test("a replacement approval survives the old outcome dismissal deadline", async () => {
  const ui = await prompt();
  ui.emit({ token: "a", id: "client-a" });
  const answer = ui.answer(true);
  ui.requests[0].resolve();
  await answer;
  const oldTimer = [...ui.timers.values()][0];
  ui.emit({ token: "b", id: "client-b" });
  // Exercise a stale callback independently of the host timer cancellation.
  oldTimer();
  assert.equal(ui.inspect().pending.token, "b");
  assert.equal(ui.inspect().outcome, null);
  ui.cleanup();
});
