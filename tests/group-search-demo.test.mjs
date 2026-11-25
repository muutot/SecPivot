import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
const storage = new Map();
globalThis.localStorage = {
  getItem: (key) => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, String(value)),
  removeItem: (key) => storage.delete(key),
};
globalThis.window = globalThis;
const { vault } = await import("../src/lib/services/vault.ts");
const { collectGroups } = await import("../src/lib/utils/tree.ts");
const group = (state, uuid) => collectGroups(state.root).find((g) => g.uuid === uuid);
const wireGroup = (uuid, searchingSetting, children = []) => ({
  uuid,
  name: uuid,
  parentUuid: uuid === "root" ? null : "root",
  searchingSetting,
  enableSearching: true,
  isRecycleBin: false,
  children,
  entries: [],
});
async function load(root) {
  storage.set(
    "secpivot-browser-vault",
    JSON.stringify({
      path: "synthetic-demo.kdbx",
      fileName: "synthetic-demo.kdbx",
      revision: 0,
      dirty: false,
      readOnly: false,
      customIcons: {},
      root,
    }),
  );
  return vault.open("synthetic-demo.kdbx", "");
}

test("demo additions inherit and parent edits propagate while explicit children override", async () => {
  try {
    await load(wireGroup("root", null, [wireGroup("parent", false)]));
    let state = await vault.addGroup({ parentUuid: "parent", name: "child" });
    const child = collectGroups(state.root).find((g) => g.name === "child");
    assert.equal(child.searchingSetting, null);
    assert.equal(child.enableSearching, false);
    state = await vault.updateGroupMeta(child.uuid, { notes: "Synthetic note" });
    assert.equal(group(state, child.uuid).searchingSetting, null);
    state = await vault.updateGroupMeta("parent", { enableSearching: true });
    assert.equal(group(state, child.uuid).enableSearching, true);
    state = await vault.updateGroupMeta(child.uuid, { enableSearching: true });
    state = await vault.updateGroupMeta("parent", { enableSearching: false });
    assert.equal(group(state, child.uuid).enableSearching, true);
    assert.equal(group(state, child.uuid).searchingSetting, true);
    state = await vault.updateGroupMeta(child.uuid, { enableSearching: false });
    assert.equal(group(state, child.uuid).enableSearching, false);
  } finally {
    await vault.closeAll();
  }
});
test("legacy demo booleans retain explicit meaning through load and metadata edits", async () => {
  try {
    const child = wireGroup("legacy", undefined);
    child.enableSearching = true;
    const parent = wireGroup("parent", false, [child]);
    child.parentUuid = "parent";
    let state = await load(wireGroup("root", null, [parent]));
    assert.equal(group(state, "legacy").searchingSetting, true);
    assert.equal(group(state, "legacy").enableSearching, true);
    state = await vault.updateGroupMeta("legacy", { notes: "Synthetic note" });
    assert.equal(group(state, "legacy").searchingSetting, true);
    assert.equal(group(state, "legacy").enableSearching, true);
  } finally {
    await vault.closeAll();
  }
});
