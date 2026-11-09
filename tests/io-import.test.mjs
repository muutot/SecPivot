import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";

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
const { importRowsToEntries, importEntries } = await import("../src/lib/services/io.ts");
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
