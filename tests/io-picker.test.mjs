import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {} };
globalThis.window = globalThis;
let userAgent = "Android";
Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: {
    get userAgent() {
      return userAgent;
    },
  },
});
let respond;
const calls = [];
globalThis.__TAURI_INTERNALS__ = {
  invoke: (command, args) => {
    calls.push({ command, args });
    return respond(command, args);
  },
};
const { pickImportFile } = await import("../src/lib/services/io.ts");
const { SessionViewGuard, LatestOperationGuard } =
  await import("../src/lib/utils/session-state.ts");

function fixture(agent = "Android") {
  calls.length = 0;
  userAgent = agent;
  const sessionView = new SessionViewGuard();
  sessionView.activate("owner");
  const notices = [];
  return {
    sessionView,
    operations: new LatestOperationGuard(),
    currentState: () => null,
    setBusy: () => {},
    notify: (message) => notices.push(message),
    notices,
  };
}

test("Android text import selects a SAF document and forwards its display name with the URI", async () => {
  for (const [extension, mime] of [
    ["csv", "text/csv"],
    ["xml", "application/xml"],
    ["json", "application/json"],
    ["1pif", "application/octet-stream"],
  ]) {
    const host = fixture();
    const uri = "content://synthetic/document/42";
    respond = async (command) =>
      command === "read_text_file" ? "synthetic text" : { uri, name: `export.${extension}` };
    assert.equal(
      await pickImportFile(host, host.sessionView.capture(), [
        { name: "Import", extensions: [extension] },
      ]),
      "synthetic text",
    );
    assert.equal(calls[0].command, "plugin:saf|open_document");
    assert.ok(calls[0].args.mimeTypes.includes(mime));
    assert.ok(calls[0].args.mimeTypes.includes("text/plain"));
    if (extension === "xml") assert.ok(calls[0].args.mimeTypes.includes("text/xml"));
    assert.deepEqual(calls[1], {
      command: "read_text_file",
      args: { path: uri, fileName: `export.${extension}` },
    });
  }
});

test("cancelled Android picker never starts a file read", async () => {
  const host = fixture();
  respond = async () => null;
  assert.equal(
    await pickImportFile(host, host.sessionView.capture(), [{ name: "CSV", extensions: ["csv"] }]),
    null,
  );
  assert.equal(calls.length, 1);
  assert.equal(host.notices.length, 0);
});

for (const agent of ["Android", "Windows"]) {
  test(`${agent} picker results from a previous A view never read plaintext`, async () => {
    const host = fixture(agent);
    let release;
    respond = (command) =>
      command === "read_text_file"
        ? Promise.resolve("obsolete text")
        : new Promise((resolve) => {
            release = resolve;
          });
    const pending = pickImportFile(host, host.sessionView.capture(), [
      { name: "CSV", extensions: ["csv"] },
    ]);
    host.sessionView.activate("other");
    host.sessionView.activate("owner");
    release(
      agent === "Android"
        ? { uri: "content://synthetic/document/42", name: "export.csv" }
        : "export.csv",
    );
    assert.equal(await pending, null);
    assert.equal(calls.filter((call) => call.command === "read_text_file").length, 0);
    assert.equal(host.notices.length, 0);
  });
}

test("desktop import keeps the dialog path and filters", async () => {
  const host = fixture("Windows");
  const filters = [{ name: "CSV", extensions: ["csv"] }];
  respond = async (command) => (command === "read_text_file" ? "current text" : "export.csv");
  assert.equal(await pickImportFile(host, host.sessionView.capture(), filters), "current text");
  assert.equal(calls[0].command, "plugin:dialog|open");
  assert.deepEqual(calls[0].args.options.filters, filters);
  assert.deepEqual(calls[1], { command: "read_text_file", args: { path: "export.csv" } });
});

test("a file-read error reports only in the current view", async () => {
  const host = fixture();
  respond = async (command) => {
    if (command === "read_text_file") throw new Error("synthetic provider error");
    return { uri: "content://synthetic/document/42", name: "export.csv" };
  };
  assert.equal(
    await pickImportFile(host, host.sessionView.capture(), [{ name: "CSV", extensions: ["csv"] }]),
    null,
  );
  assert.equal(host.notices.length, 1);
  assert.match(host.notices[0], /synthetic provider error/);
});
