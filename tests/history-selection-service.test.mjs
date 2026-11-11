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
const pending = [];
const calls = [];
let sequence = 0;
let active = null;
const selection = { index: 1, selectionToken: "synthetic-lifetime:4:synthetic-entry:1" };
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
          [...sessions]
            .sort(([a], [b]) => Number(b === active) - Number(a === active))
            .map(([sessionId, s]) => ({
              sessionId,
              path: s.path,
              fileName: s.fileName,
              dirty: s.dirty,
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
        const state = { ...snapshot(args.sessionId), databaseName: "Synthetic replacement" };
        sessions.set(args.sessionId, state);
        return Promise.resolve(state);
      }
      case "get_entry_history":
      case "get_history_website_rules":
      case "restore_entry_version":
      case "delete_entry_history":
        return new Promise((resolve, reject) => pending.push({ command, args, resolve, reject }));
      case "set_config":
        return Promise.resolve(args.config);
      case "clear_saved_credential":
        return Promise.resolve();
      default:
        return Promise.reject(new Error(`Unexpected fixture command: ${command}`));
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

test("history read retains the exact token and originating session", async () => {
  const id = await fixture();
  const request = vault.getEntryHistory("synthetic-entry");
  assert.deepEqual(pending[0].args, { uuid: "synthetic-entry", sessionId: id });
  const history = [{ ...selection, title: "Synthetic selected" }];
  pending[0].resolve(history);
  assert.equal(await request, history);
});

test("historical rule read binds both sources to the exact selected token", async () => {
  for (const source of ["version", "current"]) {
    const id = await fixture();
    const task = vault.getHistoryWebsiteRules("synthetic-entry", selection, source);
    assert.deepEqual(pending[0].args, {
      sessionId: id,
      request: { uuid: "synthetic-entry", ...selection, source },
    });
    const policy = {
      version: 1,
      rules: [{ url: "https://synthetic.example/exact?x=1", mode: "exact" }],
    };
    pending[0].resolve(policy);
    assert.equal(await task, policy);
    assert.equal(vault.get().revision, 1);
  }
});

test("history rule null and empty policies are returned distinctly; failures reject", async () => {
  await fixture();
  for (const value of [null, { version: 1, rules: [] }]) {
    const task = vault.getHistoryWebsiteRules("synthetic-entry", selection, "version");
    pending.at(-1).resolve(value);
    assert.equal(await task, value);
  }
  const task = Promise.allSettled([
    vault.getHistoryWebsiteRules("synthetic-entry", selection, "version"),
  ]);
  pending.at(-1).reject("synthetic-invalid-policy");
  assert.equal((await task)[0].status, "rejected");
});

test("invalid history rule selection or source never reaches IPC", async () => {
  await fixture();
  await assert.rejects(vault.getHistoryWebsiteRules("synthetic-entry", 1, "version"));
  await assert.rejects(vault.getHistoryWebsiteRules("synthetic-entry", selection));
  await assert.rejects(vault.getHistoryWebsiteRules("synthetic-entry", selection, "unknown"));
  assert.equal(calls.length, 0);
});

test("closed or replaced history rule reads reject late protected values", async () => {
  for (const replace of [false, true]) {
    await fixture();
    const task = Promise.allSettled([
      vault.getHistoryWebsiteRules("synthetic-entry", selection, "version"),
    ]);
    if (replace) await vault.refreshRemote();
    else await vault.closeAll();
    pending[0].resolve({ version: 1, rules: [] });
    assert.equal((await task)[0].status, "rejected");
  }
});

test("parked history rule reads use their explicit origin and browser rejects them", async () => {
  const owner = await fixture();
  await vault.open("C:/synthetic/other.kdbx", "synthetic-master");
  const task = vault.callInSession(owner, () =>
    vault.getHistoryWebsiteRules("synthetic-entry", selection, "version"),
  );
  assert.equal(pending[0].args.sessionId, owner);
  pending[0].resolve(null);
  assert.equal(await task, null);
  delete globalThis.__TAURI_INTERNALS__;
  try {
    await assert.rejects(vault.getHistoryWebsiteRules("synthetic-entry", selection, "version"));
  } finally {
    globalThis.__TAURI_INTERNALS__ = backend;
  }
});

test("both history mutations require the selected token and publish the result", async () => {
  for (const [method, command] of [
    ["restoreEntryVersion", "restore_entry_version"],
    ["deleteEntryHistory", "delete_entry_history"],
  ]) {
    const id = await fixture();
    const request = vault[method]("synthetic-entry", selection);
    assert.deepEqual(pending[0].args, { uuid: "synthetic-entry", ...selection, sessionId: id });
    assert.equal(pending[0].command, command);
    pending[0].resolve(snapshot(id, 2));
    assert.equal((await request).revision, 2);
    assert.equal(vault.get().revision, 2);
  }
});

test("bare indexes, omitted tokens and invalid selections never reach IPC", async () => {
  await fixture();
  for (const method of ["restoreEntryVersion", "deleteEntryHistory"]) {
    for (const bad of [
      undefined,
      1,
      { index: 1 },
      { index: 1, selectionToken: "" },
      { ...selection, index: -1 },
      { ...selection, index: 1.5 },
    ]) {
      await assert.rejects(vault[method]("synthetic-entry", bad));
    }
  }
  assert.equal(calls.length, 0);
});

test("native stale selection errors are localized and do not publish state", async () => {
  const id = await fixture();
  for (const method of ["restoreEntryVersion", "deleteEntryHistory"]) {
    const result = Promise.allSettled([vault[method]("synthetic-entry", selection)]);
    pending.at(-1).reject("HISTORY_SELECTION_STALE: fixture refusal");
    const [outcome] = await result;
    assert.equal(outcome.status, "rejected");
    assert.match(outcome.reason.message, /刷新历史|Refresh history/);
    assert.equal(vault.get().revision, 1);
    assert.equal(vault.getActiveSessionId(), id);
  }
});

test("closed or replaced history reads reject their late responses", async () => {
  for (const replace of [false, true]) {
    await fixture();
    const result = Promise.allSettled([vault.getEntryHistory("synthetic-entry")]);
    if (replace) await vault.refreshRemote();
    else await vault.closeAll();
    pending[0].resolve([{ ...selection }]);
    assert.equal((await result)[0].status, "rejected");
    if (replace) assert.equal(vault.get().databaseName, "Synthetic replacement");
    else assert.equal(vault.get(), null);
  }
});

test("history mutation completions cannot refill a closed or replaced session", async () => {
  for (const method of ["restoreEntryVersion", "deleteEntryHistory"]) {
    for (const replace of [false, true]) {
      const id = await fixture();
      const result = Promise.allSettled([vault[method]("synthetic-entry", selection)]);
      if (replace) await vault.refreshRemote();
      else await vault.closeAll();
      pending[0].resolve(snapshot(id, 99));
      assert.equal((await result)[0].status, "rejected");
      if (replace) assert.equal(vault.get().databaseName, "Synthetic replacement");
      else assert.equal(vault.get(), null);
    }
  }
});

test("parked history reads and mutations keep the explicit owner", async () => {
  const owner = await fixture();
  await vault.open("C:/synthetic/other.kdbx", "synthetic-master");
  const other = vault.getActiveSessionId();
  const read = vault.callInSession(owner, () => vault.getEntryHistory("synthetic-entry"));
  pending[0].resolve([{ ...selection }]);
  assert.equal((await read)[0].selectionToken, selection.selectionToken);
  const write = vault.callInSession(owner, () =>
    vault.deleteEntryHistory("synthetic-entry", selection),
  );
  assert.equal(pending[1].args.sessionId, owner);
  pending[1].resolve(snapshot(owner, 2));
  assert.equal((await write).revision, 2);
  assert.equal(vault.getActiveSessionId(), other);
  assert.equal(vault.get().revision, 1);
});

test("closed sessions and browser mutations reject before native invocation", async () => {
  await fixture();
  await vault.closeAll();
  calls.length = 0;
  await assert.rejects(vault.getEntryHistory("synthetic-entry"));
  await assert.rejects(vault.restoreEntryVersion("synthetic-entry", selection));
  await assert.rejects(vault.deleteEntryHistory("synthetic-entry", selection));
  delete globalThis.__TAURI_INTERNALS__;
  try {
    assert.deepEqual(await vault.getEntryHistory("synthetic-entry"), []);
    await assert.rejects(vault.restoreEntryVersion("synthetic-entry", selection));
    await assert.rejects(vault.deleteEntryHistory("synthetic-entry", selection));
  } finally {
    globalThis.__TAURI_INTERNALS__ = backend;
  }
  assert.equal(calls.length, 0);
});
