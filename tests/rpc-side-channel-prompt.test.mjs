import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";

// Run the complete production script; timers, events and clipboard are isolated.
async function prompt({ delayedListen = false } = {}) {
  const source = await readFile(
    new URL("../src/lib/components/RpcSideChannelPrompt.svelte", import.meta.url),
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
  const timers = new Map();
  const copies = [];
  const resume = new Map();
  let callback;
  let cleanup;
  let now = 0;
  let timerId = 0;
  let unlistened = false;
  let finishListen;
  const derived = (value) => value;
  derived.by = (fn) => fn();
  const eventTarget = {
    addEventListener: (name, listener) => resume.set(name, listener),
    removeEventListener: (name) => resume.delete(name),
  };
  const context = vm.createContext({
    $state: (value) => value,
    $effect: (effect) => effect(),
    $derived: derived,
    $appSettings: { general: { language: "en" } },
    appSettings: {
      subscribe: (listener) => {
        listener({ general: { language: "en" } });
        return () => {};
      },
    },
    isTauriRuntime: () => true,
    onMount: (mount) => {
      cleanup = mount();
    },
    listen: (_, listener) => {
      callback = listener;
      const stop = () => {
        unlistened = true;
      };
      return delayedListen
        ? new Promise((resolve) => {
            finishListen = () => resolve(stop);
          })
        : Promise.resolve(stop);
    },
    copyText: (text) => new Promise((resolve, reject) => copies.push({ text, resolve, reject })),
    Date: { now: () => now },
    document: eventTarget,
    window: eventTarget,
    setInterval: (listener) => {
      timers.set(++timerId, listener);
      return timerId;
    },
    clearInterval: (id) => timers.delete(id),
  });
  vm.runInContext(
    `${outputText.replace(/^export \{\};\s*$/m, "")}\nglobalThis.inspect = () => ({ pending, remaining, copied }); globalThis.copy = copyPassword; globalThis.dismiss = close;`,
    context,
  );
  await Promise.resolve();
  return {
    emit: (payload) => callback({ payload }),
    advance: (ms) => {
      now += ms;
    },
    inspect: () => context.inspect(),
    copy: () => context.copy(),
    close: () => context.dismiss(),
    cleanup: () => cleanup(),
    finishListen: async () => {
      finishListen();
      await Promise.resolve();
    },
    unlistened: () => unlistened,
    resume,
    copies,
    timers,
  };
}

const request = (password = "synthetic-a", expiresInSecs = 120) => ({ password, expiresInSecs });

test("expiry removes the displayed password and timer", async () => {
  const ui = await prompt();
  ui.emit(request("synthetic-a", 1));
  ui.advance(1000);
  [...ui.timers.values()][0]();
  assert.equal(ui.inspect().pending, null);
  assert.equal(ui.inspect().remaining, 0);
  assert.equal(ui.timers.size, 0);
  ui.cleanup();
});

test("a delayed interval uses elapsed time rather than callback count", async () => {
  const ui = await prompt();
  ui.emit(request());
  ui.advance(110000);
  [...ui.timers.values()][0]();
  assert.equal(ui.inspect().remaining, 10);
  ui.advance(10000);
  [...ui.timers.values()][0]();
  assert.equal(ui.inspect().pending, null);
  ui.cleanup();
});

test("copy cannot start after expiry even before the timer resumes", async () => {
  const ui = await prompt();
  ui.emit(request());
  ui.advance(120000);
  const copying = ui.copy();
  // Release an unexpected old implementation call without hanging the test.
  ui.copies[0]?.resolve();
  await copying;
  assert.equal(ui.copies.length, 0);
  assert.equal(ui.inspect().pending, null);
  ui.cleanup();
});

test("an old copy completion cannot mark a replacement prompt copied", async () => {
  const ui = await prompt();
  ui.emit(request());
  const old = ui.copy();
  ui.emit(request("synthetic-b"));
  ui.copies[0].resolve();
  await old;
  assert.equal(ui.inspect().pending.password, "synthetic-b");
  assert.equal(ui.inspect().copied, false);
  const current = ui.copy();
  assert.equal(ui.copies[1].text, "synthetic-b");
  ui.copies[1].resolve();
  await current;
  assert.equal(ui.inspect().copied, true);
  ui.cleanup();
});

test("expiry during copy cannot restore copied feedback", async () => {
  const ui = await prompt();
  ui.emit(request("synthetic-a", 1));
  const copying = ui.copy();
  ui.advance(1000);
  [...ui.timers.values()][0]();
  ui.copies[0].resolve();
  await copying;
  assert.equal(ui.inspect().pending, null);
  assert.equal(ui.inspect().copied, false);
  ui.cleanup();
});

test("stale timer callbacks cannot decrement or expire a replacement", async () => {
  const ui = await prompt();
  ui.emit(request("synthetic-a", 1));
  const oldTimer = [...ui.timers.values()][0];
  ui.advance(1000);
  ui.emit(request("synthetic-b"));
  oldTimer();
  assert.equal(ui.inspect().pending.password, "synthetic-b");
  assert.equal(ui.inspect().remaining, 120);
  ui.cleanup();
});

test("unmount clears the password and ignores queued events and copies", async () => {
  const ui = await prompt({ delayedListen: true });
  ui.emit(request());
  const copying = ui.copy();
  ui.cleanup();
  ui.emit(request("synthetic-b"));
  ui.copies[0].resolve();
  await copying;
  await ui.finishListen();
  assert.equal(ui.inspect().pending, null);
  assert.equal(ui.inspect().copied, false);
  assert.equal(ui.timers.size, 0);
  assert.equal(ui.resume.size, 0);
  assert.equal(ui.unlistened(), true);
});

test("invalid or expired lifetimes cannot display a password", async () => {
  const ui = await prompt();
  for (const ttl of [0, -1, NaN, Infinity]) {
    ui.emit(request("synthetic-a", ttl));
    assert.equal(ui.inspect().pending, null);
    assert.equal(ui.timers.size, 0);
  }
  ui.cleanup();
});

test("resume events refresh expiry and cleanup removes their listeners", async () => {
  const ui = await prompt();
  ui.emit(request());
  ui.advance(120000);
  assert.ok(ui.resume.has("visibilitychange"));
  ui.resume.get("visibilitychange")();
  assert.equal(ui.inspect().pending, null);
  ui.emit(request("synthetic-b"));
  ui.advance(120000);
  ui.resume.get("focus")();
  assert.equal(ui.inspect().pending, null);
  ui.cleanup();
  assert.equal(ui.resume.size, 0);
});
