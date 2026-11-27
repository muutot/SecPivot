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
const { importBitwarden } = await import("../src/lib/services/io.ts");
const { vault } = await import("../src/lib/services/vault.ts");
// This IO host models a listed open session; actual service admission/replacement
// epochs are independently exercised by vault-session-publication.test.mjs.
const openEpoch = Symbol("synthetic-open-session");
vault.getSessionEpoch = () => openEpoch;
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
  counts: { folders: 2, collections: 3, items: 3, unresolvedMemberships: 0 },
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
    requestBitwardenSettings: async () => ({ defaultMatch: 0, equivalentDomains: [] }),
  };
  return {
    host,
    sessionView,
    busy,
    notices,
    run: () => importBitwarden(host, sessionView.capture(), "target-group"),
  };
}

test("the production page and Android service pass only stable selection identity to native import", async () => {
  const f = fixture();
  respond = async (command) =>
    command === "plugin:saf|open_document"
      ? { uri: "content://synthetic/document/42", name: "Export.JSON" }
      : result;
  const script = readFileSync(new URL("../src/routes/+page.svelte", import.meta.url), "utf8").match(
    /<script lang="ts">([\s\S]*?)<\/script>/,
  )[1];
  const ast = ts.createSourceFile("page.ts", script, ts.ScriptTarget.Latest, true);
  const handler = ast.statements.find(
    (node) => ts.isFunctionDeclaration(node) && node.name?.text === "handleImportBitwarden",
  );
  assert.ok(handler);
  const context = {
    currentVault: state,
    sessionView: f.sessionView,
    selectedGroup: "target-group",
    ioHost: f.host,
    runImportBitwarden: importBitwarden,
    isTauriRuntime: () => true,
  };
  runInNewContext(
    ts.transpileModule(handler.getText(ast), {
      compilerOptions: { target: ts.ScriptTarget.ES2022 },
    }).outputText,
    context,
  );
  await context.handleImportBitwarden();
  assert.deepEqual(
    calls.map((call) => call.command),
    ["plugin:saf|open_document", "import_bitwarden_file"],
  );
  assert.deepEqual(calls[1].args, {
    path: "content://synthetic/document/42",
    fileName: "Export.JSON",
    targetGroupUuid: "target-group",
    sessionId: "owner",
    uriSettings: { defaultMatch: 0, equivalentDomains: [] },
  });
  assert.deepEqual(f.busy, [true, false]);
  assert.ok(f.notices[0].message.includes("3"));
});

test("desktop uses the JSON filter and never requests plaintext file reading", async () => {
  const f = fixture("Windows");
  respond = async (command) => (command === "plugin:dialog|open" ? "synthetic.json" : result);
  await f.run();
  assert.equal(calls[0].command, "plugin:dialog|open");
  assert.deepEqual(calls[0].args.options.filters[0].extensions, ["json"]);
  assert.equal(calls[1].args.fileName, null);
  assert.ok(
    calls.every(
      (call) =>
        ![
          "read_text_file",
          "parse_bitwarden_json",
          "parse_1pif",
          "import_entries",
          "add_group",
        ].includes(call.command),
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

test("out-of-scope collection memberships are explicitly reported after a successful import", async () => {
  const f = fixture();
  respond = async (command) =>
    command === "plugin:saf|open_document"
      ? { uri: "content://synthetic/org", name: "organization.json" }
      : { state, counts: { folders: 0, collections: 2, items: 4, unresolvedMemberships: 7 } };
  await f.run();
  assert.equal(f.notices.length, 1);
  assert.ok(f.notices[0].message.includes("4"));
  assert.ok(f.notices[0].message.includes("7"));
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
  release({ uri: "content://synthetic/old", name: "old.json" });
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
        : Promise.resolve({ uri: "content://synthetic/current", name: "current.json" })
      : Promise.resolve(result);
  const old = f.run();
  await f.run();
  release({ uri: "content://synthetic/old", name: "old.json" });
  await old;
  const imports = calls.filter((call) => call.command === "import_bitwarden_file");
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
        ? Promise.resolve({ uri: "content://synthetic/current", name: "current.json" })
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
      return { uri: "content://synthetic/current", name: "current.json" };
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

test("source choices are required and cancellation or invalid choices cannot send an import", async () => {
  for (const choice of [null, { defaultMatch: 6, equivalentDomains: [] }]) {
    const f = fixture();
    respond = async () => ({ uri: "content://synthetic/current", name: "current.json" });
    f.host.requestBitwardenSettings = async () => choice;
    await f.run();
    assert.equal(calls.length, 1);
    assert.deepEqual(f.busy, [true, false]);
    assert.equal(f.notices.length, choice === null ? 0 : 1);
  }
  const f = fixture();
  delete f.host.requestBitwardenSettings;
  await f.run();
  assert.equal(calls.length, 1);
  assert.equal(f.notices[0].kind, "error");
});

test("switching, supersession or same-session replacement while choosing settings cannot import", async () => {
  for (const action of ["switch", "supersede", "replace"]) {
    const f = fixture();
    let release;
    let epoch = Symbol();
    const previous = vault.getSessionEpoch;
    vault.getSessionEpoch = () => epoch;
    try {
      respond = async (command) =>
        command === "plugin:saf|open_document"
          ? { uri: "content://synthetic/current", name: "current.json" }
          : result;
      f.host.requestBitwardenSettings = () => new Promise((resolve) => (release = resolve));
      const old = f.run();
      await new Promise((resolve) => setImmediate(resolve));
      if (action === "switch") {
        f.sessionView.activate("other");
        f.sessionView.activate("owner");
      }
      if (action === "supersede") f.host.operations.begin();
      if (action === "replace") epoch = Symbol();
      release({ defaultMatch: 0, equivalentDomains: [] });
      await old;
      assert.equal(calls.length, 1);
      assert.equal(f.notices.length, 0);
    } finally {
      vault.getSessionEpoch = previous;
    }
  }
});

test("explicit account choices cross IPC detached and never include source rows", async () => {
  const f = fixture();
  const choices = { defaultMatch: 5, equivalentDomains: [["example.com", "example.net"]] };
  f.host.requestBitwardenSettings = async () => choices;
  respond = async (command) =>
    command === "plugin:saf|open_document"
      ? { uri: "content://synthetic/current", name: "current.json" }
      : result;
  await f.run();
  const payload = calls.find((call) => call.command === "import_bitwarden_file").args;
  assert.deepEqual(payload.uriSettings, choices);
  choices.equivalentDomains[0][0] = "changed";
  assert.equal(payload.uriSettings.equivalentDomains[0][0], "example.com");
  assert.deepEqual(Object.keys(payload).sort(), [
    "fileName",
    "path",
    "sessionId",
    "targetGroupUuid",
    "uriSettings",
  ]);
  assert.rejects(() => vault.importBitwarden("synthetic.json", null, "root", undefined));
});
