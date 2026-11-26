import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire, register } from "node:module";
import test from "node:test";
import { pathToFileURL } from "node:url";
import { runInNewContext } from "node:vm";
import * as store from "svelte/store";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
const settings = await import("../src/lib/services/settings.ts");
const { appSettings } = settings;
const require = createRequire(import.meta.url);
const ts = require("typescript");
const { compileModule } = require("svelte/compiler");
const client = await import(pathToFileURL(require.resolve("svelte/internal/client")).href);

// Run complete production selection/layout modules through the real compiler
// and client effects. No DOM, vault, clipboard or native IPC is touched.
function loadModule(path) {
  const js = ts.transpileModule(readFileSync(new URL(path, import.meta.url), "utf8"), {
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
const { usePanelLayout } = loadModule("../src/lib/composables/usePanelLayout.svelte.ts");
const { useVaultSelection } = loadModule("../src/lib/composables/useVaultSelection.svelte.ts");
const entry = (uuid = "A", title = "Synthetic entry") => ({ uuid, title });

function fixture(t, withVersion = true) {
  appSettings.updateGeneral("showDetailOnSelect", true);
  let selection;
  let layout;
  const dispose = client.effect_root(() => {
    selection = useVaultSelection({ visibleUuids: () => ["A", "B"] });
    layout = usePanelLayout({
      entryColumns: () => [],
      urlColWidth: () => 97,
      selectedEntry: () => selection.selectedEntry,
      ...(withVersion ? { selectionVersion: () => selection.selectionVersion } : {}),
    });
  });
  client.flush();
  t.after(() => {
    dispose();
    appSettings.destroy();
  });
  return {
    layout,
    selection,
    select(value) {
      selection.setSingleSelection(value);
      client.flush();
    },
    refresh(value) {
      selection.selectedEntry = value;
      client.flush();
    },
    hide() {
      layout.detailVisible = false;
      client.flush();
    },
    autoShow(value) {
      appSettings.updateGeneral("showDetailOnSelect", value);
      client.flush();
    },
  };
}

test("same UUID snapshots preserve a manually closed detail panel", (t) => {
  const f = fixture(t);
  f.select(entry());
  assert.equal(f.layout.detailVisible, true);
  f.hide();
  const version = f.selection.selectionVersion;
  for (const title of ["Changed favorite", "Changed notes", "Changed icon", "Bridge update"]) {
    f.refresh(entry("A", title));
    assert.equal(f.layout.detailVisible, false);
    assert.equal(f.selection.selectionVersion, version);
  }
});

test("explicit same-entry re-selection opens detail after closing it", (t) => {
  const f = fixture(t);
  const selected = entry();
  f.select(selected);
  f.hide();
  f.select(selected);
  assert.equal(f.layout.detailVisible, true);
  f.hide();
  f.selection.handleRowClick({ shiftKey: false, ctrlKey: false, metaKey: false }, selected);
  client.flush();
  assert.equal(f.layout.detailVisible, true);
});

test("new UUIDs open detail and clearing selection closes it", (t) => {
  const f = fixture(t);
  f.select(entry());
  f.hide();
  f.select(entry("B"));
  assert.equal(f.layout.detailVisible, true);
  f.select(null);
  assert.equal(f.layout.detailVisible, false);
  f.select(entry());
  assert.equal(f.layout.detailVisible, true);
});

test("showDetailOnSelect still controls current selection and explicit re-selection", (t) => {
  const f = fixture(t);
  f.select(entry());
  f.autoShow(false);
  assert.equal(f.layout.detailVisible, false);
  f.select(entry("B"));
  assert.equal(f.layout.detailVisible, false);
  f.select(entry("B"));
  assert.equal(f.layout.detailVisible, false);
  f.autoShow(true);
  assert.equal(f.layout.detailVisible, true);
});

test("right-click suppression survives background refresh and consumes exactly one selection", (t) => {
  const f = fixture(t);
  f.select(entry());
  f.hide();
  f.layout.suppressNextAutoOpen();
  f.refresh(entry("A", "Background update"));
  assert.equal(f.layout.detailVisible, false);
  f.select(entry("B"));
  assert.equal(f.layout.detailVisible, false);
  f.select(entry("B"));
  assert.equal(f.layout.detailVisible, true);
});

test("unrelated settings updates preserve a closed panel", (t) => {
  const f = fixture(t);
  f.select(entry());
  f.hide();
  appSettings.updateGeneral("language", "en");
  client.flush();
  assert.equal(f.layout.detailVisible, false);
});

test("stable UUID gating also works without an optional selection version", (t) => {
  const f = fixture(t, false);
  f.select(entry());
  f.hide();
  f.refresh(entry("A", "Replacement snapshot"));
  assert.equal(f.layout.detailVisible, false);
  f.refresh(entry("B"));
  assert.equal(f.layout.detailVisible, true);
});
