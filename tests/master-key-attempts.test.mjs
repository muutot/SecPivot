import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { register } from "node:module";
import { after, test } from "node:test";
import { runInNewContext } from "node:vm";
import ts from "typescript";
import { get } from "svelte/store";
import {
  MasterKeyAttempts,
  normalizeMasterKeyTries,
} from "../src/lib/utils/master-key-attempts.ts";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.window = globalThis;
let persisted;
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    if (command === "set_config") return (persisted = structuredClone(args.config));
    if (command === "get_config") return structuredClone(persisted);
    if (command === "remote_secret_present") return { accessKey: false, secretKey: false };
    throw new Error(`Unexpected ${command}`);
  },
};
const { appSettings, normalizeSettings, DEFAULT_APP_SETTINGS } =
  await import("../src/lib/services/settings.ts");
after(() => appSettings.destroy());

test("official default and positive int32 counts normalize and survive actual settings persistence", async () => {
  assert.equal(DEFAULT_APP_SETTINGS.security.masterKeyTries, 3);
  for (const value of [undefined, null, "3", {}, NaN, Infinity]) {
    assert.equal(normalizeMasterKeyTries(value), 3);
    assert.equal(
      normalizeSettings({ security: { masterKeyTries: value } }).security.masterKeyTries,
      3,
    );
  }
  for (const [value, expected] of [
    [-2, 1],
    [0, 1],
    [2.5, 3],
    [7, 7],
    [2_147_483_648, 2_147_483_647],
  ]) {
    appSettings.updateSecurity("masterKeyTries", value);
    await appSettings.flush();
    assert.equal(persisted.security.masterKeyTries, expected);
    await appSettings.initialize();
    assert.equal(get(appSettings).security.masterKeyTries, expected);
  }
});

test("a prompt admits exactly its configured attempts; unsubmitted work refunds its slot", () => {
  for (const limit of [1, 3, 7]) {
    const attempts = new MasterKeyAttempts();
    attempts.start(limit);
    const unused = attempts.reserve();
    assert.equal(attempts.reserve(), null);
    assert.equal(attempts.finish(unused, false).exhausted, false);
    for (let n = 1; n <= limit; n++) {
      const ticket = attempts.reserve();
      assert.equal(ticket.ordinal, n);
      assert.equal(attempts.finish(ticket).exhausted, n === limit);
      assert.equal(attempts.finish(ticket), null);
    }
    assert.equal(attempts.reserve(), null);
  }
});

test("reopening and disposal invalidate previous tickets without changing a new prompt", () => {
  const attempts = new MasterKeyAttempts();
  attempts.start(1);
  const old = attempts.reserve();
  attempts.start(3);
  const current = attempts.reserve();
  assert.equal(attempts.isPending(old), false);
  assert.equal(attempts.finish(old, false), null);
  assert.equal(attempts.finish(current).exhausted, false);
  const pending = attempts.reserve();
  attempts.close();
  assert.equal(attempts.finish(pending), null);
  assert.equal(attempts.reserve(), null);
});

function fixture(kind, limit = 3) {
  const names =
    kind === "welcome"
      ? [
          "startOpenPrompt",
          "clearCredentialPrompt",
          "cancelCredentialPrompt",
          "failedOpen",
          "confirmOpen",
          "confirmRemoteOpen",
          "openRecent",
          "selectRemoteKey",
          "switchRemoteTab",
        ]
      : ["restartUnlock", "failedUnlock", "unlock", "unlockWithHello"];
  const path = kind === "welcome" ? "VaultWelcome" : "LockScreen";
  const script = readFileSync(`src/lib/components/${path}.svelte`, "utf8").match(
    /<script lang="ts">([^]*?)<\/script>/,
  )[1];
  const ast = ts.createSourceFile("host.ts", script, ts.ScriptTarget.Latest, true);
  const functions = ast.statements.filter(
    (node) => ts.isFunctionDeclaration(node) && names.includes(node.name.text),
  );
  assert.equal(functions.length, names.length);
  const cleanup = ast.statements.filter(
    (node) =>
      ts.isExpressionStatement(node) &&
      ts.isCallExpression(node.expression) &&
      node.expression.expression.getText(ast) === "onDestroy",
  );
  assert.equal(cleanup.length, 1);
  const executable =
    [...functions, ...cleanup].map((node) => node.getText(ast)).join("\n") +
    `\nexports.api={${names.join(",")}};`;
  const settings = { security: { masterKeyTries: limit }, general: { recentFileNames: {} } };
  const calls = [],
    saved = [],
    cleanups = [];
  const context = {
    exports: {},
    openAttempts: new MasterKeyAttempts(),
    settings,
    appSettings: {},
    get: () => settings,
    lang: "en",
    modal: "none",
    path: "",
    pendingName: null,
    pendingPersistable: undefined,
    password: "",
    confirm: "",
    keyfilePath: "",
    showPassword: false,
    isDemo: false,
    busy: false,
    error: "",
    attemptNotice: "",
    attemptsExhausted: false,
    remoteKey: "",
    remoteMode: "memory",
    remoteTab: "open",
    loadRemoteObjects: async () => {},
    remembered: { path: "C:/synthetic/a.kdbx", fileName: "a.kdbx" },
    helloAvailable: true,
    onDestroy: (run) => cleanups.push(run),
    t: (_, key, args) => `${key}${args ? ` ${args.count}` : ""}`,
    rememberCredential: (...args) => saved.push(args),
    opened: 0,
    onopened: () => context.opened++,
    invoke: async () => context.savedCredential,
    savedCredential: { password: "synthetic-saved-only" },
    vault: {
      open: async (...args) => {
        calls.push({ kind: "local", args });
        return context.outcome();
      },
      openRemote: async (...args) => {
        calls.push({ kind: "remote", args });
        return context.outcome();
      },
    },
    outcome: async () => {
      throw new Error("Synthetic native rejection");
    },
  };
  runInNewContext(
    ts.transpileModule(executable, { compilerOptions: { target: ts.ScriptTarget.ESNext } })
      .outputText,
    context,
  );
  const api = context.exports.api;
  if (kind === "lock") api.restartUnlock();
  return { context, api, calls, saved, cleanup: () => cleanups.forEach((run) => run()) };
}
function credentials(context, keyfile = false) {
  context.password = keyfile ? "" : "synthetic-master-only";
  context.keyfilePath = keyfile ? "C:/synthetic/keyfile" : "";
  context.showPassword = true;
}

test("actual local open handler ends after three failures, clears fields and permits a new opening intent", async () => {
  const f = fixture("welcome");
  f.api.openRecent("C:/synthetic/a.kdbx");
  await f.api.confirmOpen(); // empty preflight is not a submitted attempt
  assert.equal(f.calls.length, 0);
  for (let n = 1; n <= 3; n++) {
    credentials(f.context);
    await f.api.confirmOpen();
    assert.equal(f.calls.length, n);
    assert.equal(f.context.modal, n === 3 ? "none" : "open");
  }
  assert.equal(f.context.password, "");
  assert.equal(f.context.keyfilePath, "");
  assert.equal(f.context.showPassword, false);
  assert.match(f.context.attemptNotice, /masterKeyAttemptsExceeded 3/);
  credentials(f.context);
  await f.api.confirmOpen();
  assert.equal(f.calls.length, 3);
  f.api.openRecent("C:/synthetic/a.kdbx");
  credentials(f.context);
  f.context.outcome = async () => ({});
  await f.api.confirmOpen();
  assert.equal(f.calls.length, 4);
  assert.equal(f.context.opened, 1);
  assert.equal(f.saved[0][1], "synthetic-master-only");
  assert.equal(f.context.attemptNotice, "");
  f.cleanup();
});

test("actual remote handler shares the budget; selecting another database resets and clears credentials", async () => {
  const f = fixture("welcome");
  f.context.modal = "remote";
  f.api.selectRemoteKey("a.kdbx");
  credentials(f.context, true);
  await f.api.confirmRemoteOpen();
  f.api.selectRemoteKey("b.kdbx");
  assert.equal(f.context.keyfilePath, "");
  for (let n = 1; n <= 3; n++) {
    credentials(f.context, true);
    await f.api.confirmRemoteOpen();
    assert.equal(f.context.modal, n === 3 ? "none" : "remote");
  }
  assert.equal(f.calls.length, 4);
  assert(f.calls.every((call) => call.kind === "remote"));
  assert.equal(f.saved.length, 0);
  credentials(f.context, true);
  f.api.switchRemoteTab("create");
  assert.equal(f.context.keyfilePath, "");
  assert.equal(f.context.openAttempts.reserve(), null);
  f.api.switchRemoteTab("open");
  f.api.selectRemoteKey("b.kdbx");
  credentials(f.context, true);
  await f.api.confirmRemoteOpen();
  assert.equal(f.calls.length, 5);
  f.cleanup();
});

test("duplicate submissions and cancellation cannot change a pending actual prompt", async () => {
  const f = fixture("welcome");
  f.api.openRecent("C:/synthetic/a.kdbx");
  credentials(f.context);
  let reject;
  f.context.outcome = () => new Promise((_, r) => (reject = r));
  const pending = f.api.confirmOpen();
  await f.api.confirmOpen();
  f.api.cancelCredentialPrompt();
  f.api.openRecent("C:/synthetic/b.kdbx");
  assert.equal(f.context.path, "C:/synthetic/a.kdbx");
  assert.equal(f.context.modal, "open");
  assert.equal(f.calls.length, 1);
  f.cleanup();
  f.context.error = "new view";
  reject(new Error("obsolete failure"));
  await pending;
  assert.equal(f.context.error, "new view");
});

test("actual lock handler counts manual and Hello opens together and explicitly restarts", async () => {
  const f = fixture("lock");
  credentials(f.context);
  await f.api.unlock();
  await f.api.unlockWithHello();
  await f.api.unlockWithHello();
  assert.equal(f.calls.length, 3);
  assert.equal(f.context.attemptsExhausted, true);
  assert.equal(f.context.password, "");
  assert.equal(f.context.keyfilePath, "");
  assert.equal(f.context.showPassword, false);
  await f.api.unlockWithHello();
  assert.equal(f.calls.length, 3);
  f.api.restartUnlock();
  credentials(f.context, true);
  await f.api.unlock();
  assert.equal(f.calls.length, 4);
  assert.equal(f.context.attemptsExhausted, false);
  f.cleanup();
});

test("Hello retrieval failures do not spend an open attempt, and late saved credentials cannot submit", async () => {
  const f = fixture("lock", 1);
  f.context.savedCredential = null;
  await f.api.unlockWithHello();
  assert.equal(f.calls.length, 0);
  assert.equal(f.context.attemptsExhausted, false);
  credentials(f.context);
  await f.api.unlock();
  assert.equal(f.calls.length, 1);
  assert.equal(f.context.attemptsExhausted, true);
  f.api.restartUnlock();
  let resolve;
  f.context.invoke = () => new Promise((r) => (resolve = r));
  const pending = f.api.unlockWithHello();
  f.cleanup();
  resolve({ password: "synthetic-late-only" });
  await pending;
  assert.equal(f.calls.length, 1);
});
