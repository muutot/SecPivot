import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";
import { get } from "svelte/store";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
const storage = new Map();
globalThis.localStorage = {
  getItem: (key) => storage.get(key) ?? null,
  setItem: (key, value) => storage.set(key, String(value)),
  removeItem: (key) => storage.delete(key),
};
globalThis.window = globalThis;
const sessions = new Map();
let active = null;
let sequence = 0;
const pending = [];
let deferRefresh = false;
let closeFails = false;
let deferTabsOnce = false;
let deferRemote = false;
let deferCleanup = false;

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

globalThis.__TAURI_INTERNALS__ = {
  invoke(command, args = {}) {
    switch (command) {
      case "open_vault": {
        active = `synthetic-${++sequence}`;
        const state = snapshot(active);
        sessions.set(active, state);
        return Promise.resolve({ sessionId: active, state });
      }
      case "list_sessions": {
        const listed = [...sessions.keys()]
          .sort((a, b) => Number(b === active) - Number(a === active))
          .map((id) => ({
            sessionId: id,
            path: sessions.get(id).path,
            fileName: sessions.get(id).fileName,
            dirty: sessions.get(id).dirty,
          }));
        if (deferTabsOnce) {
          deferTabsOnce = false;
          return new Promise((resolve) => pending.push({ command, resolve, listed }));
        }
        return Promise.resolve(listed);
      }
      case "get_vault_state":
        if (!deferRefresh) return Promise.resolve(sessions.get(args.sessionId) ?? null);
        break;
      case "add_entry":
      case "save_vault":
        break;
      case "merge_remote_vault": {
        const state = snapshot(args.sessionId, 2);
        sessions.set(args.sessionId, state);
        return Promise.resolve(state);
      }
      case "close_vault":
        if (closeFails) return Promise.reject(new Error("synthetic close failure"));
        sessions.delete(args.sessionId);
        if (active === args.sessionId) active = [...sessions.keys()].at(-1) ?? null;
        return Promise.resolve();
      case "close_all_vaults":
        sessions.clear();
        active = null;
        return Promise.resolve();
      case "set_active_session":
        active = args.sessionId;
        return Promise.resolve(sessions.get(active));
      case "refresh_remote_vault": {
        if (deferRemote) break;
        const state = snapshot(args.sessionId, 1);
        state.databaseName = "Synthetic replacement";
        sessions.set(args.sessionId, state);
        return Promise.resolve(state);
      }
      case "set_config":
        return Promise.resolve(args.config);
      case "clear_saved_credential":
        return Promise.resolve();
      case "open_attachment_temp":
        return Promise.resolve({
          token: "synthetic-token",
          sessionId: args.sessionId,
          path: "C:/synthetic/temporary.txt",
          name: "Synthetic.txt",
        });
      case "cleanup_attachment_temp":
        if (deferCleanup) break;
        return Promise.resolve();
      default:
        return Promise.reject(new Error(`Unexpected synthetic command: ${command}`));
    }
    return new Promise((resolve) => pending.push({ command, sessionId: args.sessionId, resolve }));
  },
};
const { vault } = await import("../src/lib/services/vault.ts");

async function fixture() {
  deferRefresh = false;
  closeFails = false;
  deferTabsOnce = false;
  deferRemote = false;
  deferCleanup = false;
  await vault.closeAll();
  pending.length = 0;
  await vault.open("C:/synthetic/open.kdbx", "synthetic-master");
  return vault.getActiveSessionId();
}

function edit() {
  return vault.addEntry({
    groupUuid: "root",
    title: "Synthetic",
    username: "",
    password: "",
    url: "",
    notes: "",
  });
}

for (const method of ["close", "closeTab", "closeAll"]) {
  test(`late state responses after ${method} cannot repopulate a closed-session cache`, async () => {
    const id = await fixture();
    const outcomes = Promise.allSettled([edit(), edit()]);
    assert.equal(pending.length, 2);
    await vault[method](...(method === "closeTab" ? [id] : []));
    // Both operations were admitted under the same original session. Only IPC
    // delivery is delayed; closing must invalidate their renderer publication.
    pending[0].resolve(snapshot(id, 99));
    pending[1].resolve(snapshot(id, 2));
    const actual = (await outcomes).map((result) => ({
      status: result.status,
      ...(result.status === "fulfilled" ? { revision: result.value.revision } : {}),
    }));
    assert.deepEqual(actual, [{ status: "rejected" }, { status: "rejected" }]);
    assert.equal(vault.get(), null);
    assert.equal(vault.getActiveSessionId(), null);
  });
}

test("closing A with B retained rejects A responses without changing B", async () => {
  const id = await fixture();
  const outcome = Promise.allSettled([edit()]);
  await vault.open("C:/synthetic/other.kdbx", "synthetic-master");
  const visible = vault.get();
  const other = vault.getActiveSessionId();
  await vault.closeTab(id);
  pending[0].resolve(snapshot(id, 99));
  assert.equal((await outcome)[0].status, "rejected");
  assert.equal(vault.getActiveSessionId(), other);
  assert.equal(vault.get().revision, visible.revision);
  assert.equal(vault.get().path, visible.path);
});

test("reopening the same path cannot admit a closed lifetime's late response", async () => {
  const id = await fixture();
  const outcome = Promise.allSettled([edit()]);
  await vault.closeAll();
  await vault.open("C:/synthetic/open.kdbx", "synthetic-master");
  const current = vault.getActiveSessionId();
  assert.notEqual(current, id);
  pending[0].resolve(snapshot(id, 99));
  assert.equal((await outcome)[0].status, "rejected");
  assert.equal(vault.getActiveSessionId(), current);
  assert.equal(vault.get().revision, 1);
});

test("late refresh of a closed session cannot publish its returned tree", async () => {
  const id = await fixture();
  deferRefresh = true;
  const outcome = Promise.allSettled([vault.refresh()]);
  deferRefresh = false;
  await vault.closeAll();
  pending[0].resolve(snapshot(id, 99));
  assert.equal((await outcome)[0].status, "rejected");
  assert.equal(vault.get(), null);
});

test("a failed close leaves the session eligible for a pending response", async () => {
  const id = await fixture();
  const result = edit();
  closeFails = true;
  await assert.rejects(vault.close(), /synthetic close failure/);
  closeFails = false;
  pending[0].resolve(snapshot(id, 2));
  assert.equal((await result).revision, 2);
  assert.equal(vault.get().revision, 2);
});

test("pending updates to a retained parked session remain usable after A to B to A", async () => {
  const id = await fixture();
  const result = edit();
  await vault.open("C:/synthetic/other.kdbx", "synthetic-master");
  const other = vault.getActiveSessionId();
  pending[0].resolve(snapshot(id, 2));
  assert.equal((await result).revision, 2);
  assert.equal(vault.getActiveSessionId(), other);
  await vault.setActiveSession(id);
  assert.equal(vault.getActiveSessionId(), id);
  assert.equal(vault.get().revision, 2);
});

test("remote replacement epoch still wins over an older pending mutation", async () => {
  const id = await fixture();
  const result = edit();
  await vault.refreshRemote();
  pending[0].resolve(snapshot(id, 99));
  assert.equal((await result).databaseName, "Synthetic replacement");
  assert.equal(vault.get().databaseName, "Synthetic replacement");
  assert.equal(vault.get().revision, 1);
});

test("newest revisions still win for a live session", async () => {
  const id = await fixture();
  const first = edit(),
    second = edit();
  pending[1].resolve(snapshot(id, 3));
  assert.equal((await second).revision, 3);
  pending[0].resolve(snapshot(id, 2));
  assert.equal((await first).revision, 3);
  assert.equal(vault.get().revision, 3);
});

test("a stale tab-list completion cannot restore a closed active id", async () => {
  await fixture();
  deferTabsOnce = true;
  const result = vault.mergeRemote();
  for (
    let attempt = 0;
    attempt < 10 && !pending.some((p) => p.command === "list_sessions");
    attempt++
  ) {
    await new Promise((resolve) => setImmediate(resolve));
  }
  const request = pending.find((p) => p.command === "list_sessions");
  assert.ok(request);
  await vault.closeAll();
  request.resolve(request.listed);
  await result;
  assert.equal(vault.getActiveSessionId(), null);
  assert.equal(get(vault.tabs).length, 0);
  assert.equal(vault.get(), null);
});

test("uncached listed sessions can be activated then correctly invalidated on close", async () => {
  await fixture();
  const id = `synthetic-${++sequence}`;
  sessions.set(id, snapshot(id));
  await vault.refresh();
  await vault.setActiveSession(id);
  assert.equal(vault.getActiveSessionId(), id);
  const outcome = Promise.allSettled([edit()]);
  await vault.closeTab(id);
  pending[0].resolve(snapshot(id, 99));
  assert.equal((await outcome)[0].status, "rejected");
});

test("late remote replacement cannot reintroduce a closed session", async () => {
  const id = await fixture();
  deferRemote = true;
  const outcome = Promise.allSettled([vault.refreshRemote()]);
  deferRemote = false;
  await vault.closeAll();
  pending[0].resolve(snapshot(id, 99));
  assert.equal((await outcome)[0].status, "rejected");
  assert.equal(vault.get(), null);
});

test("explicit unlisted session admission is invalidated by closing that id", async () => {
  await fixture();
  const id = `synthetic-${++sequence}`;
  sessions.set(id, snapshot(id));
  const outcome = Promise.allSettled([vault.callInSession(id, edit)]);
  await vault.closeTab(id);
  pending[0].resolve(snapshot(id, 99));
  assert.equal((await outcome)[0].status, "rejected");
});

test("failed re-admission of a disposed id cannot revive an earlier response", async () => {
  const id = await fixture();
  const outcome = Promise.allSettled([edit()]);
  await vault.closeAll();
  await assert.rejects(vault.setActiveSession(id));
  pending[0].resolve(snapshot(id, 99));
  assert.equal((await outcome)[0].status, "rejected");
  assert.equal(vault.get(), null);
  assert.equal(vault.getActiveSessionId(), null);
});

test("successful backend lock clears renderer state before slow temp cleanup", async () => {
  await fixture();
  await vault.openAttachmentTemp("synthetic-entry", "Synthetic.txt");
  deferCleanup = true;
  const closing = vault.closeAll();
  for (
    let attempt = 0;
    attempt < 10 && !pending.some((p) => p.command === "cleanup_attachment_temp");
    attempt++
  ) {
    await new Promise((resolve) => setImmediate(resolve));
  }
  const request = pending.find((p) => p.command === "cleanup_attachment_temp");
  assert.ok(request);
  const observed = [
    vault.get() === null,
    get(vault.tabs).length,
    vault.getActiveSessionId() === null,
  ];
  request.resolve();
  await closing;
  assert.deepEqual(observed, [true, 0, true]);
});
