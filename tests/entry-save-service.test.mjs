import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {} };
globalThis.window = globalThis;
let active = null;
let sequence = 0;
const sessions = new Map();
const pending = [];
const calls = [];
function snapshot(id, revision = 1) {
  return {
    path: `C:/synthetic/${id}.kdbx`,
    fileName: `${id}.kdbx`,
    revision,
    dirty: revision > 1,
    readOnly: false,
    root: { uuid: "root", name: "", children: [], entries: [] },
    customIcons: {},
  };
}
const backend = {
  invoke(command, args = {}) {
    calls.push({ command, args });
    switch (command) {
      case "open_vault": {
        active = `synthetic-${++sequence}`;
        const state = snapshot(active);
        sessions.set(active, state);
        return Promise.resolve({ sessionId: active, state });
      }
      case "list_sessions":
        return Promise.resolve(
          [...sessions].map(([id, state]) => ({
            sessionId: id,
            path: state.path,
            fileName: state.fileName,
            dirty: state.dirty,
          })),
        );
      case "close_all_vaults":
        sessions.clear();
        active = null;
        return Promise.resolve();
      case "set_active_session":
        active = args.sessionId;
        return Promise.resolve(sessions.get(active));
      case "get_vault_state":
        return Promise.resolve(sessions.get(args.sessionId));
      case "refresh_remote_vault": {
        const state = snapshot(args.sessionId);
        state.databaseName = "Replacement";
        sessions.set(args.sessionId, state);
        return Promise.resolve(state);
      }
      case "save_entry":
        return new Promise((resolve, reject) => pending.push({ args, resolve, reject }));
      case "set_config":
        return Promise.resolve(args.config);
      case "clear_saved_credential":
        return Promise.resolve();
      default:
        return Promise.reject(new Error(`Unexpected synthetic command ${command}`));
    }
  },
};
globalThis.__TAURI_INTERNALS__ = backend;
const { vault } = await import("../src/lib/services/vault.ts");
const input = {
  groupUuid: "root",
  title: "Synthetic",
  username: "",
  password: "",
  url: "",
  notes: "",
};
const request = {
  target: { mode: "create" },
  input,
  autoType: { enabled: false, associations: [] },
  flags: { qualityCheck: false },
};
async function fixture() {
  await vault.closeAll();
  await vault.open("C:/synthetic/start.kdbx", "synthetic-master");
  calls.length = 0;
  pending.length = 0;
  return vault.getActiveSessionId();
}

test("editor transaction carries one exact request and publishes its UUID/state", async () => {
  const id = await fixture();
  const result = vault.saveEntry(request);
  assert.deepEqual(pending[0].args, { sessionId: id, request });
  assert.equal(calls.filter((call) => call.command === "save_entry").length, 1);
  pending[0].resolve({ uuid: "exact-created-id", state: snapshot(id, 2) });
  assert.equal((await result).uuid, "exact-created-id");
  assert.equal(vault.get().revision, 2);
});
test("native transaction refusal leaves the published state unchanged", async () => {
  await fixture();
  const before = vault.get();
  const result = vault.saveEntry(request);
  pending[0].reject(new Error("Synthetic refusal"));
  await assert.rejects(result, /Synthetic refusal/);
  assert.equal(vault.get(), before);
});
test("closed save cannot return a UUID or repopulate state", async () => {
  const id = await fixture();
  const result = Promise.allSettled([vault.saveEntry(request)]);
  await vault.closeAll();
  pending[0].resolve({ uuid: "obsolete", state: snapshot(id, 99) });
  assert.equal((await result)[0].status, "rejected");
  assert.equal(vault.get(), null);
});
test("replacement save cannot attach an old UUID to the replacement database", async () => {
  const id = await fixture();
  const result = Promise.allSettled([vault.saveEntry(request)]);
  await vault.refreshRemote();
  pending[0].resolve({ uuid: "obsolete", state: snapshot(id, 99) });
  assert.equal((await result)[0].status, "rejected");
  assert.equal(vault.get().databaseName, "Replacement");
});
test("parked origin save publishes only the addressed session", async () => {
  const a = await fixture();
  await vault.open("C:/synthetic/other.kdbx", "synthetic-master");
  const b = vault.getActiveSessionId();
  const result = vault.callInSession(a, () => vault.saveEntry(request));
  assert.equal(pending[0].args.sessionId, a);
  pending[0].resolve({ uuid: "origin-id", state: snapshot(a, 2) });
  assert.equal((await result).state.revision, 2);
  assert.equal(vault.getActiveSessionId(), b);
  assert.equal(vault.get().revision, 1);
});
test("browser editor uses one candidate for creation, failure and metadata edits", async () => {
  await fixture();
  await vault.closeAll();
  delete globalThis.__TAURI_INTERNALS__;
  try {
    const created = await vault.saveEntry(request);
    let entry = created.state.root.entries.find((e) => e.uuid === created.uuid);
    assert.equal(entry.title, "Synthetic");
    assert.equal(entry.autoType.enabled, false);
    assert.equal(entry.qualityCheck, false);
    const before = structuredClone(vault.get());
    await assert.rejects(
      vault.saveEntry({
        ...request,
        target: { mode: "edit", uuid: created.uuid },
        input: { ...input, title: "Rejected" },
        flags: { foregroundColor: "invalid" },
      }),
    );
    assert.deepEqual(vault.get(), before);
    await assert.rejects(
      vault.saveEntry({ ...request, input: { ...input, groupUuid: "missing" } }),
    );
    assert.deepEqual(vault.get(), before);
    await assert.rejects(
      vault.saveEntry({ ...request, target: { mode: "edit", uuid: "missing" } }),
    );
    assert.deepEqual(vault.get(), before);
    const saved = await vault.saveEntry({
      target: { mode: "edit", uuid: created.uuid },
      input: { ...input, title: "Edited" },
      flags: { overrideUrl: " https://synthetic.example ", foregroundColor: "#112233" },
    });
    entry = saved.state.root.entries.find((e) => e.uuid === created.uuid);
    assert.equal(entry.title, "Edited");
    assert.equal(entry.overrideUrl, "https://synthetic.example");
    assert.equal(entry.foregroundColor, "#112233");
    assert.equal(entry.autoType.enabled, false);
    assert.equal(entry.qualityCheck, false);
  } finally {
    globalThis.__TAURI_INTERNALS__ = backend;
  }
});
