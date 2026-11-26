import assert from "node:assert/strict";
import { register } from "node:module";
import { test, after } from "node:test";
import { get } from "svelte/store";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.window = globalThis;
const sessions = new Map(),
  pending = [];
let active = null,
  sequence = 0,
  deferPrepare = false,
  deferApply = false,
  deferRead = false;
const copy = (value) => structuredClone(value);
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args = {}) => {
    if (command === "set_config") return args.config;
    if (command === "clear_saved_credential") return;
    if (command === "open_vault") {
      active = "synthetic-" + ++sequence;
      const state = {
        path: args.path,
        fileName: "synthetic.kdbx",
        revision: 1,
        dirty: false,
        readOnly: false,
        root: { uuid: "root", name: "", children: [], entries: [] },
        customIcons: {},
      };
      sessions.set(active, state);
      return { sessionId: active, state: copy(state) };
    }
    if (command === "list_sessions")
      return [...sessions]
        .sort(([a], [b]) => Number(b === active) - Number(a === active))
        .map(([sessionId, state]) => ({
          sessionId,
          path: state.path,
          fileName: state.fileName,
          dirty: state.dirty,
        }));
    if (command === "set_active_session") {
      active = args.sessionId;
      return copy(sessions.get(active));
    }
    if (command === "close_all_vaults") {
      sessions.clear();
      active = null;
      return;
    }
    if (command === "close_vault") {
      sessions.delete(args.sessionId);
      if (active === args.sessionId) active = [...sessions.keys()].at(-1) ?? null;
      return;
    }
    if (command === "get_vault_state") {
      const state = copy(sessions.get(args.sessionId) ?? null);
      if (deferRead)
        return new Promise((resolve) => pending.push({ command, args, resolve, result: state }));
      return state;
    }
    if (command === "reload_changed_vault") {
      const result = { status: "ready", token: "synthetic-token", state: null };
      if (deferPrepare)
        return new Promise((resolve) => pending.push({ command, args, resolve, result }));
      return result;
    }
    if (command === "apply_changed_vault") {
      const state = copy(sessions.get(args.sessionId));
      state.revision++;
      state.databaseName = "synthetic external version";
      sessions.set(args.sessionId, state);
      const result = { status: "reloaded", token: null, state };
      if (deferApply)
        return new Promise((resolve) => pending.push({ command, args, resolve, result }));
      return result;
    }
    throw new Error("Unexpected synthetic command " + command);
  },
};
const { vault, fileReloadApplying } = await import("../src/lib/services/vault.ts");
const { appSettings } = await import("../src/lib/services/settings.ts");
after(() => appSettings.destroy());
async function reset() {
  deferRead = deferApply = deferPrepare = false;
  await vault.closeAll();
  await vault.open("C:/synthetic/first.kdbx", "synthetic-only");
}
test("actual service stages without publication and apply renews the epoch to reject older reads", async () => {
  await reset();
  const id = vault.getActiveSessionId(),
    oldEpoch = vault.getSessionEpoch(id);
  const old = vault.get();
  const ready = await vault.prepareChangedFile();
  assert.equal(ready.token, "synthetic-token");
  assert.equal(vault.get(), old);
  assert.equal(vault.getSessionEpoch(id), oldEpoch);
  deferRead = true;
  const reading = vault.refresh();
  const delivered = pending.shift();
  deferRead = false;
  const result = await vault.applyChangedFile(ready.token);
  assert.equal(result.state.databaseName, "synthetic external version");
  assert.notEqual(vault.getSessionEpoch(id), oldEpoch);
  delivered.resolve(delivered.result);
  await reading;
  assert.equal(vault.get().databaseName, "synthetic external version");
});
test("the commit lease spans native IPC and is released on closed-session refusal", async () => {
  await reset();
  deferApply = true;
  const applying = Promise.allSettled([vault.applyChangedFile("synthetic-token")]);
  assert.equal(get(fileReloadApplying), true);
  const delivered = pending.shift();
  await vault.closeAll();
  delivered.resolve(delivered.result);
  const [result] = await applying;
  assert.equal(result.status, "rejected");
  assert.equal(vault.get(), null);
  assert.equal(get(fileReloadApplying), false);
});
test("a closed preparation never returns a usable candidate", async () => {
  await reset();
  deferPrepare = true;
  const preparing = Promise.allSettled([vault.prepareChangedFile()]);
  const delivered = pending.shift();
  await vault.closeAll();
  delivered.resolve(delivered.result);
  const [result] = await preparing;
  assert.equal(result.status, "rejected");
  assert.equal(vault.get(), null);
});
test("an explicitly addressed parked session reload cannot replace the visible tab", async () => {
  await reset();
  const first = vault.getActiveSessionId();
  await vault.open("C:/synthetic/second.kdbx", "synthetic-only");
  const visible = vault.getActiveSessionId(),
    before = vault.get();
  await vault.callInSession(first, () => vault.applyChangedFile("synthetic-token"));
  assert.equal(vault.getActiveSessionId(), visible);
  assert.equal(vault.get(), before);
  await vault.setActiveSession(first);
  assert.equal(vault.get().databaseName, "synthetic external version");
});
