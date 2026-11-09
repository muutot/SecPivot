import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { register } from "node:module";
import test from "node:test";
import vm from "node:vm";
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
globalThis.__TAURI_INTERNALS__ = {
  invoke: (command, args) => {
    calls.push({ command, args });
    return respond(command, args);
  },
};
const { save } = await import("@tauri-apps/plugin-dialog");
const { invoke } = await import("@tauri-apps/api/core");
const io = await import("../src/lib/services/io.ts");
const { vault } = await import("../src/lib/services/vault.ts");
const { SessionViewGuard, KeyedViewGuard, awaitCurrentView } =
  await import("../src/lib/utils/session-state.ts");

const cases = [
  ["routes/+page.svelte", "handleExportCsv", "export_csv", "text/csv"],
  ["routes/+page.svelte", "handleExportXml", "export_xml", "application/xml"],
  ["routes/+page.svelte", "confirmExportEmergency", "export_emergency_sheet", "text/html"],
  [
    "lib/components/EntryDetail.svelte",
    "saveAttachment",
    "save_attachment",
    "application/octet-stream",
  ],
  [
    "lib/components/AttachmentPreviewDialog.svelte",
    "saveToDisk",
    "save_attachment",
    "application/octet-stream",
  ],
];

async function fixture(file, name, agent = "Android") {
  calls.length = 0;
  navigator.userAgent = agent;
  const source = await readFile(new URL(`../src/${file}`, import.meta.url), "utf8");
  const match = source.match(new RegExp(`^  async function ${name}\\([^]*?^  \\}`, "m"));
  assert.ok(match, `production ${name} handler must be extracted`);
  const sessionView = new SessionViewGuard();
  sessionView.activate("owner");
  const detailView = new KeyedViewGuard();
  detailView.activate("owner:entry");
  const dialogView = new KeyedViewGuard();
  dialogView.activate("owner:entry:attachment");
  const notices = [];
  const context = vm.createContext({
    sessionView,
    detailView,
    dialogView,
    awaitCurrentView,
    vault,
    invoke,
    save,
    saveDialog: save,
    pickSaveFile: io.pickSaveFile,
    isTauriRuntime: () => true,
    isAndroid: () => true,
    currentVault: { fileName: "synthetic.kdbx" },
    settings: { general: { language: "en" } },
    window: { confirm: () => true },
    t: (_lang, key) => key,
    lang: "en",
    flash: (message) => notices.push(message),
    sessionId: "owner",
    detailSessionId: () => "owner",
    entry: { uuid: "entry" },
    entryUuid: "entry",
    attachment: { name: "synthetic.bin" },
    savingToDisk: false,
    externalError: "",
    onsaved: () => {},
    emergencyIncludePasswords: true,
    emergencyExportOpen: true,
  });
  vm.runInContext(
    ts.transpileModule(`${match[0]}\nglobalThis.run = ${name};`, {
      compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None },
    }).outputText,
    context,
  );
  return { context, sessionView, detailView, dialogView, notices };
}

for (const [file, name, command, mime] of cases) {
  test(`${name} uses SAF and writes only the captured session and selected URI`, async () => {
    const { context, notices } = await fixture(file, name);
    respond = async (call) =>
      call === "plugin:saf|create_document"
        ? { uri: "content://synthetic/document/42" }
        : call === "plugin:dialog|save"
          ? "synthetic-export.bin"
          : null;
    await context.run("synthetic.bin");
    assert.equal(calls[0].command, "plugin:saf|create_document");
    assert.ok(calls[0].args.mimeTypes.includes(mime));
    const write = calls.find((call) => call.command === command);
    assert.ok(write);
    assert.equal(write.args.sessionId, "owner");
    assert.equal(write.args.path ?? write.args.dest, "content://synthetic/document/42");
    if (command === "export_emergency_sheet") assert.equal(write.args.includePasswords, true);
    assert.ok(notices.length <= 1);
  });

  test(`${name} cancels obsolete picker results before secret extraction or write`, async () => {
    const { context, sessionView, detailView, dialogView, notices } = await fixture(file, name);
    let release;
    respond = () =>
      new Promise((resolve) => {
        release = resolve;
      });
    const pending = context.run("synthetic.bin");
    sessionView.activate("other");
    sessionView.activate("owner");
    detailView.activate("other");
    detailView.activate("owner:entry");
    dialogView.activate("other");
    dialogView.activate("owner:entry:attachment");
    context.emergencyIncludePasswords = false;
    release(
      calls[0].command === "plugin:saf|create_document"
        ? { uri: "content://synthetic/document/42" }
        : "synthetic-export.bin",
    );
    await pending;
    assert.equal(calls.length, 1);
    assert.equal(notices.length, 0);
    assert.equal(context.emergencyExportOpen, true);
  });

  test(`${name} treats user cancellation as no export`, async () => {
    const { context, notices } = await fixture(file, name);
    respond = async () => null;
    await context.run("synthetic.bin");
    assert.equal(calls.length, 1);
    assert.equal(notices.length, 0);
    assert.equal(context.emergencyExportOpen, true);
    assert.equal(context.savingToDisk, false);
  });

  test(`${name} keeps the desktop save dialog and selected path`, async () => {
    const { context } = await fixture(file, name, "Windows");
    respond = async (call) => (call === "plugin:dialog|save" ? "synthetic-export.bin" : null);
    await context.run("synthetic.bin");
    assert.equal(calls[0].command, "plugin:dialog|save");
    assert.equal(typeof calls[0].args.options.defaultPath, "string");
    const write = calls.find((call) => call.command === command);
    assert.ok(write);
    assert.equal(write.args.sessionId, "owner");
    assert.equal(write.args.path ?? write.args.dest, "synthetic-export.bin");
  });
}

test("CSV and XML refusal never opens a chooser or resolves secrets", async () => {
  for (const [file, name] of cases.slice(0, 2)) {
    const { context } = await fixture(file, name);
    context.window.confirm = () => false;
    respond = async () => {
      throw new Error("must not invoke");
    };
    await context.run();
    assert.equal(calls.length, 0);
  }
});

test("emergency export snapshots password opt-in before the native picker", async () => {
  const { context } = await fixture(cases[2][0], cases[2][1]);
  let release;
  respond = (call) =>
    call === "plugin:saf|create_document"
      ? new Promise((resolve) => {
          release = resolve;
        })
      : Promise.resolve(null);
  const pending = context.run();
  context.emergencyIncludePasswords = false;
  release({ uri: "content://synthetic/document/42" });
  await pending;
  assert.equal(calls[1].args.includePasswords, true);
});

test("vault creation retains the KDBX MIME default", async () => {
  const { createDocument } = await import("../src/lib/services/saf.ts");
  calls.length = 0;
  respond = async () => null;
  await createDocument("vault.kdbx");
  assert.equal(calls[0].command, "plugin:saf|create_document");
  assert.deepEqual(calls[0].args.mimeTypes, ["application/octet-stream", "application/x-keepass2"]);
});
