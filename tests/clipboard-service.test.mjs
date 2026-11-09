import { register } from "node:module";
import { test, mock } from "node:test";
import assert from "node:assert/strict";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
let clipboardText = "";
const calls = [];
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
    if (command === "clipboard_read_text") {
      const old = clipboardText;
      clipboardText = "synthetic-foreign-copy";
      return old;
    }
    if (command === "clipboard_clear") {
      clipboardText = "";
      return;
    }
    if (command === "clipboard_clear_if_matches") {
      // Replacement occurs before the single native transaction acquires its
      // OS lock, instead of between an IPC read and a blind IPC clear.
      clipboardText = "synthetic-foreign-copy";
      if (clipboardText !== args.text) return false;
      clipboardText = "";
      return true;
    }
  },
};
const { copyText, clearClipboardIfUnchanged, clearClipboard } =
  await import("../src/lib/utils/clipboard.ts");

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
