import assert from "node:assert/strict";
import { register } from "node:module";
import { after, test } from "node:test";
import { get } from "svelte/store";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
const { FileReloadController, registerFileReloadBlocker, isFileReloadBlocked } =
  await import("../src/lib/services/file-reload.ts");

test("mounted inline activity predicates block preparation and late candidates without storing draft values", async () => {
  const entry = readFileSync("src/lib/components/EntryDetail.svelte", "utf8");
  const group = readFileSync("src/lib/components/GroupNode.svelte", "utf8");
  const flags = {
    editing: null,
    editSaving: false,
    notesEditing: false,
    notesDirty: false,
    notesSaving: false,
    attachmentDragActive: false,
    renaming: false,
  };
  const releases = [];
  const context = vm.createContext({
    ...flags,
    onMount: (run) => releases.push(run()),
    registerFileReloadBlocker,
  });
  for (const source of [entry, group]) {
    const match = source.match(/  onMount\(\(\) =>\s*registerFileReloadBlocker\([\s\S]*?\);/);
    assert.ok(match);
    vm.runInContext(ts.transpile(match[0], { target: ts.ScriptTarget.ES2022 }), context);
  }
  try {
    assert.equal(isFileReloadBlocked(), false);
    for (const flag of Object.keys(flags)) {
      context[flag] = flag === "editing" ? { kind: "username" } : true;
      assert.equal(isFileReloadBlocked(), true, flag);
      const f = fixture(() => !isFileReloadBlocked());
      await f.controller.check();
      assert.equal(f.calls.length, 0);
      context[flag] = flags[flag];
      const run = f.controller.check();
      context[flag] = flag === "editing" ? { kind: "username" } : true;
      f.calls[0].resolve(ready);
      await run;
      assert.equal(f.calls.length, 1);
      context[flag] = flags[flag];
      assert.equal(isFileReloadBlocked(), false, flag);
    }
    context.renaming = true;
  } finally {
    for (const release of releases) {
      release();
      release();
    }
  }
  assert.equal(isFileReloadBlocked(), false);
});
function fixture(available = () => true) {
  const owner = { sessionId: "A", epoch: Symbol() };
  const calls = [],
    results = [],
    errors = [];
  let enabled = true,
    canApply = true;
  const controller = new FileReloadController({
    enabled: () => enabled,
    canApply: () => canApply && available(),
    prepare: (id) =>
      new Promise((resolve, reject) => calls.push({ type: "prepare", id, resolve, reject })),
    apply: (id, token) =>
      new Promise((resolve, reject) => calls.push({ type: "apply", id, token, resolve, reject })),
    result: (status) => results.push(status),
    error: (error) => errors.push(String(error)),
  });
  controller.activate(owner);
  return {
    controller,
    owner,
    calls,
    results,
    errors,
    disable: () => (enabled = false),
    dialog: () => (canApply = false),
  };
}
const ready = { status: "ready", token: "synthetic-token", state: null };
test("unchanged/busy/disabled checks never apply a candidate", async () => {
  for (const status of ["unchanged", "busy", "disabled", "ready"]) {
    const f = fixture();
    const run = f.controller.check();
    f.calls[0].resolve({ status, token: null, state: null });
    await run;
    assert.equal(f.calls.length, 1);
    assert.deepEqual(f.results, []);
  }
});
test("one check admits a current candidate and reports a successful renewed lifetime", async () => {
  const f = fixture();
  const run = f.controller.check();
  await f.controller.check();
  assert.equal(f.calls.length, 1);
  f.calls[0].resolve(ready);
  await Promise.resolve();
  assert.equal(f.calls[1].type, "apply");
  assert.equal(f.calls[1].id, "A");
  assert.equal(f.calls[1].token, "synthetic-token");
  f.controller.activate({ sessionId: "A", epoch: Symbol() });
  f.calls[1].resolve({ status: "merged", state: {}, token: null });
  await run;
  assert.deepEqual(f.results, ["merged"]);
});
test("drafts, disabled settings, A-B-A changes and disposal discard pending candidates", async () => {
  for (const action of ["dialog", "disable", "revisit", "dispose"]) {
    const f = fixture();
    const run = f.controller.check();
    if (action === "revisit") {
      f.controller.activate({ sessionId: "B", epoch: Symbol() });
      f.controller.activate(f.owner);
    } else if (action === "dispose") f.controller.dispose();
    else f[action]();
    f.calls[0].resolve(ready);
    await run;
    assert.equal(f.calls.length, 1);
    assert.equal(f.results.length, 0);
  }
  for (const action of ["dialog", "disable"]) {
    const f = fixture();
    f[action]();
    await f.controller.check();
    assert.equal(f.calls.length, 0);
  }
});
test("late apply statuses do not show on another tab; repeated failures are deduplicated", async () => {
  const f = fixture();
  const run = f.controller.check();
  f.calls[0].resolve(ready);
  await Promise.resolve();
  f.controller.activate({ sessionId: "B", epoch: Symbol() });
  f.calls[1].resolve({ status: "reloaded", state: {}, token: null });
  await run;
  assert.equal(f.results.length, 0);
  for (let i = 0; i < 2; i++) {
    const retry = f.controller.check();
    f.calls.at(-1).reject(new Error("synthetic read failure"));
    await retry;
  }
  assert.equal(f.errors.length, 1);
  const retry = f.controller.check();
  f.calls.at(-1).resolve({ status: "unchanged", state: null, token: null });
  await retry;
  const failure = f.controller.check();
  f.calls.at(-1).reject(new Error("synthetic read failure"));
  await failure;
  assert.equal(f.errors.length, 2);
});

globalThis.window = globalThis;
let config = {};
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    if (command === "set_config") return (config = structuredClone(args.config));
    if (command === "get_config") return structuredClone(config);
    if (command === "remote_secret_present") return { accessKey: false, secretKey: false };
    throw new Error("Unexpected synthetic command " + command);
  },
};
const { appSettings, normalizeSettings, DEFAULT_APP_SETTINGS } =
  await import("../src/lib/services/settings.ts");
after(() => appSettings.destroy());
test("auto reload defaults off and both values survive actual config publication/reload", async () => {
  assert.equal(DEFAULT_APP_SETTINGS.general.autoReloadChangedFiles, false);
  for (const value of [undefined, {}, { autoReloadChangedFiles: "true" }]) {
    assert.equal(normalizeSettings({ general: value }).general.autoReloadChangedFiles, false);
  }
  for (const enabled of [true, false]) {
    appSettings.updateGeneral("autoReloadChangedFiles", enabled);
    await appSettings.flush();
    assert.equal(config.general.autoReloadChangedFiles, enabled);
    await appSettings.initialize();
    assert.equal(get(appSettings).general.autoReloadChangedFiles, enabled);
  }
});
