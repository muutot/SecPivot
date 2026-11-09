import assert from "node:assert/strict";
import { register } from "node:module";
import { mock, test } from "node:test";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.window = globalThis;
globalThis.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {} };
let clipboardText = "";
let pendingText = "";
let pendingWrite;
let openingGate;
let openingStarted;
let active = null;
let nextId = 0;
let closes = 0;
const sessions = new Map();
const writes = [];

Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: {
    clipboard: {
      writeText: async (text) => {
        writes.push(text);
        if (text === pendingText) await pendingWrite.promise;
        clipboardText = text;
      },
      readText: async () => clipboardText,
    },
  },
});
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    switch (command) {
      case "open_vault": {
        active = `synthetic-${++nextId}`;
        const state = {
          path: args.path,
          fileName: args.path,
          revision: 1,
          dirty: false,
          readOnly: false,
          root: { uuid: "root", name: "", children: [], entries: [] },
        };
        sessions.set(active, state);
        if (openingGate && args.path === "synthetic-B.kdbx") {
          openingStarted.resolve();
          await openingGate.promise;
        }
        return { sessionId: active, state };
      }
      case "list_sessions":
        return [...sessions]
          .sort(([a], [b]) => (a === active ? -1 : b === active ? 1 : 0))
          .map(([sessionId, state]) => ({
            sessionId,
            path: state.path,
            fileName: state.fileName,
            dirty: false,
          }));
      case "get_vault_state":
        return sessions.get(args.sessionId ?? active) ?? null;
      case "set_active_session":
        assert.ok(sessions.has(args.sessionId));
        active = args.sessionId;
        return sessions.get(active);
      case "close_all_vaults":
        closes += 1;
        sessions.clear();
        active = null;
        return;
      case "clipboard_clear":
        clipboardText = "";
        return true;
      case "clipboard_cancel_scheduled_wipe":
      case "clipboard_schedule_wipe":
      case "clear_saved_credential":
        return;
      case "set_config":
        return args.config;
      default:
        throw new Error(`Unexpected synthetic IPC command: ${command}`);
    }
  },
};
const { vault } = await import("../src/lib/services/vault.ts");
const { copySensitive } = await import("../src/lib/services/security.ts");
const { copyText, clearClipboard } = await import("../src/lib/utils/clipboard.ts");
const { appSettings, DEFAULT_SECURITY_SETTINGS } = await import("../src/lib/services/settings.ts");

function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

async function setup() {
  mock.timers.enable({ apis: ["setTimeout"] });
  await vault.closeAll();
  await clearClipboard();
  appSettings.updateSecurity("lockAfterAction", true);
  appSettings.updateSecurity("clipboardClearSeconds", 0);
  await vault.open("synthetic-A.kdbx", "synthetic-master");
  closes = 0;
  pendingText = "synthetic-delayed-password";
  pendingWrite = deferred();
  openingGate = undefined;
  writes.length = 0;
  return vault.getActiveSessionId();
}

async function cleanup() {
  pendingWrite.resolve();
  openingGate?.resolve();
  pendingText = "";
  await clearClipboard();
  await vault.closeAll();
  appSettings.updateSecurity("lockAfterAction", DEFAULT_SECURITY_SETTINGS.lockAfterAction);
  appSettings.updateSecurity(
    "clipboardClearSeconds",
    DEFAULT_SECURITY_SETTINGS.clipboardClearSeconds,
  );
  mock.timers.reset();
}

test("a delayed sensitive copy cannot lock a newly opened vault", async () => {
  await setup();
  try {
    const copying = copySensitive(pendingText);
    await vault.open("synthetic-B.kdbx", "synthetic-master");
    const ownerB = vault.getActiveSessionId();
    pendingWrite.resolve();
    await copying;
    assert.equal(vault.getActiveSessionId(), ownerB);
    assert.equal(closes, 0);
  } finally {
    await cleanup();
  }
});

test("A to B to A invalidates lock-after-copy even when the session id matches again", async () => {
  const ownerA = await setup();
  try {
    const copying = copySensitive(pendingText);
    await vault.open("synthetic-B.kdbx", "synthetic-master");
    await vault.setActiveSession(ownerA);
    pendingWrite.resolve();
    await copying;
    assert.equal(vault.getActiveSessionId(), ownerA);
    assert.equal(closes, 0);
  } finally {
    await cleanup();
  }
});

test("a queued sensitive copy cancelled by clear cannot trigger lock after action", async () => {
  const ownerA = await setup();
  try {
    const blocker = copyText(pendingText);
    const copying = copySensitive("synthetic-cancelled-password");
    const clearing = clearClipboard();
    pendingWrite.resolve();
    await Promise.all([blocker, copying, clearing]);
    assert.equal(writes.includes("synthetic-cancelled-password"), false);
    assert.equal(vault.getActiveSessionId(), ownerA);
    assert.equal(closes, 0);
  } finally {
    await cleanup();
  }
});

test("an unchanged current sensitive copy locks once and preserves the copied value", async () => {
  await setup();
  try {
    await copySensitive("synthetic-current-password");
    assert.equal(vault.getActiveSessionId(), null);
    assert.equal(closes, 1);
    assert.equal(clipboardText, "synthetic-current-password");
  } finally {
    await cleanup();
  }
});

test("lock-after-copy revalidates ownership inside the topology queue after an earlier open", async () => {
  await setup();
  try {
    const copying = copySensitive(pendingText);
    openingGate = deferred();
    openingStarted = deferred();
    const opening = vault.open("synthetic-B.kdbx", "synthetic-master");
    await openingStarted.promise;
    pendingWrite.resolve();
    await new Promise(setImmediate);
    openingGate.resolve();
    await Promise.all([copying, opening]);
    assert.equal(vault.get()?.path, "synthetic-B.kdbx");
    assert.notEqual(vault.getActiveSessionId(), null);
    assert.equal(closes, 0);
  } finally {
    await cleanup();
  }
});

test("browser demo opening another path invalidates the old copy even with its fixed session id", async () => {
  const tauri = globalThis.__TAURI_INTERNALS__;
  delete globalThis.__TAURI_INTERNALS__;
  await setup();
  try {
    assert.equal(vault.getActiveSessionId(), "browser");
    const copying = copySensitive(pendingText);
    await vault.open("synthetic-B.kdbx", "synthetic-master");
    pendingWrite.resolve();
    await copying;
    assert.equal(vault.getActiveSessionId(), "browser");
    assert.equal(vault.get()?.path, "synthetic-B.kdbx");
  } finally {
    await cleanup();
    globalThis.__TAURI_INTERNALS__ = tauri;
  }
});

test("a rejected sensitive copy releases lifetime observers without locking", async () => {
  const ownerA = await setup();
  const originalWrite = navigator.clipboard.writeText;
  const originalActiveSubscribe = vault.activeId.subscribe;
  const originalStateSubscribe = vault.subscribe;
  let observers = 0;
  const track =
    (subscribe) =>
    (...args) => {
      observers += 1;
      const stop = subscribe(...args);
      return () => {
        observers -= 1;
        stop();
      };
    };
  const activeMock = mock.method(vault.activeId, "subscribe", track(originalActiveSubscribe));
  const stateMock = mock.method(vault, "subscribe", track(originalStateSubscribe));
  navigator.clipboard.writeText = async () => {
    throw new Error("synthetic-write-failed");
  };
  try {
    await assert.rejects(copySensitive("synthetic-rejected-password"), /synthetic-write-failed/);
    assert.equal(vault.getActiveSessionId(), ownerA);
    assert.equal(closes, 0);
    assert.equal(observers, 0);
  } finally {
    navigator.clipboard.writeText = originalWrite;
    activeMock.mock.restore();
    stateMock.mock.restore();
    await cleanup();
  }
});

test("current copy still locks when renderer digest support is unavailable", async () => {
  await setup();
  const originalDigest = crypto.subtle.digest;
  crypto.subtle.digest = async () => {
    throw new Error("synthetic-crypto-unavailable");
  };
  appSettings.updateSecurity("clipboardClearSeconds", 30);
  try {
    await copySensitive("synthetic-native-safety-net-password");
    assert.equal(vault.getActiveSessionId(), null);
    assert.equal(closes, 1);
    assert.equal(clipboardText, "synthetic-native-safety-net-password");
  } finally {
    crypto.subtle.digest = originalDigest;
    await cleanup();
  }
});
