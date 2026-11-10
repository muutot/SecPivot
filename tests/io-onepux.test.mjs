import assert from "node:assert/strict";
import { register } from "node:module";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import test from "node:test";
import ts from "typescript";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {} };
globalThis.window = globalThis;
Object.defineProperty(globalThis, "navigator", {
  configurable: true,
  value: { userAgent: "Android" },
});
let respond;
const calls = [];
const native = {
  invoke: (command, args) => {
    calls.push({ command, args });
    return respond(command, args);
  },
};
globalThis.__TAURI_INTERNALS__ = native;
const { importOnePux } = await import("../src/lib/services/io.ts");
const { SessionViewGuard, LatestOperationGuard } =
  await import("../src/lib/utils/session-state.ts");

const state = {
  revision: 1,
  path: "synthetic.kdbx",
  fileName: "synthetic.kdbx",
  dirty: true,
  readOnly: false,
  root: { uuid: "root", name: "", children: [], entries: [] },
};
const result = {
  state,
  counts: { accounts: 1, vaults: 2, items: 3, archived: 1, attachments: 2, unreferencedFiles: 1 },
};

function fixture(agent = "Android") {
  calls.length = 0;
  navigator.userAgent = agent;
  globalThis.__TAURI_INTERNALS__ = native;
  const sessionView = new SessionViewGuard();
  sessionView.activate("owner");
  const busy = [],
    notices = [];
  const host = {
    sessionView,
    operations: new LatestOperationGuard(),
    setBusy: (value) => busy.push(value),
    notify: (message, kind) => notices.push({ message, kind }),
    currentState: () => state,
  };
  return {
    host,
    sessionView,
    busy,
    notices,
    run: () => importOnePux(host, sessionView.capture(), "target-group"),
  };
}

test("the production page and Android service pass only stable selection identity to native import", async () => {
  const f = fixture();
  respond = async (command) =>
    command === "plugin:saf|open_document"
      ? { uri: "content://synthetic/document/42", name: "Export.1PUX" }
      : result;
  const script = readFileSync(new URL("../src/routes/+page.svelte", import.meta.url), "utf8").match(
    /<script lang="ts">([\s\S]*?)<\/script>/,
  )[1];
  const ast = ts.createSourceFile("page.ts", script, ts.ScriptTarget.Latest, true);
  const handler = ast.statements.find(
    (node) => ts.isFunctionDeclaration(node) && node.name?.text === "handleImportOnePux",
  );
  assert.ok(handler);
  const context = {
    currentVault: state,
    sessionView: f.sessionView,
    selectedGroup: "target-group",
    ioHost: f.host,
    runImportOnePux: importOnePux,
  };
  runInNewContext(
    ts.transpileModule(handler.getText(ast), {
      compilerOptions: { target: ts.ScriptTarget.ES2022 },
    }).outputText,
    context,
  );
  await context.handleImportOnePux();
  assert.deepEqual(
    calls.map((call) => call.command),
    ["plugin:saf|open_document", "import_1pux_file"],
  );
  assert.deepEqual(calls[1].args, {
    path: "content://synthetic/document/42",
    fileName: "Export.1PUX",
    targetGroupUuid: "target-group",
    sessionId: "owner",
  });
  assert.deepEqual(f.busy, [true, false]);
  assert.ok(f.notices[0].message.includes("3") && f.notices[0].message.includes("1"));
});

test("desktop uses the 1PUX filter and never requests plaintext file reading", async () => {
  const f = fixture("Windows");
  respond = async (command) => (command === "plugin:dialog|open" ? "synthetic.1pux" : result);
  await f.run();
  assert.equal(calls[0].command, "plugin:dialog|open");
  assert.deepEqual(calls[0].args.options.filters[0].extensions, ["1pux"]);
  assert.equal(calls[1].args.fileName, null);
  assert.ok(
    calls.every(
      (call) =>
        !["read_text_file", "parse_1pif", "import_entries", "add_group"].includes(call.command),
    ),
  );
});

test("picker cancellation performs no native import and clears current busy state", async () => {
  const f = fixture();
  respond = async () => null;
  await f.run();
  assert.equal(calls.length, 1);
  assert.equal(f.notices.length, 0);
  assert.deepEqual(f.busy, [true, false]);
});

test("A to B to A while picking cannot read or import the old selection", async () => {
  const f = fixture();
  let release;
  respond = () =>
    new Promise((resolve) => {
      release = resolve;
    });
  const pending = f.run();
  f.sessionView.activate("other");
  f.sessionView.activate("owner");
  release({ uri: "content://synthetic/old", name: "old.1pux" });
  await pending;
  assert.equal(calls.length, 1);
  assert.equal(f.notices.length, 0);
  assert.deepEqual(f.busy, [true]);
});

test("a superseded same-view picker cannot start a second native import", async () => {
  const f = fixture();
  let release;
  let picks = 0;
  respond = (command) =>
    command === "plugin:saf|open_document"
      ? ++picks === 1
        ? new Promise((resolve) => {
            release = resolve;
          })
        : Promise.resolve({ uri: "content://synthetic/current", name: "current.1pux" })
      : Promise.resolve(result);
  const old = f.run();
  await f.run();
  release({ uri: "content://synthetic/old", name: "old.1pux" });
  await old;
  const imports = calls.filter((call) => call.command === "import_1pux_file");
  assert.equal(imports.length, 1);
  assert.equal(imports[0].args.path, "content://synthetic/current");
  assert.equal(f.notices.length, 1);
  assert.deepEqual(f.busy, [true, true, false]);
});

test("late completion or rejection cannot publish feedback or busy reset into a new view", async () => {
  for (const fail of [false, true]) {
    const f = fixture();
    let finish;
    respond = (command) =>
      command === "plugin:saf|open_document"
        ? Promise.resolve({ uri: "content://synthetic/current", name: "current.1pux" })
        : new Promise((resolve, reject) => {
            finish = () => (fail ? reject(new Error("synthetic failure")) : resolve(result));
          });
    const pending = f.run();
    await new Promise((resolve) => setImmediate(resolve));
    f.sessionView.activate("other");
    finish();
    await pending;
    assert.equal(f.notices.length, 0);
    assert.deepEqual(f.busy, [true]);
  }
});

test("current import failures are reported and browser mode never opens a native picker", async () => {
  const f = fixture();
  respond = async (command) => {
    if (command === "plugin:saf|open_document")
      return { uri: "content://synthetic/current", name: "current.1pux" };
    throw new Error("synthetic import rejection");
  };
  await f.run();
  assert.equal(f.notices[0].kind, "error");
  assert.deepEqual(f.busy, [true, false]);
  const browser = fixture();
  delete globalThis.__TAURI_INTERNALS__;
  await browser.run();
  assert.equal(calls.length, 0);
  assert.equal(browser.notices[0].kind, "error");
  assert.ok(!browser.notices[0].message.includes("Error:"));
  assert.deepEqual(browser.busy, [true, false]);
});
