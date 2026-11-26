import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire, register } from "node:module";
import test from "node:test";
import { runInNewContext } from "node:vm";
import { get } from "svelte/store";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
globalThis.window = globalThis;
let persisted;
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    if (command === "set_config") {
      persisted = structuredClone(args.config);
      return structuredClone(persisted);
    }
    if (command === "get_config") return structuredClone(persisted);
    if (command === "remote_secret_present") return { accessKey: false, secretKey: false };
    throw new Error(`Unexpected command: ${command}`);
  },
};
const { appSettings } = await import("../src/lib/services/settings.ts");
const query = (text) => ({ text, field: "all", regex: false, exclude: false });

test("saved search names are unique for every immediate subscriber and persisted value", async () => {
  const seen = [];
  const unsubscribe = appSettings.subscribe((value) => {
    const searches = value.general.savedSearches;
    seen.push(searches);
  });
  try {
    appSettings.updateGeneral("savedSearches", [
      { name: " Work ", query: query("first") },
      { name: "Work", query: query("second") },
      { name: "Work", query: query("third") },
    ]);
    const immediate = get(appSettings).general.savedSearches;
    assert.deepEqual(
      immediate.map((search) => search.name),
      ["Work", "Work 2", "Work 3"],
    );
    assert.deepEqual(
      immediate.map((search) => search.query.text),
      ["first", "second", "third"],
    );
    assert.equal(seen.at(-1), immediate, "subscriber must see normalized names before persistence");
    await appSettings.flush();
    assert.deepEqual(persisted.general.savedSearches, immediate);
    assert.deepEqual(get(appSettings).general.savedSearches, immediate);
    for (const searches of seen) {
      assert.equal(new Set(searches.map((search) => search.name)).size, searches.length);
    }
  } finally {
    unsubscribe();
    appSettings.destroy();
  }
});

test("normalization preserves existing suffixes and unnamed queries before publication", async () => {
  try {
    appSettings.updateGeneral("savedSearches", [
      { name: "Work", query: query("first") },
      { name: "Work 2", query: query("existing suffix") },
      { name: " Work ", query: query("duplicate") },
      { name: "", query: query("blank") },
      { query: query("legacy") },
    ]);
    const current = get(appSettings).general.savedSearches;
    assert.deepEqual(
      current.map((search) => search.name),
      ["Work", "Work 2", "Work 3", "Unnamed search", "Unnamed search 2"],
    );
    assert.deepEqual(
      current.map((search) => search.query.text),
      ["first", "existing suffix", "duplicate", "blank", "legacy"],
    );
    await appSettings.flush();
    assert.deepEqual(persisted.general.savedSearches, current);
  } finally {
    appSettings.destroy();
  }
});

// Execute the complete production handlers against the real settings store.
// The browser check additionally mounts the keyed list and tests native input.
const require = createRequire(import.meta.url);
const ts = require("typescript");
const component = readFileSync(
  new URL("../src/lib/components/AdvancedSearchDialog.svelte", import.meta.url),
  "utf8",
);
const script = component.match(/<script lang="ts">([^]*?)<\/script>/)[1];
const ast = ts.createSourceFile("AdvancedSearchDialog.ts", script, ts.ScriptTarget.Latest, true);
const handlers = ast.statements.filter(
  (node) =>
    ts.isFunctionDeclaration(node) && ["saveSearch", "deleteSearch"].includes(node.name?.text),
);
assert.equal(handlers.length, 2);
const js = ts.transpileModule(
  handlers.map((node) => node.getText(ast)).join("\n") +
    "\nexports.saveSearch = saveSearch; exports.deleteSearch = deleteSearch;",
  { compilerOptions: { target: ts.ScriptTarget.ESNext } },
).outputText;

test("dialog saves repeated names, deletes only the chosen query and reloads exact survivors", async () => {
  const context = {
    exports: {},
    appSettings,
    saveName: "Work",
    text: "first",
    field: "all",
    regex: false,
    exclude: false,
    onlyExpired: false,
    onlyFavorites: false,
    tags: "",
    requireQualityCheck: false,
  };
  const unsubscribe = appSettings.subscribe((value) => {
    context.savedSearches = value.general.savedSearches;
  });
  try {
    appSettings.updateGeneral("savedSearches", []);
    runInNewContext(js, context);
    context.exports.saveSearch();
    context.saveName = " Work ";
    context.text = "second";
    context.exports.saveSearch();
    context.saveName = "Other";
    context.text = "unrelated";
    context.exports.saveSearch();
    assert.deepEqual(
      Array.from(context.savedSearches, (search) => search.name),
      ["Work", "Work 2", "Other"],
    );
    context.exports.deleteSearch("Work 2");
    assert.deepEqual(
      Array.from(context.savedSearches, (search) => search.query.text),
      ["first", "unrelated"],
    );
    await appSettings.flush();
    const beforeReload = structuredClone(context.savedSearches);
    assert.deepEqual(persisted.general.savedSearches, beforeReload);
    await appSettings.initialize();
    assert.deepEqual(get(appSettings).general.savedSearches, beforeReload);
  } finally {
    unsubscribe();
    appSettings.destroy();
  }
});
