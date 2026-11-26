import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { register } from "node:module";
import { get, writable } from "svelte/store";
import { runInNewContext } from "node:vm";
import test from "node:test";
import ts from "typescript";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
const { KdfOpenPromptController } = await import("../src/lib/services/kdf-open-prompt.ts");
const { MasterKeyOpenPromptController } =
  await import("../src/lib/services/master-key-open-prompt.ts");
const source = readFileSync("src/lib/components/KdfOpenPrompt.svelte", "utf8").match(
  /<script lang="ts">([^]*?)<\/script>/,
)[1];
const ast = ts.createSourceFile("host.ts", source, ts.ScriptTarget.Latest, true);
const executable = ast.statements
  .filter((node) => !ts.isImportDeclaration(node))
  .map((node) => node.getText(ast))
  .join("\n");
const js = ts.transpileModule(
  executable +
    "\nexports.controller=controller; exports.prompt=()=>prompt; exports.masterController=masterController; exports.masterPrompt=()=>masterPrompt;",
  { compilerOptions: { target: ts.ScriptTarget.ESNext } },
).outputText;
const flush = () => new Promise((resolve) => setImmediate(resolve));
const offer = {
  token: "native-lifetime",
  fileName: "synthetic.kdbx",
  parameters: { algorithm: "Aes", rounds: "1" },
  recommended: { algorithm: "Aes", rounds: "600000" },
  readOnly: false,
};
function fixture(runtime = true, masterRequired = false) {
  const state = writable(null),
    tabs = writable([]);
  const appSettings = writable({
    security: { keyTransformWeakWarning: true },
    general: { language: "en" },
  });
  const cleanups = [],
    calls = [],
    inspections = [],
    masterInspections = [];
  let active = null;
  const epochs = new Map();
  let operationOwner = null;
  const vault = {
    subscribe: state.subscribe,
    tabs,
    getActiveSessionId: () => active,
    getSessionEpoch: (id) => epochs.get(id),
    callInSession: (id, operation) => {
      const previous = operationOwner;
      operationOwner = id;
      try {
        return operation();
      } finally {
        operationOwner = previous;
      }
    },
    getKdfUpgradeOffer: () =>
      new Promise((resolve) => inspections.push({ id: operationOwner, resolve })),
    getMasterKeyExpiryOffer: () =>
      masterRequired
        ? new Promise((resolve) => masterInspections.push({ id: operationOwner, resolve }))
        : Promise.resolve({ required: false, recommended: false, invalidRules: [] }),
    closeTab: async (id) => {
      calls.push({ closed: id });
    },
    upgradeWeakKdf: async (offer) => {
      calls.push({ id: operationOwner, offer });
    },
  };
  const context = {
    exports: {},
    $state: (value) => value,
    get,
    appSettings,
    vault,
    KdfOpenPromptController,
    MasterKeyOpenPromptController,
    isTauriRuntime: () => runtime,
    onDestroy: (callback) => cleanups.push(callback),
    t: (_, key) => key,
    String,
  };
  runInNewContext(js, context);
  return {
    context,
    calls,
    inspections,
    masterInspections,
    open: (id) => {
      active = id;
      epochs.set(id, Symbol(id));
      state.set({ dirty: false });
      tabs.set([...epochs.keys()].map((sessionId) => ({ sessionId })));
    },
    close: () => {
      active = null;
      epochs.clear();
      state.set(null);
      tabs.set([]);
    },
    cleanup: () => cleanups.forEach((run) => run()),
  };
}

test("actual prompt host subscribes to native lifetimes and scopes approval to the original session", async () => {
  const f = fixture();
  f.open("origin");
  await flush();
  assert.equal(f.inspections[0].id, "origin");
  f.inspections[0].resolve(offer);
  await flush();
  assert.equal(f.context.exports.prompt().offer, offer);
  assert.deepEqual(f.calls, []);
  await f.context.exports.controller.upgrade();
  assert.deepEqual(f.calls, [{ id: "origin", offer }]);
  assert.equal(f.context.exports.prompt(), null);
  f.cleanup();
});

test("actual host ignores late offers after close/unmount and browser preview never invokes native IPC", async () => {
  for (const unmount of [false, true]) {
    const f = fixture();
    f.open("origin");
    await flush();
    if (unmount) f.cleanup();
    else f.close();
    f.inspections[0].resolve(offer);
    await flush();
    assert.equal(f.context.exports.prompt(), null);
    assert.deepEqual(f.calls, []);
    if (!unmount) f.cleanup();
  }
  const browser = fixture(false);
  browser.open("browser");
  assert.deepEqual(browser.inspections, []);
  browser.cleanup();
});

test("actual host renews same-id recommendations after a decrypted lifetime replacement", async () => {
  const f = fixture();
  f.open("origin");
  await flush();
  f.inspections[0].resolve(offer);
  await flush();
  f.context.exports.controller.dismiss();
  f.open("origin");
  await flush();
  assert.equal(f.inspections.length, 2);
  f.inspections[1].resolve({ ...offer, token: "replacement" });
  await flush();
  assert.equal(f.context.exports.prompt().offer.token, "replacement");
  f.cleanup();
});

test("actual host requires the master-key decision before inspecting KDF costs", async () => {
  const f = fixture(true, true);
  f.open("origin");
  assert.equal(f.masterInspections[0].id, "origin");
  assert.deepEqual(f.inspections, []);
  const required = {
    token: "native-expiry",
    required: true,
    recommended: false,
    readOnly: false,
    invalidRules: [],
  };
  f.masterInspections[0].resolve(required);
  await flush();
  f.context.exports.masterController.decline();
  assert.deepEqual(f.inspections, []);
  f.context.exports.masterController.change();
  const owner = f.context.exports.masterPrompt().owner;
  f.context.exports.masterController.changed(owner);
  await flush();
  assert.equal(f.inspections[0].id, "origin");
  f.inspections[0].resolve(offer);
  await flush();
  assert.equal(f.context.exports.prompt().offer, offer);
  f.cleanup();
});
