import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire, register } from "node:module";
import test from "node:test";
import { pathToFileURL } from "node:url";
import { runInNewContext } from "node:vm";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
const { prepareAdvancedSearch } = await import("../src/lib/utils/entry-search.ts");
const { collectGroups } = await import("../src/lib/utils/tree.ts");
const require = createRequire(import.meta.url);
const ts = require("typescript");
const { compileModule } = require("svelte/compiler");
const client = await import(pathToFileURL(require.resolve("svelte/internal/client")).href);
const cases = JSON.parse(
  readFileSync(new URL("./fixtures/group-search-snapshots.json", import.meta.url)),
);
const source = readFileSync(
  new URL("../src/lib/composables/useEntryFilter.svelte.ts", import.meta.url),
  "utf8",
);
const js = ts.transpileModule(source, {
  compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext },
}).outputText;
const compiled = compileModule(js, { generate: "client" }).js.code;
const commonjs = ts.transpileModule(compiled, {
  compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.CommonJS },
}).outputText;

// Shared wire projection is independently checked against saved/reopened native
// KDBX snapshots. Only random identities are normalized there. This fixture
// attaches entries/tree links; it never calculates effective search flags.
function snapshot(index) {
  const groups = cases[index].groups.map(({ titles, ...wire }) => ({
    ...wire,
    children: [],
    entries: titles.map((title) => ({
      uuid: title,
      groupUuid: wire.uuid,
      title,
      username: "",
      url: "",
      notes: "",
      tags: "",
    })),
  }));
  for (const group of groups) {
    if (group.parentUuid)
      groups.find((parent) => parent.uuid === group.parentUuid).children.push(group);
  }
  return { root: groups[0] };
}

function fixture(t, index, scope = "Root", search = "entry", advanced = null) {
  const exports = {};
  runInNewContext(commonjs, {
    exports,
    require(name) {
      if (name === "svelte/internal/client") return client;
      if (name === "$lib/utils/entry-search") return { prepareAdvancedSearch };
      throw new Error(`Unexpected compiled import ${name}`);
    },
  });
  const live = client.state(snapshot(index));
  const selected = client.state(scope);
  const query = client.state(search);
  const advancedQuery = client.state(advanced);
  let filter;
  const dispose = client.effect_root(() => {
    filter = exports.useEntryFilter({
      currentVault: () => client.get(live),
      selectedSubtree: () => {
        const state = client.get(live);
        if (!state) return [];
        const group = collectGroups(state.root).find((g) => g.uuid === client.get(selected));
        return group ? collectGroups(group) : [];
      },
      search: () => client.get(query),
      advancedQuery: () => client.get(advancedQuery),
    });
  });
  t.after(dispose);
  return {
    titles: () => [...filter.filteredEntries].map(({ entry }) => entry.title),
    select(value) {
      client.set(selected, value);
      client.flush();
    },
    search(value) {
      client.set(query, value);
      client.flush();
    },
    replace(value) {
      client.set(live, value);
      client.flush();
    },
  };
}

test("inherited disabled group remains excluded when selected directly", (t) => {
  const f = fixture(t, 0, "Synthetic child");
  assert.deepEqual(f.titles(), ["Synthetic leaf entry"]);
  f.select("Synthetic grandchild");
  assert.deepEqual(f.titles(), ["Synthetic leaf entry"]);
});
test("explicit enable overrides disabled ancestors in whole-vault search", (t) => {
  const f = fixture(t, 1);
  assert.deepEqual(f.titles(), ["Synthetic child entry", "Synthetic grandchild entry"]);
});
test("explicit parent overrides disabled root across scope changes", (t) => {
  const f = fixture(t, 2);
  assert.deepEqual(f.titles(), [
    "Synthetic parent entry",
    "Synthetic child entry",
    "Synthetic grandchild entry",
  ]);
  f.select("Synthetic child");
  assert.deepEqual(f.titles(), ["Synthetic child entry", "Synthetic grandchild entry"]);
});
test("explicit disabled leaf is excluded when selected directly", (t) => {
  assert.deepEqual(fixture(t, 1, "Synthetic leaf").titles(), []);
});
test("blank search browses disabled groups without changing their settings", (t) => {
  const f = fixture(t, 0, "Synthetic child", "  ");
  assert.deepEqual(f.titles(), [
    "Synthetic child entry",
    "Synthetic grandchild entry",
    "Synthetic leaf entry",
  ]);
  f.search("child entry");
  assert.deepEqual(f.titles(), []);
});
test("advanced search applies effective flags and the actual advanced matcher", (t) => {
  const advanced = { text: "grandchild", field: "title", regex: false, exclude: false };
  assert.deepEqual(fixture(t, 1, "Root", "", advanced).titles(), ["Synthetic grandchild entry"]);
  assert.deepEqual(fixture(t, 0, "Root", "", advanced).titles(), []);
});
test("new native snapshot invalidates inherited results without changing selected scope", (t) => {
  const f = fixture(t, 0, "Synthetic child");
  assert.deepEqual(f.titles(), ["Synthetic leaf entry"]);
  f.replace(snapshot(1));
  assert.deepEqual(f.titles(), ["Synthetic child entry", "Synthetic grandchild entry"]);
  f.replace(null);
  assert.deepEqual(f.titles(), []);
});
