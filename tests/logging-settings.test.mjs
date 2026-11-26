import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { register } from "node:module";
import { runInNewContext } from "node:vm";
import { after, test } from "node:test";
import { get } from "svelte/store";
import ts from "typescript";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.window = globalThis;
let persisted;
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    if (command === "set_config") return (persisted = structuredClone(args.config));
    if (command === "get_config") return structuredClone(persisted);
    if (command === "remote_secret_present") return { accessKey: false, secretKey: false };
    throw new Error(`Unexpected ${command}`);
  },
};
const { appSettings, DEFAULT_APP_SETTINGS, normalizeSettings } =
  await import("../src/lib/services/settings.ts");
after(() => appSettings.destroy());
test("logging defaults off; valid explicit choices publish and survive actual persistence/reload", async () => {
  assert.equal(DEFAULT_APP_SETTINGS.logging.enabled, false);
  for (const input of [undefined, {}, { enabled: null }, { enabled: 1 }, { enabled: "true" }]) {
    assert.equal(normalizeSettings({ logging: input }).logging.enabled, false);
  }
  for (const enabled of [true, false]) {
    appSettings.updateLogging(enabled);
    assert.equal(get(appSettings).logging.enabled, enabled);
    await appSettings.flush();
    assert.deepEqual(persisted.logging, { enabled });
    await appSettings.initialize();
    assert.equal(get(appSettings).logging.enabled, enabled);
  }
});
const script = readFileSync("src/lib/components/settings/LoggingSettingsCard.svelte", "utf8");
const handler = script.match(/^  async function refresh\([^]*?^  \}/m)[0];
const executable = ts.transpileModule(handler + "\nexports.refresh=refresh;", {
  compilerOptions: { target: ts.ScriptTarget.ES2022 },
}).outputText;
function fixture() {
  const calls = [];
  let flushes = 0;
  const context = {
    exports: {},
    enabled: true,
    generation: 0,
    status: null,
    pending: false,
    unavailable: false,
    appSettings: {
      flush: async () => {
        flushes++;
      },
    },
    invoke: (command) => new Promise((resolve, reject) => calls.push({ command, resolve, reject })),
  };
  runInNewContext(executable, context);
  return { context, calls, flushes: () => flushes };
}
const tick = () => new Promise((resolve) => setImmediate(resolve));
test("the actual status handler flushes changes, reports failed/full sinks and polls without repeated persistence", async () => {
  const f = fixture();
  const run = f.context.exports.refresh(true);
  await tick();
  assert.equal(f.flushes(), 1);
  assert.equal(f.calls[0].command, "diagnostic_log_status");
  f.calls[0].resolve({ enabled: true, active: false, path: "synthetic.log", limitReached: true });
  await run;
  assert.equal(f.context.unavailable, true);
  assert.equal(f.context.status.limitReached, true);
  assert.equal(f.context.pending, false);
  const poll = f.context.exports.refresh(true, false);
  f.calls[1].resolve({ enabled: true, active: true, path: "synthetic.log", limitReached: false });
  await poll;
  assert.equal(f.flushes(), 1);
  assert.equal(f.context.unavailable, false);
});
test("disposed and replaced logging status replies cannot republish old paths or feedback", async () => {
  for (const changed of [false, true]) {
    const f = fixture();
    const run = f.context.exports.refresh(true);
    await tick();
    if (changed) f.context.enabled = false;
    else f.context.generation++;
    f.calls[0].resolve({
      enabled: true,
      active: true,
      path: "synthetic-old.log",
      limitReached: false,
    });
    await run;
    assert.equal(f.context.status, null);
  }
});
