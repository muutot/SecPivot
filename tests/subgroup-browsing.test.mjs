import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { createRequire, register } from "node:module";
import { after, test } from "node:test";
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
const { buildVaultTreeIndex, collectGroups } = await import("../src/lib/utils/tree.ts");
const { buildToolbarMenuItems } = await import("../src/lib/utils/menu-items.ts");
after(() => appSettings.destroy());
const require = createRequire(import.meta.url);
const ts = require("typescript");
const { compileModule } = require("svelte/compiler");
const client = await import(pathToFileURL(require.resolve("svelte/internal/client")).href);
function commonjs(source, runes = true) {
  const js = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.ESNext },
  }).outputText;
  return ts.transpileModule(runes ? compileModule(js, { generate: "client" }).js.code : js, {
    compilerOptions: { target: ts.ScriptTarget.ESNext, module: ts.ModuleKind.CommonJS },
  }).outputText;
}
const filterPath = "src/lib/composables/useEntryFilter.svelte.ts";
// Explicit read-only negative comparison; ordinary verification always uses
// the current production source, including the actual page selection effect.
const filterSource = process.env.SECPIVOT_SUBGROUP_COMPARE_REF
  ? execFileSync("git", ["show", `${process.env.SECPIVOT_SUBGROUP_COMPARE_REF}:${filterPath}`], {
      encoding: "utf8",
    })
  : readFileSync(filterPath, "utf8");
const filterCode = commonjs(filterSource);
const selectionCode = commonjs(
  readFileSync("src/lib/composables/useVaultSelection.svelte.ts", "utf8"),
);
const page = readFileSync("src/routes/+page.svelte", "utf8");
const effect = page.match(/  \/\/ A direct-group view[\s\S]*?\n  \}\);/);
assert.ok(effect, "production direct-group selection effect is present");
const effectCode = commonjs(effect[0]);
const action = page.match(/  function handleToolbarMenuAction[\s\S]*?\n  \}\n<\/script>/);
assert.ok(action, "production toolbar action dispatcher is present");
const actionCode = commonjs(action[0].replace(/\n<\/script>$/, ""), false);
const entry = (uuid, groupUuid) => ({
  uuid,
  groupUuid,
  title: "Synthetic " + uuid,
  username: "",
  url: "",
  notes: "",
  tags: "",
  hasTotp: false,
});
const group = (uuid, entries = [], children = [], extra = {}) => ({
  uuid,
  name: uuid,
  entries: entries.map((id) => entry(id, uuid)),
  children,
  enableSearching: true,
  ...extra,
});
const tree = () => ({
  root: group(
    "root",
    ["root-entry"],
    [
      group(
        "branch",
        ["direct"],
        [group("child", ["descendant"], [group("deep", ["deep-entry"])])],
      ),
      group("empty", [], [group("empty-child", ["under-empty"])]),
      group("excluded", ["search-disabled"], [], { enableSearching: false }),
      group("bin", ["deleted"], [group("bin-child", ["nested-deleted"])], { isRecycleBin: true }),
    ],
  ),
});
function fixture(t) {
  appSettings.updateGeneral("showEntriesOfSubGroups", false);
  const data = client.state(tree());
  const selected = client.state("branch");
  const query = client.state("");
  const advanced = client.state(null);
  const settings = client.state(get(appSettings));
  const unsubscribe = appSettings.subscribe((value) => client.set(settings, value));
  let filter, selection;
  const env = {
    exports: {},
    require(name) {
      if (name === "svelte/internal/client") return client;
      if (name === "$lib/utils/entry-search") return { prepareAdvancedSearch };
      throw new Error("Unexpected compiled import " + name);
    },
  };
  const filterExports = {};
  runInNewContext(filterCode, { ...env, exports: filterExports });
  const selectionExports = {};
  runInNewContext(selectionCode, { ...env, exports: selectionExports });
  const dispose = client.effect_root(() => {
    filter = filterExports.useEntryFilter({
      currentVault: () => client.get(data),
      selectedSubtree: () => {
        const vault = client.get(data);
        if (!vault) return [];
        const index = buildVaultTreeIndex(vault.root);
        const id = client.get(selected);
        if (id === null) return index.groups.filter((g) => !index.recycleBinUuids.has(g.uuid));
        const chosen = index.groupByUuid.get(id);
        return chosen ? collectGroups(chosen) : index.groups;
      },
      selectedGroup: () => client.get(selected),
      showEntriesOfSubGroups: () => client.get(settings).general.showEntriesOfSubGroups,
      search: () => client.get(query),
      advancedQuery: () => client.get(advanced),
    });
    selection = selectionExports.useVaultSelection({
      visibleUuids: () => filter.filteredEntries.map((row) => row.entry.uuid),
    });
    runInNewContext(effectCode, {
      ...env,
      untrack: client.untrack,
      selection,
      get selectedGroup() {
        return client.get(selected);
      },
      get settings() {
        return client.get(settings);
      },
      get search() {
        return client.get(query);
      },
      get advancedQuery() {
        return client.get(advanced);
      },
      get sortedEntries() {
        return filter.filteredEntries;
      },
    });
  });
  client.flush();
  t.after(() => {
    unsubscribe();
    dispose();
  });
  return {
    selection,
    ids: () => Array.from(filter.filteredEntries, (row) => row.entry.uuid),
    group(value) {
      client.set(selected, value);
      client.flush();
    },
    search(value) {
      client.set(query, value);
      client.flush();
    },
    advanced(value) {
      client.set(advanced, value);
      client.flush();
    },
    enabled(value) {
      appSettings.updateGeneral("showEntriesOfSubGroups", value);
      client.flush();
    },
    replace(value) {
      client.set(data, value);
      client.flush();
    },
    choose(id) {
      selection.setSingleSelection(buildVaultTreeIndex(client.get(data).root).entryByUuid.get(id));
    },
    toggleFromMenu() {
      const context = {
        appSettings,
        get settings() {
          return client.get(settings);
        },
      };
      runInNewContext(
        actionCode + '\nhandleToolbarMenuAction("toggle-subgroup-entries");',
        context,
      );
      client.flush();
    },
  };
}

test("concrete browsing starts with direct entries and toggles descendants in real tree order", (t) => {
  const f = fixture(t);
  assert.deepEqual(f.ids(), ["direct"]);
  f.enabled(true);
  assert.deepEqual(f.ids(), ["direct", "descendant", "deep-entry"]);
  f.group("empty");
  assert.deepEqual(f.ids(), ["under-empty"]);
  f.enabled(false);
  assert.deepEqual(f.ids(), []);
  f.group("child");
  assert.deepEqual(f.ids(), ["descendant"]);
  f.group("root");
  assert.deepEqual(f.ids(), ["root-entry"]);
});
test("virtual All entries, selected recycle-bin scopes and searches remain independent", (t) => {
  const f = fixture(t);
  f.group(null);
  assert.deepEqual(f.ids(), [
    "root-entry",
    "direct",
    "descendant",
    "deep-entry",
    "under-empty",
    "search-disabled",
  ]);
  f.enabled(true);
  assert.equal(f.ids().length, 6);
  f.group("bin");
  assert.deepEqual(f.ids(), ["deleted", "nested-deleted"]);
  f.enabled(false);
  assert.deepEqual(f.ids(), ["deleted"]);
  f.group("branch");
  f.search("synthetic");
  assert.deepEqual(f.ids(), ["direct", "descendant", "deep-entry"]);
  f.search("");
  f.advanced({ text: "deep", field: "title", regex: false, exclude: false });
  assert.deepEqual(f.ids(), ["deep-entry"]);
  f.group(null);
  f.advanced(null);
  f.search("synthetic");
  assert.equal(f.ids().length, 5, "search inheritance still excludes the disabled group");
});
test("actual page effect retains visible selections and clears hidden focus and anchors", (t) => {
  const f = fixture(t);
  f.enabled(true);
  f.choose("descendant");
  f.selection.selectedUuids = new Set(["direct", "descendant"]);
  f.enabled(false);
  assert.deepEqual([...f.selection.selectedUuids], ["direct"]);
  assert.equal(f.selection.selectedEntry, null);
  assert.equal(f.selection.selectionAnchor, null);
  f.choose("direct");
  f.enabled(true);
  f.enabled(false);
  assert.equal(f.selection.selectedEntry.uuid, "direct");
  f.search("synthetic");
  f.choose("deep-entry");
  f.enabled(true);
  f.enabled(false);
  assert.equal(f.selection.selectedEntry.uuid, "deep-entry", "search results keep their focus");
  f.selection.selectedUuids = new Set();
  f.search("");
  assert.equal(f.selection.selectedEntry, null, "Ctrl-deselected hidden focus is also cleared");
});
test("snapshot replacement and close never retain hidden direct-group selections", (t) => {
  const f = fixture(t);
  f.choose("direct");
  f.replace({ root: group("root", [], [group("branch", ["replacement"])]) });
  assert.deepEqual(f.ids(), ["replacement"]);
  assert.equal(f.selection.selectedEntry, null);
  assert.equal(f.selection.selectedUuids.size, 0);
  f.choose("replacement");
  f.replace(null);
  assert.deepEqual(f.ids(), []);
  assert.equal(f.selection.selectedEntry, null);
});
test("View cascade and actual page dispatcher share the persisted setting with both checked states", (t) => {
  const f = fixture(t);
  for (const enabled of [false, true]) {
    f.enabled(enabled);
    const items = buildToolbarMenuItems({
      detailVisible: true,
      busy: false,
      locale: "en",
      showEntriesOfSubGroups: enabled,
    });
    const view = items.find((item) => item.id === "view");
    assert.equal(view.label, "View");
    assert.equal(view.children[0].id, "toggle-subgroup-entries");
    assert.equal(view.children[0].checked, enabled);
    assert.equal(view.children[0].label, "Show entries of subgroups");
    f.toggleFromMenu();
    assert.equal(get(appSettings).general.showEntriesOfSubGroups, !enabled);
    assert.deepEqual(f.ids(), enabled ? ["direct"] : ["direct", "descendant", "deep-entry"]);
  }
});
test("old/invalid settings default off and both values survive real publication and reload", async () => {
  assert.equal(DEFAULT_GENERAL_SETTINGS.showEntriesOfSubGroups, false);
  for (const general of [
    undefined,
    {},
    { showEntriesOfSubGroups: "true" },
    { showEntriesOfSubGroups: 1 },
  ]) {
    assert.equal(normalizeSettings({ general }).general.showEntriesOfSubGroups, false);
  }
  for (const enabled of [true, false]) {
    appSettings.updateGeneral("showEntriesOfSubGroups", enabled);
    await appSettings.flush();
    assert.equal(nativeConfig.general.showEntriesOfSubGroups, enabled);
    await appSettings.initialize();
    assert.equal(get(appSettings).general.showEntriesOfSubGroups, enabled);
  }
});
