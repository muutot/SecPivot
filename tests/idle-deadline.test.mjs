import assert from "node:assert/strict";
import { register } from "node:module";
import { mock, test } from "node:test";

register(new URL("./helpers/lib-alias-loader.mjs", import.meta.url));
globalThis.window = globalThis;
globalThis.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {} };
const listeners = new Map();
window.addEventListener = (type, callback) => {
  if (!listeners.has(type)) listeners.set(type, new Set());
  listeners.get(type).add(callback);
};
window.removeEventListener = (type, callback) => listeners.get(type)?.delete(callback);
globalThis.document = {
  visibilityState: "visible",
  addEventListener: window.addEventListener,
  removeEventListener: window.removeEventListener,
};
Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: { userAgent: "synthetic-host" },
});
let active;
let closes = 0;
let closeGate;
let clipboardGate;
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    switch (command) {
      case "open_vault":
        active = {
          sessionId: "synthetic",
          state: {
            path: args.path,
            fileName: "synthetic.kdbx",
            revision: 1,
            root: { uuid: "root", name: "Synthetic", children: [], entries: [] },
          },
        };
        return active;
      case "list_sessions":
        return active ? [{ sessionId: active.sessionId, ...active.state }] : [];
      case "close_all_vaults":
        closes++;
        if (closeGate) await closeGate;
        active = undefined;
        return;
      case "set_config":
        return args.config;
      case "clear_saved_credential":
      case "clipboard_cancel_scheduled_wipe":
        return;
      case "clipboard_clear":
        if (clipboardGate) await clipboardGate;
        return true;
      default:
        throw new Error(`Unexpected synthetic command: ${command}`);
    }
  },
};
const { vault } = await import("../src/lib/services/vault.ts");
const { appSettings } = await import("../src/lib/services/settings.ts");
const { installAutoLock, armIdleLock } = await import("../src/lib/services/security.ts");

const settle = () => new Promise((resolve) => setImmediate(resolve));
function emit(type) {
  for (const callback of [...(listeners.get(type) ?? [])]) callback();
}

async function fixture() {
  mock.timers.enable({ apis: ["setTimeout", "Date"], now: 1000 });
  appSettings.updateSecurity("autoLockMinutes", 1);
  appSettings.updateSecurity("clearOnLock", false);
  appSettings.updateSecurity("lockOnFocusLoss", false);
  await appSettings.flush();
  await vault.open("synthetic-idle.kdbx", "synthetic-master");
  closes = 0;
  closeGate = undefined;
  clipboardGate = undefined;
  document.visibilityState = "visible";
  const stop = installAutoLock();
  const dispose = async () => {
    stop();
    closeGate = undefined;
    await vault.closeAll();
    // Do not leave settings debounce handles alive across fake-clock resets.
    await appSettings.flush();
    listeners.clear();
    mock.timers.reset();
  };
  dispose.stop = stop;
  return dispose;
}

for (const event of [
  "pointerdown",
  "keydown",
  "mousemove",
  "wheel",
  "scroll",
  "focus",
  "visibilitychange",
]) {
  test(`expired idle deadline survives delayed timer delivery and ${event}`, async () => {
    const dispose = await fixture();
    try {
      // Advance wall time without delivering due timers, as with a suspended
      // renderer. Only a normal resume/activity event is then delivered.
      mock.timers.setTime(61_001);
      emit(event);
      await settle();
      assert.equal(closes, 1, "a due idle lock must not become a new idle period");
      assert.equal(vault.get(), null);
    } finally {
      await dispose();
    }
  });
}

test("activity before expiry still receives a complete new idle period", async () => {
  const dispose = await fixture();
  try {
    mock.timers.tick(45_000);
    emit("pointerdown");
    mock.timers.tick(15_001);
    await settle();
    assert.equal(closes, 0);
    mock.timers.tick(45_000);
    await settle();
    assert.equal(closes, 1);
  } finally {
    await dispose();
  }
});

test("hidden visibility does not replace native background-lock policy", async () => {
  const dispose = await fixture();
  try {
    mock.timers.setTime(61_001);
    document.visibilityState = "hidden";
    emit("visibilitychange");
    await settle();
    assert.equal(closes, 0);
    document.visibilityState = "visible";
    emit("visibilitychange");
    await settle();
    assert.equal(closes, 1);
  } finally {
    await dispose();
  }
});

test("unrelated settings do not extend the idle deadline", async () => {
  const dispose = await fixture();
  try {
    mock.timers.tick(45_000);
    appSettings.updateSecurity("lockOnFocusLoss", true);
    mock.timers.tick(15_001);
    await settle();
    assert.equal(closes, 1);
  } finally {
    await dispose();
  }
});

test("changing idle duration before expiry starts the requested period", async () => {
  const dispose = await fixture();
  try {
    mock.timers.tick(45_000);
    appSettings.updateSecurity("autoLockMinutes", 2);
    mock.timers.tick(119_999);
    await settle();
    assert.equal(closes, 0);
    mock.timers.tick(1);
    await settle();
    assert.equal(closes, 1);
  } finally {
    await dispose();
  }
});

test("changing an enabled duration cannot revive an expired period", async () => {
  const dispose = await fixture();
  try {
    mock.timers.setTime(61_001);
    appSettings.updateSecurity("autoLockMinutes", 2);
    await settle();
    assert.equal(closes, 1);
  } finally {
    await dispose();
  }
});

test("cleanup removes every resume/activity listener and cancels the timer", async () => {
  const dispose = await fixture();
  try {
    dispose.stop();
    assert.equal(
      [...listeners.values()].reduce((n, callbacks) => n + callbacks.size, 0),
      0,
    );
    appSettings.updateSecurity("autoLockMinutes", 2);
    emit("focus");
    emit("pointerdown");
    mock.timers.tick(120_000);
    await settle();
    assert.equal(closes, 0);
  } finally {
    await dispose();
  }
});

test("published close followed by queued reopen receives a new idle period", async () => {
  const dispose = await fixture();
  // Mirror the page's open/closed transition subscription, using real stores.
  let wasOpen = !!vault.get();
  const unsubscribe = vault.subscribe((state) => {
    const open = !!state;
    if (open !== wasOpen) {
      wasOpen = open;
      armIdleLock();
    }
  });
  let release;
  try {
    closeGate = new Promise((resolve) => (release = resolve));
    mock.timers.tick(60_000);
    await settle();
    const opening = vault.open("synthetic-next.kdbx", "synthetic-master");
    release();
    closeGate = undefined;
    await opening;
    await settle();
    assert.equal(closes, 1);
    assert.equal(vault.get().path, "synthetic-next.kdbx");
    mock.timers.tick(59_999);
    await settle();
    assert.equal(closes, 1);
    mock.timers.tick(1);
    await settle();
    assert.equal(closes, 2);
    assert.equal(vault.get(), null);
  } finally {
    release?.();
    unsubscribe();
    await dispose();
  }
});

test("early resume does not extend the existing idle deadline", async () => {
  const dispose = await fixture();
  try {
    mock.timers.tick(45_000);
    emit("focus");
    emit("visibilitychange");
    mock.timers.tick(15_001);
    await settle();
    assert.equal(closes, 1);
  } finally {
    await dispose();
  }
});

test("late clipboard completion cannot release a newer pending idle close", async () => {
  const dispose = await fixture();
  let wasOpen = !!vault.get();
  const unsubscribe = vault.subscribe((state) => {
    const open = !!state;
    if (open !== wasOpen) {
      wasOpen = open;
      armIdleLock();
    }
  });
  let releaseClipboard;
  let releaseClose;
  try {
    appSettings.updateSecurity("clearOnLock", true);
    await appSettings.flush();
    // Synthetic IPC only: no OS clipboard is read or written.
    clipboardGate = new Promise((resolve) => (releaseClipboard = resolve));
    mock.timers.tick(60_000);
    await settle();
    assert.equal(closes, 1);
    assert.equal(vault.get(), null);
    await vault.open("synthetic-next.kdbx", "synthetic-master");
    closeGate = new Promise((resolve) => (releaseClose = resolve));
    mock.timers.tick(60_000);
    await settle();
    assert.equal(closes, 2);
    releaseClipboard();
    clipboardGate = undefined;
    await settle();
    await settle();
    emit("pointerdown");
    mock.timers.tick(60_001);
    await settle();
    releaseClose();
    closeGate = undefined;
    await settle();
    await settle();
    assert.equal(closes, 2, "the earlier promise must not unlock the new pending attempt");
  } finally {
    releaseClipboard?.();
    releaseClose?.();
    unsubscribe();
    await settle();
    await dispose();
  }
});

test("disabled idle locking remains disabled after delayed activity", async () => {
  const dispose = await fixture();
  try {
    appSettings.updateSecurity("autoLockMinutes", 0);
    mock.timers.setTime(61_001);
    emit("pointerdown");
    emit("focus");
    await settle();
    assert.equal(closes, 0);
  } finally {
    await dispose();
  }
});

test("a pending idle lock cannot be rescheduled by more activity", async () => {
  const dispose = await fixture();
  let release;
  try {
    closeGate = new Promise((resolve) => (release = resolve));
    mock.timers.tick(60_000);
    await settle();
    assert.equal(closes, 1);
    emit("pointerdown");
    emit("wheel");
    mock.timers.tick(60_001);
    await settle();
    assert.equal(closes, 1, "one expired period must launch only one close");
    release();
    closeGate = undefined;
    await settle();
    await settle();
    assert.equal(closes, 1, "releasing the first close must not reveal a queued duplicate");
  } finally {
    release?.();
    await settle();
    await dispose();
  }
});
