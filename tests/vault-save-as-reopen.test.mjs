import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";

// Actual service graph; only native invoke and localStorage are synthetic.
// Replies are cloned, and only delivery is delayed. No native files or IPC.
register("./helpers/lib-alias-loader.mjs", import.meta.url);
const storage = new Map();
globalThis.localStorage = {
  getItem: (key) => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, String(value)),
  removeItem: (key) => storage.delete(key),
};
globalThis.window = globalThis;
const sessions = new Map();
let active = null,
  sequence = 0;
const pending = [];
const copy = (value) => structuredClone(value);
function snapshot(path, revision = 1) {
  return {
    path,
    fileName: path.split("/").at(-1),
    revision,
    dirty: false,
    readOnly: false,
    root: { uuid: "root", name: "", children: [], entries: [] },
    customIcons: {},
  };
}
globalThis.__TAURI_INTERNALS__ = {
  invoke(command, args = {}) {
    switch (command) {
      case "open_vault": {
        active = `synthetic-${++sequence}`;
        const state = snapshot(args.path);
        sessions.set(active, state);
        return Promise.resolve(copy({ sessionId: active, state }));
      }
      case "close_all_vaults":
        sessions.clear();
        active = null;
        return Promise.resolve();
      case "list_sessions":
        return Promise.resolve(
          [...sessions]
            .sort(([a], [b]) => Number(b === active) - Number(a === active))
            .map(([sessionId, state]) => ({
              sessionId,
              path: state.path,
              fileName: state.fileName,
              dirty: state.dirty,
            })),
        );
      case "get_vault_state":
        return Promise.resolve(copy(sessions.get(args.sessionId) ?? null));
      case "set_active_session":
        active = args.sessionId;
        return Promise.resolve(copy(sessions.get(active)));
      case "set_config":
        return Promise.resolve(copy(args.config));
      case "clear_saved_credential":
        return Promise.resolve();
      case "save_vault_as": {
        const result = copy(sessions.get(args.sessionId));
        result.path = args.path;
        result.fileName = args.path.split("/").at(-1);
        result.dirty = false;
        sessions.set(args.sessionId, result);
        // The synthetic backend completed; only IPC delivery is delayed.
        return new Promise((resolve, reject) =>
          pending.push({ resolve, reject, result: copy(result), args }),
        );
      }
      case "refresh_remote_vault": {
        const replacement = copy(sessions.get(args.sessionId));
        replacement.databaseName = "Synthetic replacement";
        sessions.set(args.sessionId, replacement);
        return Promise.resolve(copy(replacement));
      }
      case "add_entry": {
        const result = copy(sessions.get(args.sessionId));
        result.revision++;
        result.dirty = true;
        sessions.set(args.sessionId, result);
        return Promise.resolve(copy(result));
      }
      default:
        return Promise.reject(new Error(`Unexpected synthetic command ${command}`));
    }
  },
};
const { vault } = await import("../src/lib/services/vault.ts");
const { appSettings } = await import("../src/lib/services/settings.ts");
const read = (store) => {
  let value;
  const unsubscribe = store.subscribe((v) => (value = v));
  unsubscribe();
  return value;
};
async function fixture() {
  await vault.closeAll();
  pending.length = 0;
  await vault.open("C:/synthetic/original.kdbx", "synthetic-master");
  return vault.getActiveSessionId();
}
const edit = () =>
  vault.addEntry({
    groupUuid: "root",
    title: "Synthetic",
    username: "",
    password: "",
    url: "",
    notes: "",
  });

test("accepted save-as remembers its selected path and provider name", async () => {
  const id = await fixture();
  const saving = vault.saveAs("C:/synthetic/accepted.kdbx", { name: "Selected name.kdbx" });
  assert.equal(pending[0].args.sessionId, id);
  pending[0].resolve(pending[0].result);
  await saving;
  assert.deepEqual(vault.getRemembered(), {
    path: "C:/synthetic/accepted.kdbx",
    fileName: "Selected name.kdbx",
  });
});

test("a rejected older save-as response cannot replace the newer quick-reopen target", async () => {
  await fixture();
  const older = vault.saveAs("C:/synthetic/older.kdbx", { name: "Older label.kdbx" });
  await edit();
  const newer = vault.saveAs("C:/synthetic/newer.kdbx", { name: "Newest label.kdbx" });
  pending[1].resolve(pending[1].result);
  await newer;
  const before = vault.getRemembered();
  pending[0].resolve(pending[0].result);
  assert.equal((await older).path, "C:/synthetic/newer.kdbx");
  assert.equal(vault.get().path, "C:/synthetic/newer.kdbx");
  assert.deepEqual(vault.getRemembered(), before);
  assert.ok(
    read(appSettings).general.recentFiles.includes("C:/synthetic/older.kdbx"),
    "successfully written old destination remains a valid recent file",
  );
});

test("a response rejected after replacement cannot overwrite the current reopen label", async () => {
  const id = await fixture();
  const saving = vault.saveAs("C:/synthetic/replaced.kdbx", { name: "Obsolete label.kdbx" });
  await vault.refreshRemote();
  // Switching back publishes the current replacement's canonical quick-reopen state.
  await vault.open("C:/synthetic/other.kdbx", "synthetic-master");
  await vault.setActiveSession(id);
  const before = vault.getRemembered();
  pending[0].resolve(pending[0].result);
  assert.equal((await saving).databaseName, "Synthetic replacement");
  assert.deepEqual(vault.getRemembered(), before);
});

test("a parked session completion cannot replace the active session reopen target", async () => {
  await fixture();
  const saving = vault.saveAs("C:/synthetic/parked.kdbx");
  await vault.open("C:/synthetic/other.kdbx", "synthetic-master");
  const before = vault.getRemembered();
  pending[0].resolve(pending[0].result);
  await saving;
  assert.deepEqual(vault.getRemembered(), before);
});

test("a closed session save-as reply remains rejected without restoring reopen state", async () => {
  await fixture();
  const outcome = Promise.allSettled([vault.saveAs("C:/synthetic/closed.kdbx")]);
  await vault.closeAll();
  const before = vault.getRemembered();
  pending[0].resolve(pending[0].result);
  assert.equal((await outcome)[0].status, "rejected");
  assert.deepEqual(vault.getRemembered(), before);
});

test("an invoke rejection preserves the existing quick-reopen target", async () => {
  await fixture();
  const before = vault.getRemembered();
  const outcome = Promise.allSettled([vault.saveAs("C:/synthetic/failed.kdbx")]);
  pending[0].reject(new Error("synthetic write refusal"));
  assert.equal((await outcome)[0].status, "rejected");
  assert.deepEqual(vault.getRemembered(), before);
});
