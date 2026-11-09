import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {} };
globalThis.window = globalThis;
const calls = [];
const state = {
  revision: 1,
  path: "synthetic.kdbx",
  fileName: "synthetic.kdbx",
  dirty: true,
  readOnly: false,
  root: { uuid: "root", name: "", children: [], entries: [] },
};
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    calls.push({ command, args });
    return state;
  },
};
const { importRowsToEntries, importEntries, csvToImportEntries } =
  await import("../src/lib/services/io.ts");
const { t } = await import("../src/lib/i18n/index.ts");
const { SessionViewGuard, LatestOperationGuard } =
  await import("../src/lib/utils/session-state.ts");
const { vault } = await import("../src/lib/services/vault.ts");

test("structured import retains protected flags and OTP through the real bulk IPC pipeline", async () => {
  const customFields = [
    { name: "1Password PIN", value: "synthetic-pin", protected: true },
    { name: "1Password Label", value: "public", protected: false },
  ];
  const row = {
    group: "",
    title: "Synthetic Login",
    username: "demo",
    password: "synthetic-password",
    url: "https://one.example https://two.example",
    notes: "note",
    totp: "JBSWY3DPEHPK3PXP",
    customFields,
  };
  const sessionView = new SessionViewGuard();
  sessionView.activate("owner");
  const notices = [];
  await importEntries(
    {
      sessionView,
      operations: new LatestOperationGuard(),
      setBusy: () => {},
      currentState: () => state,
      notify: (message) => notices.push(message),
    },
    importRowsToEntries([row]),
    sessionView.capture(),
    null,
  );
  const call = calls.find((c) => c.command === "import_entries");
  assert.ok(call, "the real import pipeline must submit the batch");
  assert.equal(call.args.sessionId, "owner");
  assert.deepEqual(call.args.inputs[0].customFields, customFields);
  assert.equal(call.args.inputs[0].totp, row.totp);
  assert.equal(call.args.inputs[0].password, row.password);
  assert.equal(call.args.inputs[0].url, row.url);
  assert.equal(notices.length, 1);
});

test("structured import preserves explicit quality flags and legacy defaults in each bulk row", async () => {
  calls.length = 0;
  const row = {
    group: "",
    title: "Synthetic",
    username: "",
    password: "",
    url: "",
    notes: "",
    customFields: [],
  };
  const sessionView = new SessionViewGuard();
  sessionView.activate("quality-owner");
  await importEntries(
    {
      sessionView,
      operations: new LatestOperationGuard(),
      setBusy: () => {},
      currentState: () => state,
      notify: () => {},
    },
    importRowsToEntries([{ ...row, qualityCheck: false }, { ...row, qualityCheck: true }, row]),
    sessionView.capture(),
    null,
  );
  const call = calls.find((c) => c.command === "import_entries");
  assert.ok(call);
  assert.deepEqual(
    call.args.inputs.map((input) => input.qualityCheck),
    [false, true, undefined],
  );
});

test("browser bulk import retains quality metadata with KeePass defaults", async () => {
  const native = globalThis.__TAURI_INTERNALS__;
  delete globalThis.__TAURI_INTERNALS__;
  try {
    await vault.open("synthetic-import-browser.kdbx", "synthetic-master");
    const row = {
      groupUuid: "root",
      title: "Synthetic import metadata",
      username: "",
      password: "",
      url: "",
      notes: "",
    };
    const result = await vault.addEntries([
      { ...row, qualityCheck: false },
      { ...row, qualityCheck: true },
      row,
    ]);
    const imported = result.root.entries.filter((entry) => entry.title === row.title);
    assert.deepEqual(
      imported.map((entry) => entry.qualityCheck),
      [false, true, true],
    );
    await vault.closeAll();
  } finally {
    globalThis.__TAURI_INTERNALS__ = native;
  }
});

test("CSV conversion rejects a whole malformed document with localized content-free errors", () => {
  const marker = "synthetic-sensitive-marker";
  for (const language of ["en", "zh-CN"]) {
    for (const text of [
      `Group,Title\r\n,valid\r\n,"${marker}`,
      `Group,Title\r\n,"${marker}"junk\r\n`,
    ]) {
      assert.throws(
        () => csvToImportEntries(text, language),
        (error) =>
          error.message === t(language, "error.invalidCsv") && !error.message.includes(marker),
      );
    }
  }
});

function csvPageFixture(pick) {
  calls.length = 0;
  const source = readFileSync(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
  const script = source.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast = ts.createSourceFile("page.ts", script, ts.ScriptTarget.Latest, true);
  const handler = ast.statements.find(
    (node) => ts.isFunctionDeclaration(node) && node.name?.text === "handleImportCsv",
  );
  assert.ok(handler);
  const js = ts.transpileModule(handler.getText(ast), {
    compilerOptions: { target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const sessionView = new SessionViewGuard();
  sessionView.activate("csv-owner");
  const notices = [];
  const host = {
    sessionView,
    operations: new LatestOperationGuard(),
    setBusy: () => {},
    currentState: () => state,
    notify: (message) => notices.push(message),
  };
  const context = {
    currentVault: state,
    sessionView,
    selectedGroup: null,
    ioHost: host,
    settings: { general: { language: "zh-CN" } },
    // The extracted handler and real service share one Error constructor in the app.
    Error,
    t,
    csvToImportEntries,
    runImportEntries: importEntries,
    pickImportFile: pick,
    flash: (message) => notices.push(message),
  };
  runInNewContext(js, context);
  return { context, sessionView, notices, run: () => context.handleImportCsv() };
}

test("the actual CSV page handler reports malformed quoting and preserves valid multiline fields", async () => {
  for (const text of ['Group,Title\r\n,valid\r\n,"truncated', 'Group,Title\r\n,"bad"junk']) {
    const f = csvPageFixture(async () => text);
    await f.run();
    assert.equal(calls.length, 0, "malformed input must never create groups or submit entries");
    assert.equal(f.notices.length, 1);
    assert.ok(f.notices[0].includes(t("zh-CN", "error.invalidCsv")));
    assert.ok(
      !f.notices[0].includes("Error:"),
      "localized feedback must not add an English Error prefix",
    );
  }
  const f = csvPageFixture(
    async () =>
      'Group,Title,Username,Password,URL,Notes\r\n,"First,""quoted""",user,,https://synthetic.example,"one\r\ntwo"\r\n',
  );
  await f.run();
  const batch = calls.find((call) => call.command === "import_entries");
  assert.ok(batch);
  assert.equal(batch.args.sessionId, "csv-owner");
  assert.equal(batch.args.inputs.length, 1);
  assert.equal(batch.args.inputs[0].title, 'First,"quoted"');
  assert.equal(batch.args.inputs[0].notes, "one\r\ntwo");
});

test("the actual CSV page handler ignores an old picker completion after A to B to A", async () => {
  let release;
  const f = csvPageFixture(
    () =>
      new Promise((resolve) => {
        release = resolve;
      }),
  );
  const pending = f.run();
  f.sessionView.activate("other-owner");
  f.sessionView.activate("csv-owner");
  release('Group,Title\r\n,"truncated');
  await pending;
  assert.equal(calls.length, 0);
  assert.equal(f.notices.length, 0);
});
