import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
const storage = new Map();
globalThis.localStorage = {
  getItem: (key) => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, String(value)),
  removeItem: (key) => storage.delete(key),
};
globalThis.window = globalThis;
const sessions = new Map();
const calls = [];
const pending = [];
let sequence = 0;
let active = null;

const policy = {
  version: 1,
  rules: [
    { url: "https://synthetic.example:443/path", mode: "host" },
    { url: "https://synthetic.example/exact?x=1", mode: "exact" },
    { url: "https://other.example/", mode: "site" },
    { url: "https://ignored.example/", mode: "never" },
  ],
};
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
          [...sessions.keys()]
            .sort((a, b) => Number(b === active) - Number(a === active))
            .map((id) => ({
              sessionId: id,
              path: sessions.get(id).path,
              fileName: sessions.get(id).fileName,
              dirty: false,
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
        return Promise.resolve(sessions.get(args.sessionId) ?? null);
      case "refresh_remote_vault": {
        const state = snapshot(args.sessionId);
        state.databaseName = "Synthetic replacement";
        sessions.set(args.sessionId, state);
        return Promise.resolve(state);
      }
      case "get_entry_website_rules":
      case "set_entry_website_rules":
        return new Promise((resolve) => pending.push({ command, args, resolve }));
      case "set_config":
        return Promise.resolve(args.config);
      case "clear_saved_credential":
        return Promise.resolve();
      default:
        return Promise.reject(new Error(`Unexpected synthetic command: ${command}`));
    }
  },
};
globalThis.__TAURI_INTERNALS__ = backend;
const { vault } = await import("../src/lib/services/vault.ts");

async function fixture() {
  await vault.closeAll();
  await vault.open("C:/synthetic/open.kdbx", "synthetic-master");
  pending.length = 0;
  calls.length = 0;
  return vault.getActiveSessionId();
}

test("website policy read binds the originating id and returns the exact typed rule set", async () => {
  const id = await fixture();
  const result = vault.getEntryWebsiteRules("synthetic-entry");
  assert.deepEqual(pending[0].args, { uuid: "synthetic-entry", sessionId: id });
  pending[0].resolve(policy);
  assert.deepEqual(await result, policy);
});

test("website policy write uses the strict request envelope and publishes only its state", async () => {
  const id = await fixture();
  const result = vault.setEntryWebsiteRules("synthetic-entry", policy);
  assert.deepEqual(pending[0].args, {
    sessionId: id,
    request: { uuid: "synthetic-entry", rules: policy },
  });
  pending[0].resolve(snapshot(id, 2));
  assert.equal((await result).revision, 2);
  assert.equal(vault.get().revision, 2);
});

test("omitted or undefined policy never becomes a clear or reaches IPC", async () => {
  await fixture();
  await assert.rejects(vault.setEntryWebsiteRules("synthetic-entry"));
  await assert.rejects(vault.setEntryWebsiteRules("synthetic-entry", undefined));
  assert.equal(calls.length, 0);
});

test("explicit null and an empty rule set retain distinct write payloads", async () => {
  const id = await fixture();
  const empty = { version: 1, rules: [] };
  const writes = [
    vault.setEntryWebsiteRules("synthetic-entry", null),
    vault.setEntryWebsiteRules("synthetic-entry", empty),
  ];
  assert.equal(pending[0].args.request.rules, null);
  assert.deepEqual(pending[1].args.request.rules, empty);
  pending[0].resolve(snapshot(id, 2));
  pending[1].resolve(snapshot(id, 3));
  await Promise.all(writes);
});

test("retained A to B to A does not redirect a policy read", async () => {
  const id = await fixture();
  const result = vault.getEntryWebsiteRules("synthetic-entry");
  await vault.open("C:/synthetic/other.kdbx", "synthetic-master");
  await vault.setActiveSession(id);
  pending[0].resolve(policy);
  assert.deepEqual(await result, policy);
  assert.equal(pending[0].args.sessionId, id);
});

test("closed policy read is rejected even after failed old-id activation", async () => {
  const id = await fixture();
  const result = Promise.allSettled([vault.getEntryWebsiteRules("synthetic-entry")]);
  await vault.closeAll();
  await assert.rejects(vault.setActiveSession(id));
  pending[0].resolve(policy);
  assert.equal((await result)[0].status, "rejected");
  assert.equal(vault.get(), null);
});

test("remote replacement invalidates an earlier policy read", async () => {
  await fixture();
  const result = Promise.allSettled([vault.getEntryWebsiteRules("synthetic-entry")]);
  await vault.refreshRemote();
  pending[0].resolve(policy);
  assert.equal((await result)[0].status, "rejected");
  assert.equal(vault.get().databaseName, "Synthetic replacement");
});

test("policy write response cannot refill closed state", async () => {
  const id = await fixture();
  const result = Promise.allSettled([vault.setEntryWebsiteRules("synthetic-entry", policy)]);
  await vault.closeAll();
  pending[0].resolve(snapshot(id, 99));
  assert.equal((await result)[0].status, "rejected");
  assert.equal(vault.get(), null);
});

test("explicit parked-origin write cannot change the visible other vault", async () => {
  const id = await fixture();
  await vault.open("C:/synthetic/other.kdbx", "synthetic-master");
  const other = vault.getActiveSessionId();
  const result = vault.callInSession(id, () =>
    vault.setEntryWebsiteRules("synthetic-entry", policy),
  );
  assert.equal(pending[0].args.sessionId, id);
  pending[0].resolve(snapshot(id, 2));
  assert.equal((await result).revision, 2);
  assert.equal(vault.getActiveSessionId(), other);
  assert.equal(vault.get().revision, 1);
});

test("closed sessions and browser preview reject native policy operations before IPC", async () => {
  await fixture();
  await vault.closeAll();
  calls.length = 0;
  await assert.rejects(vault.getEntryWebsiteRules("synthetic-entry"));
  await assert.rejects(vault.setEntryWebsiteRules("synthetic-entry", policy));
  assert.equal(calls.length, 0);
  delete globalThis.__TAURI_INTERNALS__;
  try {
    await assert.rejects(vault.getEntryWebsiteRules("synthetic-entry"), /浏览器预览/);
    await assert.rejects(vault.setEntryWebsiteRules("synthetic-entry", null), /浏览器预览/);
  } finally {
    globalThis.__TAURI_INTERNALS__ = backend;
  }
  assert.equal(calls.length, 0);
});
