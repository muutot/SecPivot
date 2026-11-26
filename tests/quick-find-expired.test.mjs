import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire, register } from "node:module";
import { test, after } from "node:test";
import { pathToFileURL } from "node:url";
import { runInNewContext } from "node:vm";
import { get } from "svelte/store";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.window = globalThis;
let nativeConfig = {};
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    if (command === "set_config") return (nativeConfig = structuredClone(args.config));
    if (command === "get_config") return structuredClone(nativeConfig);
    if (command === "remote_secret_present") return { accessKey: false, secretKey: false };
    throw new Error("Unexpected synthetic command " + command);
  },
};
const { appSettings, normalizeSettings, DEFAULT_GENERAL_SETTINGS } =
  await import("../src/lib/services/settings.ts");
const { prepareAdvancedSearch } = await import("../src/lib/utils/entry-search.ts");
after(() => appSettings.destroy());
const require = createRequire(import.meta.url);
const ts = require("typescript");
const { compileModule } = require("svelte/compiler");
const client = await import(pathToFileURL(require.resolve("svelte/internal/client")).href);
const source = readFileSync("src/lib/composables/useEntryFilter.svelte.ts", "utf8");
const js = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext },
}).outputText;
const compiled = compileModule(js, { generate: "client" }).js.code;
const commonjs = ts.transpileModule(compiled, {
  compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.CommonJS },
}).outputText;
const NOW = Date.parse("2026-11-27T10:00:00Z");
class SearchDate extends Date {
  static now() {
    return NOW;
  }
}
const row = (uuid, overrides = {}) => ({
  uuid,
  groupUuid: "root",
  title: "Synthetic " + uuid,
  username: "",
  url: "",
  notes: "",
  hasTotp: false,
  ...overrides,
});
function fixture(t) {
  const exports = {};
  runInNewContext(commonjs, {
    exports,
    Date: SearchDate,
    require(name) {
      if (name === "svelte/internal/client") return client;
      if (name === "$lib/utils/entry-search") return { prepareAdvancedSearch };
      throw new Error("Unexpected compiled import " + name);
    },
  });
  const data = client.state({
    root: {
      uuid: "root",
      enableSearching: true,
      children: [],
      entries: [
        row("past", { expires: "2026-11-27T09:59:59Z", expired: false }),
        row("boundary", { expires: "2026-11-27T10:00:00Z", expired: false }),
        row("offset", { expires: "2026-11-27T18:00:00+08:00", expired: false }),
        row("future", { expires: "2026-11-27T10:00:01Z", expired: true }),
        row("permanent"),
        row("legacy-expired", { expired: true }),
      ],
    },
  });
  const query = client.state("synthetic");
  const advanced = client.state(null);
  const settings = client.state(get(appSettings));
  const unsubscribe = appSettings.subscribe((value) => client.set(settings, value));
  let filter;
  const dispose = client.effect_root(() => {
    filter = exports.useEntryFilter({
      currentVault: () => client.get(data),
      selectedSubtree: () => (client.get(data) ? [client.get(data).root] : []),
      search: () => client.get(query),
      advancedQuery: () => client.get(advanced),
      quickFindExcludeExpired: () => client.get(settings).general.quickFindExcludeExpired,
    });
  });
  t.after(() => {
    unsubscribe();
    dispose();
  });
  return {
    ids: () => filter.filteredEntries.map(({ entry }) => entry.uuid),
    query(value) {
      client.set(query, value);
      client.flush();
    },
    advanced(value) {
      client.set(advanced, value);
      client.flush();
    },
    enabled(value) {
      appSettings.updateGeneral("quickFindExcludeExpired", value);
      client.flush();
    },
    replace(value) {
      client.set(data, value);
      client.flush();
    },
  };
}
test("actual quick-search derivation responds to settings and reevaluates expiration at the official <= boundary", (t) => {
  appSettings.updateGeneral("quickFindExcludeExpired", false);
  const f = fixture(t);
  assert.deepEqual(
    [...f.ids()],
    ["past", "boundary", "offset", "future", "permanent", "legacy-expired"],
  );
  f.enabled(true);
  assert.deepEqual([...f.ids()], ["future", "permanent"]);
  f.query("PERMANENT");
  assert.deepEqual([...f.ids()], ["permanent"]);
  f.query("synthetic");
  f.enabled(false);
  assert.equal(f.ids().length, 6);
});
test("blank browsing and advanced expiration searches remain independent", (t) => {
  const f = fixture(t);
  f.enabled(true);
  for (const text of ["", "  "]) {
    f.query(text);
    assert.equal(f.ids().length, 6);
  }
  f.advanced({
    text: "synthetic",
    field: "title",
    regex: false,
    exclude: false,
    onlyExpired: true,
  });
  assert.deepEqual([...f.ids()], ["future", "legacy-expired"]);
  f.enabled(false);
  assert.deepEqual([...f.ids()], ["future", "legacy-expired"]);
  f.advanced({ text: "boundary", field: "title", regex: false, exclude: false });
  assert.deepEqual([...f.ids()], ["boundary"]);
});
test("search-disabled groups and replacement/close boundaries keep their existing semantics", (t) => {
  const f = fixture(t);
  f.enabled(true);
  f.replace({
    root: { uuid: "root", enableSearching: false, entries: [row("permanent")], children: [] },
  });
  assert.deepEqual([...f.ids()], []);
  f.query("");
  assert.deepEqual([...f.ids()], ["permanent"]);
  f.query("synthetic");
  f.replace({
    root: {
      uuid: "root",
      enableSearching: true,
      entries: [row("replacement", { expires: "2026-11-27T10:00:01Z" })],
      children: [],
    },
  });
  assert.deepEqual([...f.ids()], ["replacement"]);
  f.replace(null);
  assert.deepEqual([...f.ids()], []);
});
test("default/invalid config and both explicit values survive actual publication and reload", async () => {
  assert.equal(DEFAULT_GENERAL_SETTINGS.quickFindExcludeExpired, false);
  for (const general of [
    undefined,
    {},
    { quickFindExcludeExpired: "true" },
    { quickFindExcludeExpired: 1 },
  ]) {
    assert.equal(normalizeSettings({ general }).general.quickFindExcludeExpired, false);
  }
  for (const enabled of [true, false]) {
    appSettings.updateGeneral("quickFindExcludeExpired", enabled);
    await appSettings.flush();
    assert.equal(nativeConfig.general.quickFindExcludeExpired, enabled);
    await appSettings.initialize();
    assert.equal(get(appSettings).general.quickFindExcludeExpired, enabled);
  }
});
