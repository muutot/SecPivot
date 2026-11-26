import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";
register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
globalThis.window = globalThis;
globalThis.localStorage = { getItem: () => null, setItem() {}, removeItem() {} };
const state = (revision) => ({
  revision,
  statusRevision: 0,
  path: "synthetic.kdbx",
  fileName: "synthetic.kdbx",
  dirty: false,
  readOnly: false,
  customIcons: {},
  root: { uuid: "root", parentUuid: null, name: "Root", children: [], entries: [] },
});
const calls = [];
let pending, finish;
window.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    calls.push({ command, args });
    if (command === "open_vault") return { sessionId: "origin", state: state(1) };
    if (command === "list_sessions")
      return [
        { sessionId: "origin", path: "synthetic.kdbx", fileName: "synthetic.kdbx", dirty: false },
      ];
    if (command === "close_all_vaults") return null;
    if (command === "move_group") return pending ?? state(2);
    throw Error(command);
  },
};
const { vault } = await import("../src/lib/services/vault.ts");
test("move_group forwards exact optional anchor and originating session", async () => {
  await vault.open("synthetic.kdbx", "");
  calls.length = 0;
  await vault.callInSession("origin", () => vault.moveGroup("a", "b", "c"));
  assert.deepEqual(calls[0], {
    command: "move_group",
    args: { sessionId: "origin", uuid: "a", parentUuid: "b", beforeUuid: "c" },
  });
  calls.length = 0;
  await vault.moveGroup("a", "root");
  assert.equal(calls[0].args.beforeUuid, null);
});
test("late move completion cannot republish a closed database", async () => {
  pending = new Promise((resolve) => (finish = resolve));
  const moving = vault.moveGroup("a", "b");
  await vault.closeAll();
  finish(state(99));
  await assert.rejects(moving);
  assert.equal(vault.get(), null);
  assert.equal(vault.getActiveSessionId(), null);
});
