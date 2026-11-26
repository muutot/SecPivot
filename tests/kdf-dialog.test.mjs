import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { register } from "node:module";
import test from "node:test";
import { runInNewContext } from "node:vm";
import ts from "typescript";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
const { KeyedViewGuard, sessionResourceKey } = await import("../src/lib/utils/session-state.ts");
const { memoryMiB, readKdfParameters, defaultKdfParameters, isWeakKdfParameters } =
  await import("../src/lib/utils/kdf-parameters.ts");
const source = readFileSync(
  new URL("../src/lib/components/DatabaseSettingsDialog.svelte", import.meta.url),
  "utf8",
).match(/<script lang="ts">([^]*?)<\/script>/)[1];
const ast = ts.createSourceFile("dialog.ts", source, ts.ScriptTarget.Latest, true);
const handlers = ast.statements.filter(
  (node) =>
    ts.isFunctionDeclaration(node) &&
    ["adoptParameters", "changeKdf", "benchmark", "save", "continueWeak"].includes(node.name?.text),
);
assert.equal(handlers.length, 5);
const js = ts.transpileModule(
  handlers.map((node) => node.getText(ast)).join("\n") +
    "\nexports.benchmark=benchmark; exports.save=save; exports.adopt=adoptParameters; exports.continueWeak=continueWeak;",
  { compilerOptions: { target: ts.ScriptTarget.ESNext } },
).outputText;
const original = { algorithm: "Aes", rounds: "9007199254740993" };
const suggestion = {
  parameters: { algorithm: "Aes", rounds: "1200000" },
  elapsedMs: 999,
  targetMs: 1000,
  samples: 3,
  targetReached: true,
};
function fixture() {
  let resolve;
  const pending = new Promise((done) => (resolve = done));
  const writes = [];
  const dialogView = new KeyedViewGuard();
  dialogView.activate(sessionResourceKey("origin", "database-settings"));
  const context = {
    exports: {},
    memoryMiB,
    defaultKdfParameters,
    isWeakKdfParameters,
    JSON,
    Number,
    String,
    lang: "en",
    t: (_, key) => key,
    dialogView,
    sessionId: "origin",
    settings: {
      kdf: "Aes",
      kdfParameters: original,
      cipher: "Twofish",
      compression: "Gzip",
      historyMaxItems: null,
      historyMaxSize: null,
      entryTemplatesGroup: null,
      recycleBinEnabled: true,
    },
    s: { security: { keyTransformWeakWarning: true } },
    kdf: "Aes",
    kdfParameters: original,
    roundsInput: original.rounds,
    iterationsInput: "3",
    memoryInput: "64",
    lanesInput: "4",
    kdfVersion: 19,
    targetInput: "1000",
    validTarget: true,
    signature: "initial",
    benchmarkSignature: "",
    benchmarking: false,
    saving: false,
    showWeakWarning: false,
    weakWarningSignature: "",
    weakApproval: null,
    benchmarkResult: null,
    error: "",
    cipher: null,
    compression: "Gzip",
    historyInput: "",
    historySizeInput: "",
    templateGroupInput: "",
    recycleEnabled: true,
    closed: 0,
    vault: {
      callInSession: (id, operation) => {
        assert.equal(id, "origin");
        return operation();
      },
      benchmarkDatabaseKdf: async (parameters, target) => {
        assert.deepEqual(parameters, original);
        assert.equal(target, 1000);
        return pending;
      },
      updateDatabaseSettings: async (patch) => {
        writes.push(JSON.parse(JSON.stringify(patch)));
      },
    },
  };
  context.onclose = () => context.closed++;
  runInNewContext(js, context);
  return { context, resolve, writes };
}

test("benchmark never saves; explicit apply updates exact fields, then Save submits a parameter-only patch", async () => {
  const { context, resolve, writes } = fixture();
  const run = context.exports.benchmark();
  assert.equal(context.benchmarking, true);
  assert.deepEqual(writes, []);
  resolve(suggestion);
  await run;
  assert.equal(context.roundsInput, original.rounds);
  assert.deepEqual(writes, []);
  context.exports.adopt(context.benchmarkResult.parameters);
  context.kdfParameters = readKdfParameters(
    context.kdf,
    context.roundsInput,
    context.iterationsInput,
    context.memoryInput,
    context.lanesInput,
    context.kdfVersion,
  );
  await context.exports.save();
  assert.deepEqual(writes, [{ kdfParameters: suggestion.parameters }]);
  assert.equal(context.closed, 1);
});

test("weak KDF changes require explicit confirmation; cancelling keeps the unsaved draft", async () => {
  const { context, writes } = fixture();
  const weak = { algorithm: "Aes", rounds: "0" };
  context.kdfParameters = weak;
  await context.exports.save();
  assert.equal(context.showWeakWarning, true);
  assert.equal(context.saving, false);
  assert.equal(context.closed, 0);
  assert.deepEqual(writes, []);
  await context.exports.save();
  assert.deepEqual(writes, []);
  context.showWeakWarning = false; // Same cancellation as the component callback / Escape.
  assert.deepEqual(context.kdfParameters, weak);
  await context.exports.save();
  assert.equal(context.showWeakWarning, true);
  context.exports.continueWeak();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(writes, [{ kdfParameters: weak }]);
  assert.equal(context.closed, 1);
  assert.equal(context.weakApproval, null);
});

test("approval is bound to the exact reviewed cost, never a changed draft", async () => {
  const { context, writes } = fixture();
  context.kdfParameters = { algorithm: "Aes", rounds: "1" };
  await context.exports.save();
  context.kdfParameters = { algorithm: "Aes", rounds: "2" };
  context.exports.continueWeak();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(context.showWeakWarning, true);
  assert.equal(context.weakWarningSignature, JSON.stringify(context.kdfParameters));
  assert.deepEqual(writes, []);
  context.exports.continueWeak();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(writes, [{ kdfParameters: context.kdfParameters }]);
});

test("unrelated metadata edits of an imported weak database do not repeatedly warn", async () => {
  const { context, writes } = fixture();
  const weak = { algorithm: "Aes", rounds: "1" };
  context.settings.kdfParameters = weak;
  context.kdfParameters = weak;
  context.historyInput = "5";
  await context.exports.save();
  assert.equal(context.showWeakWarning, false);
  assert.deepEqual(writes, [{ historyMaxItems: 5 }]);
});

test("disabled weak-cost preference allows an explicit low-cost patch without a prompt", async () => {
  const { context, writes } = fixture();
  context.s.security.keyTransformWeakWarning = false;
  context.kdfParameters = { algorithm: "Aes", rounds: "1" };
  await context.exports.save();
  assert.equal(context.showWeakWarning, false);
  assert.deepEqual(writes, [{ kdfParameters: context.kdfParameters }]);
});

test("a detached warning cannot save into a disposed view", async () => {
  const { context, writes } = fixture();
  context.kdfParameters = { algorithm: "Aes", rounds: "1" };
  await context.exports.save();
  context.dialogView.activate(null);
  context.exports.continueWeak();
  await new Promise((resolve) => setImmediate(resolve));
  assert.deepEqual(writes, []);
  assert.equal(context.closed, 0);
});

test("detached and changed-input benchmark completions never publish a suggestion", async () => {
  for (const detach of [true, false]) {
    const { context, resolve, writes } = fixture();
    const run = context.exports.benchmark();
    if (detach) context.dialogView.activate(null);
    else context.signature = "changed";
    resolve(suggestion);
    await run;
    assert.equal(context.benchmarkResult, null);
    assert.deepEqual(writes, []);
  }
});

test("saving or benchmarking blocks duplicate work and save", async () => {
  const { context, writes } = fixture();
  context.benchmarking = true;
  await context.exports.benchmark();
  await context.exports.save();
  assert.deepEqual(writes, []);
  context.benchmarking = false;
  context.saving = true;
  await context.exports.benchmark();
  assert.equal(context.benchmarkResult, null);
});
