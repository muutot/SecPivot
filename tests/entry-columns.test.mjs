import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire, register } from "node:module";
import test from "node:test";
import { pathToFileURL } from "node:url";
import { runInNewContext } from "node:vm";
import { get } from "svelte/store";
import * as store from "svelte/store";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
globalThis.window = globalThis;
let persisted;
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    assert.equal(command, "set_config");
    persisted = structuredClone(args.config);
    return structuredClone(persisted);
  },
};
const settings = await import("../src/lib/services/settings.ts");
const date = await import("../src/lib/utils/date.ts");
const format = await import("../src/lib/utils/format.ts");
const i18n = await import("../src/lib/i18n/index.ts");
const { appSettings } = settings;
const require = createRequire(import.meta.url);
const ts = require("typescript");
const { compileModule } = require("svelte/compiler");
const client = await import(pathToFileURL(require.resolve("svelte/internal/client")).href);

// Compile entire production modules and run their real reactive state and
// subscriptions. Only config IPC is synthetic; no DOM or pointer API is used.
function loadModule(path) {
  const source = readFileSync(new URL(path, import.meta.url), "utf8");
  const js = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext },
  }).outputText;
  const compiled = compileModule(js, { generate: "client" }).js.code;
  const commonjs = ts.transpileModule(compiled, {
    compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.CommonJS },
  }).outputText;
  const exports = {};
  const imports = {
    "svelte/internal/client": client,
    "svelte/store": store,
    "$lib/services/settings": settings,
    "$lib/utils/date": date,
    "$lib/utils/format": format,
    "$lib/i18n": i18n,
  };
  runInNewContext(commonjs, {
    exports,
    require(name) {
      assert.ok(Object.hasOwn(imports, name), `Unexpected compiled import: ${name}`);
      return imports[name];
    },
  });
  return exports;
}
const { useEntryColumns } = loadModule("../src/lib/services/columns.svelte.ts");
const { usePanelLayout } = loadModule("../src/lib/composables/usePanelLayout.svelte.ts");

function fixture(t) {
  const ids = ["title", "password", "username", "notes", "custom:Project", "tags", "url"];
  const visible = ["title", "username", "custom:Project", "url"];
  const initial = ids.map((id) => ({
    id,
    visible: visible.includes(id),
    width: id === "title" ? 0 : 97,
  }));
  initial.push(
    ...get(appSettings)
      .general.entryColumns.filter((col) => !ids.includes(col.id))
      .map((col) => ({ ...col, visible: false })),
  );
  appSettings.merge({ general: { ...get(appSettings).general, entryColumns: initial } });
  let columns;
  let layout;
  const dispose = client.effect_root(() => {
    columns = useEntryColumns(() => []);
    layout = usePanelLayout({
      entryColumns: () => columns.entryColumns,
      urlColWidth: () => 97,
      selectedEntry: () => null,
    });
  });
  client.flush();
  t.after(() => {
    dispose();
    appSettings.destroy();
  });
  return {
    columns,
    visible: () => Array.from(columns.visibleCols, (col) => col.id),
    hidden: () =>
      Array.from(columns.entryColumns)
        .filter((col) => !col.visible)
        .map((col) => col.id),
    save: async () => {
      layout.saveLayout();
      await appSettings.flush();
      client.flush();
    },
    reload: () => {
      appSettings.merge(persisted);
      client.flush();
    },
  };
}

test("visible-column insertion indices ignore intervening hidden columns", async (t) => {
  const f = fixture(t);
  const hidden = f.hidden();
  f.columns.applyColumnReorder("username", 3);
  assert.deepEqual(f.visible(), ["title", "custom:Project", "username", "url"]);
  assert.deepEqual(f.hidden(), hidden);
  await f.save();
  assert.deepEqual(
    persisted.general.entryColumns.filter((col) => col.visible).map((col) => col.id),
    f.visible(),
  );
  f.reload();
  assert.deepEqual(f.visible(), ["title", "custom:Project", "username", "url"]);
  assert.deepEqual(f.hidden(), hidden);
  assert.equal(f.columns.entryColumns.find((col) => col.id === "title").width, 0);
  assert.equal(f.columns.entryColumns.find((col) => col.id === "custom:Project").width, 97);
});

test("dropping before itself or its next visible neighbor leaves the full order unchanged", (t) => {
  const f = fixture(t);
  const before = f.columns.entryColumns;
  for (const index of [1, 2]) {
    f.columns.applyColumnReorder("username", index);
    assert.equal(f.columns.entryColumns, before);
    assert.deepEqual(f.visible(), ["title", "username", "custom:Project", "url"]);
  }
});

test("custom columns move backward and to the end while hidden columns retain order", async (t) => {
  const f = fixture(t);
  const hidden = f.hidden();
  f.columns.applyColumnReorder("custom:Project", 1);
  assert.deepEqual(f.visible(), ["title", "custom:Project", "username", "url"]);
  f.columns.applyColumnReorder("custom:Project", 4);
  assert.deepEqual(f.visible(), ["title", "username", "url", "custom:Project"]);
  assert.deepEqual(f.hidden(), hidden);
  await f.save();
  f.reload();
  assert.deepEqual(f.visible(), ["title", "username", "url", "custom:Project"]);
  assert.deepEqual(f.hidden(), hidden);
});
