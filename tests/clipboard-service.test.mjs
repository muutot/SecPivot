import { register } from "node:module";
import { test, mock } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
let clipboardText = "";
const calls = [];
const argumentsSeen = [];
let nativeConditional = "replace";
let nativeClear = true;
globalThis.window = globalThis;
globalThis.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {} };
Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: {
    clipboard: {
      writeText: async (text) => {
        clipboardText = text;
      },
      readText: async () => clipboardText,
    },
  },
});
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    calls.push(command);
    argumentsSeen.push({ command, args });
    if (command === "clipboard_read_text") {
      const old = clipboardText;
      clipboardText = "synthetic-foreign-copy";
      return old;
    }
    if (command === "clipboard_clear") {
      if (nativeClear) clipboardText = "";
      return nativeClear;
    }
    if (command === "clipboard_clear_if_matches") {
      if (nativeConditional === "absent") return null;
      // Replacement occurs before the single native transaction acquires its
      // OS lock, instead of between an IPC read and a blind IPC clear.
      if (nativeConditional === "replace") clipboardText = "synthetic-foreign-copy";
      const fingerprint = Array.from(createHash("sha256").update(clipboardText).digest());
      if (JSON.stringify(fingerprint) !== JSON.stringify(args.fingerprint)) return false;
      clipboardText = "";
      return true;
    }
  },
};
const { copyText, clearClipboardIfUnchanged, clearClipboard } =
  await import("../src/lib/utils/clipboard.ts");
const { appSettings, DEFAULT_SECURITY_SETTINGS } = await import("../src/lib/services/settings.ts");

function setClearSeconds(seconds) {
  appSettings.updateSecurity("clipboardClearSeconds", seconds);
}

function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test("native conditional wipe never clears a copy replaced between IPC operations", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  try {
    await copyText("synthetic-vault-copy");
    calls.length = 0;
    await clearClipboardIfUnchanged();
    assert.equal(clipboardText, "synthetic-foreign-copy");
    assert.deepEqual(calls, ["clipboard_clear_if_matches"]);
    calls.length = 0;
    await clearClipboardIfUnchanged();
    assert.deepEqual(calls, [], "a completed ownership check releases its remembered text");
  } finally {
    await clearClipboard();
    mock.timers.reset();
  }
});

test("disabled timed clearing retains no renderer ownership baseline", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  setClearSeconds(0);
  try {
    await copyText("synthetic-disabled-timer");
    calls.length = 0;
    await clearClipboardIfUnchanged();
    assert.deepEqual(calls, []);
  } finally {
    await clearClipboard();
    setClearSeconds(DEFAULT_SECURITY_SETTINGS.clipboardClearSeconds);
    mock.timers.reset();
  }
});

test("conditional clearing sends a fingerprint instead of copied plaintext", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  nativeConditional = "match";
  try {
    await copyText("synthetic-摘要🔑");
    argumentsSeen.length = 0;
    await clearClipboardIfUnchanged();
    const clear = argumentsSeen.find((call) => call.command === "clipboard_clear_if_matches");
    assert.ok(clear);
    assert.equal(Object.hasOwn(clear.args, "text"), false);
    assert.deepEqual(
      clear.args.fingerprint,
      Array.from(createHash("sha256").update("synthetic-摘要🔑").digest()),
    );
    assert.equal(clipboardText, "");
  } finally {
    nativeConditional = "replace";
    await clearClipboard();
    mock.timers.reset();
  }
});

test("a delayed ownership read cannot clear or release a newer identical copy", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  nativeConditional = "absent";
  const originalRead = navigator.clipboard.readText;
  const pending = deferred();
  navigator.clipboard.readText = () => pending.promise;
  try {
    await copyText("synthetic-identical-copy");
    const oldWipe = clearClipboardIfUnchanged();
    await Promise.resolve();
    await copyText("synthetic-identical-copy");
    pending.resolve("synthetic-identical-copy");
    await oldWipe;
    assert.equal(clipboardText, "synthetic-identical-copy");
    navigator.clipboard.readText = originalRead;
    await clearClipboardIfUnchanged();
    assert.equal(clipboardText, "", "the newer copy keeps its own scheduled ownership");
  } finally {
    pending.resolve("synthetic-identical-copy");
    navigator.clipboard.readText = originalRead;
    nativeConditional = "replace";
    await clearClipboard();
    mock.timers.reset();
  }
});

test("an unavailable native clear falls back to the renderer clipboard write", async () => {
  nativeClear = false;
  clipboardText = "synthetic-lock-clear";
  try {
    await clearClipboard();
    assert.equal(clipboardText, "");
  } finally {
    nativeClear = true;
  }
});

test("a digest completing after lock cannot resurrect the copied ownership", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  const originalDigest = crypto.subtle.digest;
  const pending = deferred();
  const started = deferred();
  crypto.subtle.digest = async (...args) => {
    const digest = await originalDigest.call(crypto.subtle, ...args);
    started.resolve();
    await pending.promise;
    return digest;
  };
  try {
    const copy = copyText("synthetic-pending-digest");
    await started.promise;
    await clearClipboard();
    pending.resolve();
    await copy;
    calls.length = 0;
    await clearClipboardIfUnchanged();
    assert.deepEqual(calls, []);
    assert.equal(clipboardText, "");
  } finally {
    pending.resolve();
    crypto.subtle.digest = originalDigest;
    await clearClipboard();
    mock.timers.reset();
  }
});

test("unreadable browser clipboard is preserved and its remembered fingerprint released", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  nativeConditional = "absent";
  const originalRead = navigator.clipboard.readText;
  try {
    await copyText("synthetic-unreadable-copy");
    navigator.clipboard.readText = async () => {
      throw new Error("synthetic-read-denied");
    };
    await clearClipboardIfUnchanged();
    assert.equal(clipboardText, "synthetic-unreadable-copy");
    calls.length = 0;
    await clearClipboardIfUnchanged();
    assert.deepEqual(calls, []);
  } finally {
    navigator.clipboard.readText = originalRead;
    nativeConditional = "replace";
    await clearClipboard();
    mock.timers.reset();
  }
});

test("failed legacy copy reports failure and always removes the plaintext textarea", async () => {
  const originalClipboard = navigator.clipboard;
  const originalDocument = globalThis.document;
  navigator.clipboard = undefined;
  let removed = false;
  const textarea = {
    value: "",
    style: {},
    select() {},
    remove() {
      removed = true;
    },
  };
  globalThis.document = {
    createElement: () => textarea,
    body: { appendChild() {} },
    execCommand: () => false,
  };
  try {
    await assert.rejects(copyText("synthetic-rejected-copy"));
    assert.equal(removed, true);
    assert.equal(textarea.value, "");
    removed = false;
    document.execCommand = () => {
      throw new Error("synthetic-copy-error");
    };
    await assert.rejects(copyText("synthetic-thrown-copy"), /synthetic-copy-error/);
    assert.equal(removed, true);
    assert.equal(textarea.value, "");
  } finally {
    navigator.clipboard = originalClipboard;
    globalThis.document = originalDocument;
    await clearClipboard();
  }
});

test("lock clears a clipboard write that was already pending when locking started", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  const originalWrite = navigator.clipboard.writeText;
  const pending = deferred();
  navigator.clipboard.writeText = async (text) => {
    if (text === "synthetic-pending-lock-copy") await pending.promise;
    clipboardText = text;
  };
  try {
    const copy = copyText("synthetic-pending-lock-copy");
    const lockClear = clearClipboard();
    await new Promise(setImmediate);
    pending.resolve();
    await Promise.all([copy, lockClear]);
    assert.equal(clipboardText, "", "the delayed write must not restore a secret after lock clear");
    calls.length = 0;
    await clearClipboardIfUnchanged();
    assert.deepEqual(calls, [], "the cancelled copy cannot install a later ownership timer");
  } finally {
    pending.resolve();
    navigator.clipboard.writeText = originalWrite;
    await clearClipboard();
    mock.timers.reset();
  }
});

test("a delayed earlier clipboard write cannot overwrite a later copy", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  const originalWrite = navigator.clipboard.writeText;
  const pending = deferred();
  navigator.clipboard.writeText = async (text) => {
    if (text === "synthetic-earlier-copy") await pending.promise;
    clipboardText = text;
  };
  try {
    const earlier = copyText("synthetic-earlier-copy");
    const later = copyText("synthetic-later-copy");
    await new Promise(setImmediate);
    pending.resolve();
    await Promise.all([earlier, later]);
    assert.equal(clipboardText, "synthetic-later-copy");
  } finally {
    pending.resolve();
    navigator.clipboard.writeText = originalWrite;
    await clearClipboard();
    mock.timers.reset();
  }
});

test("locking cancels queued copies and closes vaults while the active write is pending", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  const { lockVault } = await import("../src/lib/services/security.ts");
  const { vault } = await import("../src/lib/services/vault.ts");
  const originalWrite = navigator.clipboard.writeText;
  const pending = deferred();
  const writes = [];
  let closed = false;
  const close = mock.method(vault, "closeAll", async () => {
    closed = true;
  });
  navigator.clipboard.writeText = async (text) => {
    writes.push(text);
    if (text === "synthetic-active-write") await pending.promise;
    clipboardText = text;
  };
  appSettings.updateSecurity("clearOnLock", true);
  try {
    const active = copyText("synthetic-active-write");
    const queued = copyText("synthetic-cancelled-write");
    const locking = lockVault();
    await new Promise(setImmediate);
    assert.equal(closed, true, "vault closure must not wait for Clipboard API completion");
    pending.resolve();
    await Promise.all([active, queued, locking]);
    assert.equal(clipboardText, "");
    assert.equal(writes.includes("synthetic-cancelled-write"), false);
  } finally {
    pending.resolve();
    navigator.clipboard.writeText = originalWrite;
    close.mock.restore();
    await clearClipboard();
    appSettings.updateSecurity("clearOnLock", DEFAULT_SECURITY_SETTINGS.clearOnLock);
    mock.timers.reset();
  }
});

test("a failed pending write does not poison the next queued copy", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  const originalWrite = navigator.clipboard.writeText;
  const pending = deferred();
  navigator.clipboard.writeText = async (text) => {
    if (text === "synthetic-failed-write") {
      await pending.promise;
      throw new Error("synthetic-write-rejected");
    }
    clipboardText = text;
  };
  try {
    const failure = assert.rejects(copyText("synthetic-failed-write"), /synthetic-write-rejected/);
    const later = copyText("synthetic-after-failure");
    pending.resolve();
    await Promise.all([failure, later]);
    assert.equal(clipboardText, "synthetic-after-failure");
  } finally {
    pending.resolve();
    navigator.clipboard.writeText = originalWrite;
    await clearClipboard();
    mock.timers.reset();
  }
});

test("an in-flight native wipe finishes before a newer identical renderer copy writes", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  const originalInvoke = globalThis.__TAURI_INTERNALS__.invoke;
  const pending = deferred();
  const started = deferred();
  globalThis.__TAURI_INTERNALS__.invoke = async (command, args) => {
    if (command === "clipboard_clear_if_matches") {
      started.resolve();
      await pending.promise;
      clipboardText = "";
      return true;
    }
    return originalInvoke(command, args);
  };
  try {
    await copyText("synthetic-identical-native-write");
    const clearing = clearClipboardIfUnchanged();
    await started.promise;
    const later = copyText("synthetic-identical-native-write");
    pending.resolve();
    await Promise.all([clearing, later]);
    assert.equal(clipboardText, "synthetic-identical-native-write");
  } finally {
    pending.resolve();
    globalThis.__TAURI_INTERNALS__.invoke = originalInvoke;
    await clearClipboard();
    mock.timers.reset();
  }
});

test("an idle copy starts the platform write synchronously after the consumer guard", async () => {
  mock.timers.enable({ apis: ["setTimeout"] });
  const originalWrite = navigator.clipboard.writeText;
  let started = false;
  navigator.clipboard.writeText = async (text) => {
    started = true;
    clipboardText = text;
  };
  try {
    const copying = copyText("synthetic-immediate-write");
    assert.equal(started, true);
    await copying;
  } finally {
    navigator.clipboard.writeText = originalWrite;
    await clearClipboard();
    mock.timers.reset();
  }
});
