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
