import { register } from "node:module";
import { test } from "node:test";
import assert from "node:assert/strict";
import { get } from "svelte/store";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
const storage = new Map();
globalThis.localStorage = {
  getItem: (key) => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, String(value)),
  removeItem: (key) => storage.delete(key),
};
globalThis.window = globalThis;
const { vault } = await import("../src/lib/services/vault.ts");

test("browser session identity is available to first snapshot subscribers and cleared on close/lock", async () => {
  const seen = [];
  const stop = vault.subscribe((value) => {
    seen.push({ path: value?.path ?? null, id: vault.getActiveSessionId() });
  });
  try {
    assert.equal(vault.getActiveSessionId(), null);
    storage.set(
      "secpivot-browser-vault",
      JSON.stringify({
        path: "synthetic-demo.kdbx",
        fileName: "synthetic-demo.kdbx",
        revision: 1,
        dirty: false,
        readOnly: false,
        customIcons: {},
        root: { uuid: "root", name: "Synthetic", parentUuid: null, children: [], entries: [] },
      }),
    );
    await vault.refresh();
    assert.deepEqual(seen.at(-1), { path: "synthetic-demo.kdbx", id: "browser" });
    assert.equal(get(vault.activeId), "browser");
    assert.equal(get(vault.tabs)[0].sessionId, vault.getActiveSessionId());

    await vault.close();
    assert.deepEqual(seen.at(-1), { path: null, id: null });
    assert.equal(get(vault.activeId), null);
    assert.deepEqual(get(vault.tabs), []);

    await vault.open("synthetic-reopened.kdbx", "");
    assert.deepEqual(seen.at(-1), { path: "synthetic-reopened.kdbx", id: "browser" });
    await vault.closeAll();
    assert.deepEqual(seen.at(-1), { path: null, id: null });
    assert.equal(vault.getActiveSessionId(), null);

    await vault.create({ path: "synthetic-created.kdbx", password: "" });
    assert.deepEqual(seen.at(-1), { path: "synthetic-created.kdbx", id: "browser" });
    await vault.closeAll();
    assert.deepEqual(seen.at(-1), { path: null, id: null });
  } finally {
    stop();
    await vault.closeAll();
  }
});
