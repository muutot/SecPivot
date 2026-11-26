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

test("local backups default off, normalize ranges and preserve preferences through actual persistence/reload", async () => {
  const defaults = { enabled: false, count: 3, template: "{name}.{timestamp}.{ext}.bak" };
  assert.deepEqual(DEFAULT_APP_SETTINGS.localBackup, defaults);
  for (const value of [undefined, {}, { enabled: "true", count: "4", template: null }]) {
    assert.deepEqual(normalizeSettings({ localBackup: value }).localBackup, defaults);
  }
  for (const [count, expected] of [
    [0, 1],
    [51, 50],
    [2.4, 2],
    [NaN, 3],
  ]) {
    assert.equal(normalizeSettings({ localBackup: { count } }).localBackup.count, expected);
  }
  appSettings.updateLocalBackup({
    enabled: true,
    count: 2,
    template: " {name}-copy-{timestamp}.{ext} ",
  });
  await appSettings.flush();
  assert.deepEqual(persisted.localBackup, {
    enabled: true,
    count: 2,
    template: "{name}-copy-{timestamp}.{ext}",
  });
  await appSettings.initialize();
  assert.deepEqual(get(appSettings).localBackup, persisted.localBackup);
  appSettings.updateLocalBackup({ enabled: false });
  await appSettings.flush();
  assert.equal(persisted.localBackup.enabled, false);
  assert.equal(persisted.localBackup.count, 2);
});

const script = readFileSync("src/lib/components/settings/LocalBackupSettingsCard.svelte", "utf8");
const handler = script.match(/^  async function apply\([^]*?^  \}/m)[0];
const executable = ts.transpileModule(handler + "\nexports.apply=apply;", {
  compilerOptions: { target: ts.ScriptTarget.ES2022 },
}).outputText;
function fixture() {
  const requests = [],
    updates = [];
  let flushes = 0;
  const context = {
    exports: {},
    busy: false,
    generation: 0,
    count: "2",
    template: " {name}-{timestamp}.{ext} ",
    error: "",
    lang: "en",
    savedCount: 3,
    savedTemplate: "old",
    isTauriRuntime: () => true,
    isMobile: () => false,
    t: (_, key) => key,
    invoke: (command, args) =>
      new Promise((resolve, reject) => requests.push({ command, args, resolve, reject })),
    appSettings: {
      updateLocalBackup: (value) => updates.push(value),
      flush: async () => {
        flushes++;
      },
    },
  };
  runInNewContext(executable, context);
  return { context, requests, updates, flushes: () => flushes };
}

test("the actual Apply handler requires native validation, then publishes canonical fields and flushes", async () => {
  const f = fixture();
  const pending = f.context.exports.apply();
  assert.equal(f.context.busy, true);
  assert.equal(f.requests[0].command, "validate_local_backup_template");
  assert.equal(f.updates.length, 0);
  f.requests[0].resolve("{name}-{timestamp}.{ext}");
  await pending;
  assert.equal(f.updates[0].count, 2);
  assert.equal(f.updates[0].template, "{name}-{timestamp}.{ext}");
  assert.equal(f.flushes(), 1);
  assert.equal(f.context.busy, false);
});

test("invalid counts, native refusal and disposed replies do not persist backup drafts", async () => {
  for (const count of ["", "0", "51", "2.5"]) {
    const f = fixture();
    f.context.count = count;
    await f.context.exports.apply();
    assert.equal(f.requests.length, 0);
    assert.equal(f.updates.length, 0);
    assert.equal(f.context.error, "database.backupCountInvalid");
  }
  for (const disposed of [true, false]) {
    const f = fixture();
    const pending = f.context.exports.apply();
    if (disposed) {
      f.context.generation++;
      f.requests[0].resolve("old-template");
    } else f.requests[0].reject(new Error("synthetic template refused"));
    await pending;
    assert.equal(f.updates.length, 0);
    assert.equal(f.flushes(), 0);
  }
});
