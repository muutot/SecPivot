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
const { findGroup } = await import("../src/lib/utils/tree.ts");
const g = (uuid, parentUuid, children = [], isRecycleBin = false) => ({
  uuid,
  parentUuid,
  name: uuid,
  children,
  entries: [],
  isRecycleBin,
  searchingSetting: null,
});
async function load() {
  const a = g("a", "root", [g("child", "a")]);
  a.notes = "Synthetic note";
  storage.set(
    "secpivot-browser-vault",
    JSON.stringify({
      path: "synthetic-move.kdbx",
      fileName: "synthetic-move.kdbx",
      revision: 1,
      dirty: false,
      readOnly: false,
      customIcons: {},
      root: g("root", null, [a, g("b", "root"), g("bin", "root", [g("deleted", "bin")], true)]),
    }),
  );
  return vault.open("synthetic-move.kdbx", "");
}
test("real demo service reorders, reparents and saves/reloads the complete subtree", async () => {
  try {
    await load();
    let state = await vault.moveGroup("b", "root", "a");
    assert.deepEqual(
      state.root.children.map((g) => g.uuid),
      ["b", "a", "bin"],
    );
    state = await vault.moveGroup("a", "b");
    assert.equal(findGroup(state.root, "a").parentUuid, "b");
    assert.equal(findGroup(state.root, "child").parentUuid, "a");
    assert.equal(findGroup(state.root, "a").notes, "Synthetic note");
    await vault.save();
    await vault.closeAll();
    await vault.open("synthetic-move.kdbx", "");
    assert.equal(findGroup(vault.get().root, "a").parentUuid, "b");
    state = await vault.moveGroup("a", "root", "b");
    assert.deepEqual(
      state.root.children.map((g) => g.uuid),
      ["a", "b", "bin"],
    );
  } finally {
    await vault.closeAll();
  }
});
test("demo invalid/no-op group moves leave the published state and dirty flag intact", async () => {
  try {
    await load();
    const original = vault.get();
    for (const args of [
      ["a", "child"],
      ["a", "a"],
      ["root", "b"],
      ["a", "bin"],
      ["deleted", "b"],
      ["a", "b", "child"],
      ["missing", "b"],
    ]) {
      await assert.rejects(vault.moveGroup(...args));
      assert.deepEqual(vault.get(), original);
    }
    assert.deepEqual(await vault.moveGroup("a", "root", "b"), original);
    assert.equal(vault.get().dirty, false);
  } finally {
    await vault.closeAll();
  }
});
