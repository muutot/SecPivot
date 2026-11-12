import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";

// Actual service graph; only invoke/storage are synthetic. Deliveries retain
// the snapshot captured before a later save or mutation completed.
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
  sequence = 0,
  delayNextRead = false,
  delayNextSave = false;
const pending = [];
const copy = (value) => structuredClone(value);
globalThis.__TAURI_INTERNALS__ = {
  invoke(command, args = {}) {
    switch (command) {
      case "open_vault": {
        active = `synthetic-status-${++sequence}`;
        const state = {
          path: args.path,
          fileName: "synthetic-status.kdbx",
          revision: 1,
          statusRevision: 0,
          dirty: false,
          readOnly: false,
          mirrorWarning: null,
          modifiedAt: "synthetic-time",
          root: { uuid: "root", name: "", children: [], entries: [] },
          customIcons: {},
        };
        sessions.set(active, state);
        return Promise.resolve(copy({ sessionId: active, state }));
      }
      case "close_all_vaults":
        sessions.clear();
        active = null;
        return Promise.resolve();
      case "list_sessions":
        return Promise.resolve(
          [...sessions].map(([sessionId, state]) => ({
            sessionId,
            path: state.path,
            fileName: state.fileName,
            dirty: state.dirty,
          })),
        );
      case "get_vault_state": {
        const captured = copy(sessions.get(args.sessionId) ?? null);
        if (delayNextRead) {
          delayNextRead = false;
          return new Promise((resolve) => pending.push({ resolve, captured, args }));
        }
        return Promise.resolve(captured);
      }
      case "set_config":
        return Promise.resolve(copy(args.config));
      case "clear_saved_credential":
        return Promise.resolve();
      case "add_entry": {
        const state = copy(sessions.get(args.sessionId));
        state.revision++;
        state.dirty = true;
        sessions.set(args.sessionId, state);
        return Promise.resolve(copy(state));
      }
      case "save_vault":
      case "change_master_key": {
        const state = copy(sessions.get(args.sessionId));
        state.statusRevision++;
        state.dirty = false;
        state.mirrorWarning = null;
        sessions.set(args.sessionId, state);
        const captured = copy(state);
        if (delayNextSave) {
          delayNextSave = false;
          return new Promise((resolve) => pending.push({ resolve, captured, args }));
        }
        return Promise.resolve(captured);
      }
      case "set_groups_expanded": {
        const state = copy(sessions.get(args.sessionId));
        state.revision++;
        sessions.set(args.sessionId, state);
        return Promise.resolve({
          kind: "groupsExpanded",
          revision: state.revision,
          statusRevision: state.statusRevision,
          dirty: state.dirty,
          readOnly: state.readOnly,
          mirrorWarning: state.mirrorWarning,
          modifiedAt: state.modifiedAt,
          groups: Object.fromEntries(args.uuids.map((uuid) => [uuid, args.expanded])),
        });
      }
      default:
        return Promise.reject(new Error(`Unexpected synthetic command ${command}`));
    }
  },
};
const { vault } = await import("../src/lib/services/vault.ts");
async function edit() {
  return vault.addEntry({
    groupUuid: "root",
    title: "Synthetic",
    username: "",
    password: "",
    url: "",
    notes: "",
  });
}
async function fixture() {
  await vault.closeAll();
  pending.length = 0;
  delayNextRead = false;
  delayNextSave = false;
  await vault.open("C:/synthetic/synthetic-status.kdbx", "synthetic-status-only");
  await edit();
  return vault.getActiveSessionId();
}
function delayRead() {
  delayNextRead = true;
  const reading = vault.refresh();
  assert.equal(pending.length, 1);
  return reading;
}
async function deliverPrior(reading) {
  pending[0].resolve(copy(pending[0].captured));
  await reading;
}
test("actual service keeps clean save status after an older same-edit-version read", async () => {
  await fixture();
  const reading = delayRead();
  const saved = await vault.save();
  assert.equal(saved.dirty, false);
  await deliverPrior(reading);
  assert.equal(vault.get().dirty, false);
  assert.equal(vault.get().statusRevision, saved.statusRevision);
});
test("actual service keeps read-only state after an older same-edit-version read", async () => {
  const id = await fixture();
  const reading = delayRead();
  sessions.get(id).readOnly = true;
  sessions.get(id).statusRevision++;
  await vault.refresh();
  assert.equal(vault.get().readOnly, true);
  await deliverPrior(reading);
  assert.equal(vault.get().readOnly, true);
});
test("actual service keeps cleared mirror warning after an older same-edit-version read", async () => {
  const id = await fixture();
  sessions.get(id).mirrorWarning = "Synthetic previous warning";
  await vault.refresh();
  const reading = delayRead();
  await vault.save();
  await deliverPrior(reading);
  assert.equal(vault.get().mirrorWarning, null);
});
test("actual service keeps clean master-change status after an older same-edit-version read", async () => {
  await fixture();
  const reading = delayRead();
  const changed = await vault.changeMasterKey("synthetic-new-status-only", null);
  await deliverPrior(reading);
  assert.equal(vault.get().dirty, false);
  assert.equal(vault.get().statusRevision, changed.statusRevision);
});
test("actual service still refuses an older edit version", async () => {
  await fixture();
  const reading = delayRead();
  const changed = await edit();
  await deliverPrior(reading);
  assert.equal(vault.get().revision, changed.revision);
});
test("normal save with no delayed reply remains clean", async () => {
  await fixture();
  const saved = await vault.save();
  assert.equal(saved.dirty, false);
  assert.equal(vault.get().dirty, false);
});
test("a delta after save carries clean status before the save reply arrives", async () => {
  await fixture();
  delayNextSave = true;
  const saving = vault.save();
  assert.equal(pending.length, 1);
  await vault.setGroupsExpanded(["root"], false);
  assert.equal(vault.get().root.isExpanded, false);
  assert.equal(vault.get().dirty, false);
  assert.equal(vault.get().statusRevision, 1);
  const newer = vault.get().revision;
  await deliverPrior(saving);
  assert.equal(vault.get().revision, newer);
  assert.equal(vault.get().dirty, false);
});
test("a rejected stale same-edit-version read cannot replace icon cache", async () => {
  const id = await fixture();
  sessions.get(id).customIcons = { synthetic: "data:old-synthetic" };
  const reading = delayRead();
  sessions.get(id).statusRevision++;
  sessions.get(id).customIcons = { synthetic: "data:new-synthetic" };
  await vault.refresh();
  await deliverPrior(reading);
  assert.deepEqual(vault.get().customIcons, { synthetic: "data:new-synthetic" });
  // An icon-omitting mutation reads the cache; it must retain the newer value.
  sessions.get(id).customIcons = undefined;
  await edit();
  assert.deepEqual(vault.get().customIcons, { synthetic: "data:new-synthetic" });
});
