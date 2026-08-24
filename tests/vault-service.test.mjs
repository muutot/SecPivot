/**
 * Runtime tests for the `vault` service IPC glue (T-1 audit remediation).
 *
 * These exercise the REAL module graph — `services/vault.ts`, its settings
 * dependency and session-state helpers — with only two seams faked:
 * - a `$lib/*` resolve hook (registered below) so Node can load the alias;
 * - `window.__TAURI_INTERNALS__.invoke`, exactly the seam
 *   `@tauri-apps/api/core#invoke` delegates to, so command names, camelCase
 *   payloads, session binding and epoch/revision guards run for real.
 *
 * Run by `npm run test:frontend` (this file registers its own loader before
 * dynamically importing the service).
 */
import { register } from "node:module";
import { mock, test } from "node:test";
import assert from "node:assert/strict";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));

// --- browser-ish globals the services expect -------------------------------
const localStorageBacking = new Map();
globalThis.localStorage = {
  getItem: (key) => localStorageBacking.get(key) ?? null,
  setItem: (key, value) => localStorageBacking.set(key, String(value)),
  removeItem: (key) => localStorageBacking.delete(key),
  clear: () => localStorageBacking.clear(),
};
globalThis.window = globalThis;

// --- fake Tauri backend -----------------------------------------------------
function makeState(revision) {
  return {
    revision,
    path: "C:/vaults/demo.kdbx",
    fileName: "demo.kdbx",
    dirty: false,
    readOnly: false,
    root: {
      uuid: "root",
      parentUuid: null,
      name: "",
      isRecycleBin: false,
      children: [],
      entries: [],
    },
    databaseName: undefined,
    databaseDescription: undefined,
    customIcons: {},
  };
}

let state = makeState(0);
const calls = [];
const deferred = [];

const invoke = mock.fn((cmd, args = {}) => {
  calls.push({ cmd, args });
  switch (cmd) {
    case "open_vault":
      state = makeState(1);
      return Promise.resolve({ sessionId: "s1", state });
    case "list_sessions":
      return Promise.resolve(
        state
          ? [{ sessionId: "s1", path: state.path, fileName: state.fileName, dirty: state.dirty }]
          : [],
      );
    case "get_vault_state":
      return Promise.resolve(state);
    case "add_entry":
      state = makeState(state.revision + 1);
      return Promise.resolve(state);
    case "save_vault": {
      const result = makeState(state.revision + 1);
      return new Promise((resolvePromise) => {
        deferred.push({ args, resolvePromise, result });
      });
    }
    default:
      return Promise.reject(new Error(`unexpected command in test: ${cmd}`));
  }
});

globalThis.window.__TAURI_INTERNALS__ = { invoke };

// Import AFTER the seams are in place; dynamic import so registration applies.
const { vault } = await import("../src/lib/services/vault.ts");

test("open binds the active session and add_entry forwards exact command + payload", async () => {
  await vault.open("C:/vaults/demo.kdbx", "pw");
  assert.equal(vault.getActiveSessionId(), "s1");

  calls.length = 0;
  const input = {
    groupUuid: "root",
    title: "t",
    username: "u",
    password: "p",
    url: "",
    notes: "",
    totp: undefined,
    customFields: [],
    attachments: [],
  };
  const returned = await vault.callInSession("s1", () => vault.addEntry(input));
  const entryCall = calls.find((c) => c.cmd === "add_entry");
  assert.ok(entryCall, "add_entry must be invoked");
  assert.equal(entryCall.args.sessionId, "s1");
  assert.deepEqual(entryCall.args.input, input);
  assert.equal(returned.revision, state.revision);
});

test("callInSession overrides an inactive session id on the wire", async () => {
  calls.length = 0;
  await vault.callInSession("s2", () =>
    vault.addEntry({
      groupUuid: "root",
      title: "x",
      username: "",
      password: "",
      url: "",
      notes: "",
      totp: undefined,
      customFields: [],
      attachments: [],
    }),
  );
  const entryCall = calls.find((c) => c.cmd === "add_entry");
  assert.equal(entryCall.args.sessionId, "s2", "override must reach the backend payload");
});

test("out-of-order save responses cannot clobber newer committed state", async () => {
  // Two concurrent saves; responses arrive out of order — the stale
  // lower-revision snapshot lands after the newer one already committed.
  const first = vault.save();
  const second = vault.save();
  assert.equal(deferred.length, 2);
  const [olderRequest, newerRequest] = deferred;

  newerRequest.resolvePromise(newerRequest.result);
  await second;

  olderRequest.resolvePromise({
    ...newerRequest.result,
    revision: newerRequest.result.revision - 1,
  });
  await first;

  assert.equal(vault.get().revision, newerRequest.result.revision);
});
