import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
import { pathToFileURL } from "node:url";
import { runInNewContext } from "node:vm";

const require = createRequire(import.meta.url);
const ts = require("typescript");
const { compileModule } = require("svelte/compiler");
const client = await import(pathToFileURL(require.resolve("svelte/internal/client")).href);
const source = readFileSync(
  new URL("../src/lib/composables/useTotpCode.svelte.ts", import.meta.url),
  "utf8",
);
const javascript = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext },
}).outputText;
const compiled = compileModule(javascript, { generate: "client" }).js.code;
const commonjs = ts.transpileModule(compiled, {
  compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.CommonJS },
}).outputText;

// Complete production rune module and actual Svelte effects. Only vault/OTP,
// clipboard and timers/events are synthetic; no OS clipboard or vault is used.
function fixture(t) {
  const pending = [];
  const timers = new Map();
  let timerId = 0;
  let active = "synthetic-owner";
  const activeListeners = new Set();
  const vault = {
    getActiveSessionId: () => active,
    activeId: {
      subscribe(fn) {
        activeListeners.add(fn);
        fn(active);
        return () => activeListeners.delete(fn);
      },
    },
    callInSession: async (id, run) => {
      assert.equal(id, active);
      return run();
    },
    totpCode: async (uuid) => ({
      code: `synthetic-${uuid}`,
      kind: "totp",
      period: 30,
      validFor: 30,
    }),
  };
  const exports = {};
  runInNewContext(commonjs, {
    exports,
    require(name) {
      if (name === "svelte/internal/client") return client;
      if (name === "$lib/services/vault") return { vault };
      if (name === "$lib/utils/clipboard") {
        return {
          copyText: (code) =>
            new Promise((resolve, reject) => pending.push({ code, resolve, reject })),
        };
      }
      throw new Error(`Unexpected compiled import ${name}`);
    },
    Date,
    document: { hidden: false, addEventListener() {} },
    setTimeout(fn, delay) {
      timers.set(++timerId, { fn, delay, kind: "timeout" });
      return timerId;
    },
    clearTimeout: (id) => timers.delete(id),
    setInterval(fn, delay) {
      timers.set(++timerId, { fn, delay, kind: "interval" });
      return timerId;
    },
    clearInterval: (id) => timers.delete(id),
  });
  const selected = client.state("A");
  let state;
  const disposers = [
    client.effect_root(() => {
      state = exports.useTotpCode(() => client.get(selected));
    }),
  ];
  const settle = async () => {
    for (let i = 0; i < 6; i++) {
      client.flush();
      await Promise.resolve();
    }
  };
  const dispose = () => {
    for (const close of disposers.splice(0)) close();
  };
  t.after(dispose);
  return {
    state,
    pending,
    timers,
    settle,
    dispose,
    select(uuid) {
      client.set(selected, uuid);
      client.flush();
    },
    switchSession(id) {
      active = id;
      for (const fn of activeListeners) fn(id);
      client.flush();
    },
    keepSiblingOnA() {
      disposers.push(client.effect_root(() => exports.useTotpCode(() => "A")));
      client.flush();
    },
    expireFeedback() {
      const [id, timer] = [...timers].find(([, value]) => value.kind === "timeout");
      assert.equal(timer.delay, 1200);
      timers.delete(id);
      timer.fn();
      client.flush();
    },
  };
}

test("pending OTP copy cannot mark a later entry copied", async (t) => {
  const f = fixture(t);
  await f.settle();
  assert.equal(f.state.code, "synthetic-A");
  const copying = f.state.copy();
  assert.equal(f.pending[0].code, "synthetic-A");
  f.select("B");
  await f.settle();
  assert.equal(f.state.code, "synthetic-B");
  f.pending[0].resolve();
  await copying;
  await f.settle();
  assert.equal(f.state.copied, false);
  assert.equal(f.timers.size, 1, "only the shared ticker remains");
});

test("pending OTP copy cannot mark a revisited entry copied", async (t) => {
  const f = fixture(t);
  await f.settle();
  const copying = f.state.copy();
  f.select("B");
  await f.settle();
  f.select("A");
  await f.settle();
  f.pending[0].resolve();
  await copying;
  await f.settle();
  assert.equal(f.state.copied, false);
});

test("revisiting a shared cached OTP entry still invalidates old copy feedback", async (t) => {
  const f = fixture(t);
  await f.settle();
  f.keepSiblingOnA();
  const copying = f.state.copy();
  f.select("B");
  await f.settle();
  f.select("A");
  await f.settle();
  f.pending[0].resolve();
  await copying;
  await f.settle();
  assert.equal(f.state.copied, false);
});

test("pending OTP copy cannot mark a new session copied", async (t) => {
  const f = fixture(t);
  await f.settle();
  const copying = f.state.copy();
  f.switchSession("synthetic-other-owner");
  await f.settle();
  f.pending[0].resolve();
  await copying;
  await f.settle();
  assert.equal(f.state.copied, false);
});

test("pending OTP copy cannot mark a revisited session copied", async (t) => {
  const f = fixture(t);
  await f.settle();
  const copying = f.state.copy();
  f.switchSession("synthetic-other-owner");
  await f.settle();
  f.switchSession("synthetic-owner");
  await f.settle();
  f.pending[0].resolve();
  await copying;
  await f.settle();
  assert.equal(f.state.copied, false);
});

test("disposed OTP state cannot regain copy feedback or create a timer", async (t) => {
  const f = fixture(t);
  await f.settle();
  const copying = f.state.copy();
  f.dispose();
  f.pending[0].resolve();
  await copying;
  await f.settle();
  assert.equal(f.state.copied, false);
  assert.equal(f.timers.size, 0);
});

test("current OTP copy shows feedback and clears it after 1200 ms", async (t) => {
  const f = fixture(t);
  await f.settle();
  const copying = f.state.copy();
  f.pending[0].resolve();
  await copying;
  await f.settle();
  assert.equal(f.state.copied, true);
  f.expireFeedback();
  assert.equal(f.state.copied, false);
});

test("failed OTP copy does not show success feedback", async (t) => {
  const f = fixture(t);
  await f.settle();
  const copying = f.state.copy();
  f.pending[0].reject(new Error("Synthetic clipboard refusal"));
  await copying;
  await f.settle();
  assert.equal(f.state.copied, false);
  assert.equal(f.timers.size, 1);
});
