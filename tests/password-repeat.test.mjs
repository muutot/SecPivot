import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { register } from "node:module";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";
import { get } from "svelte/store";
import { passwordRepeatMatches } from "../src/lib/utils/password-repeat.ts";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
globalThis.window = globalThis;
let persisted;
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    if (command === "set_config") {
      persisted = structuredClone(args.config);
      return structuredClone(persisted);
    }
    if (command === "get_config") return structuredClone(persisted);
    if (command === "remote_secret_present") return { accessKey: false, secretKey: false };
    throw new Error(`Unexpected command: ${command}`);
  },
};
const { appSettings, normalizeSettings, DEFAULT_APP_SETTINGS } =
  await import("../src/lib/services/settings.ts");
const { isWeakKdfParameters } = await import("../src/lib/utils/kdf-parameters.ts");
const { SessionViewGuard } = await import("../src/lib/utils/session-state.ts");

test("repetition preserves exact password bytes and hidden validation", () => {
  for (const onlyWhenHidden of [false, true]) {
    for (const visible of [false, true]) {
      assert.equal(passwordRepeatMatches(" 文字 ", " 文字 ", visible, onlyWhenHidden), true);
      assert.equal(passwordRepeatMatches("", "", visible, onlyWhenHidden), true);
      assert.equal(
        passwordRepeatMatches(" 文字 ", "文字", visible, onlyWhenHidden),
        visible && onlyWhenHidden,
      );
    }
  }
});

test("legacy and malformed config defaults to the official enabled value", () => {
  assert.equal(DEFAULT_APP_SETTINGS.security.repeatPasswordOnlyWhenHidden, true);
  for (const value of [undefined, null, 0, "false", {}]) {
    assert.equal(
      normalizeSettings({ security: { repeatPasswordOnlyWhenHidden: value } }).security
        .repeatPasswordOnlyWhenHidden,
      true,
    );
  }
});

test("repetition preference publishes and survives settings persistence/reload", async () => {
  try {
    for (const enabled of [false, true]) {
      appSettings.updateSecurity("repeatPasswordOnlyWhenHidden", enabled);
      assert.equal(get(appSettings).security.repeatPasswordOnlyWhenHidden, enabled);
      await appSettings.flush();
      assert.equal(persisted.security.repeatPasswordOnlyWhenHidden, enabled);
      await appSettings.initialize();
      assert.equal(get(appSettings).security.repeatPasswordOnlyWhenHidden, enabled);
    }
  } finally {
    appSettings.destroy();
  }
});

const source = readFileSync("src/lib/components/settings/SecuritySettingsPanel.svelte", "utf8");
const handler = source.match(/^  async function submitMasterKeyChange\([^]*?^  \}/m)?.[0];
const continueHandler = source.match(/^  function continueWeakMasterKey\([^]*?^  \}/m)?.[0];
assert.ok(handler, "extract complete production master-key submission");
assert.ok(continueHandler, "extract complete production weak-cost confirmation");

function masterFixture(onlyWhenHidden, showPassword) {
  const calls = [];
  const masterView = new SessionViewGuard();
  masterView.activate("synthetic-origin");
  const context = vm.createContext({
    newPassword: " synthetic-new ",
    confirmPassword: "different",
    newKeyfile: null,
    onlyWhenHidden,
    showPassword,
    busy: false,
    showWeakWarning: false,
    weakWarningSignature: "",
    weakMasterApproval: null,
    weakWarningView: null,
    masterView,
    isWeakKdfParameters,
    keyTransformWeakWarning: true,
    database: { kdfParameters: { algorithm: "Aes", rounds: "600000" } },
    feedback: null,
    lang: "en",
    passwordRepeatMatches,
    appSettings: {},
    get: () => ({
      security: {
        repeatPasswordOnlyWhenHidden: context.onlyWhenHidden,
        keyTransformWeakWarning: context.keyTransformWeakWarning,
      },
    }),
    t: (_lang, key) => key,
    rememberCredential: async () => {},
    vault: {
      getActiveSessionId: () => "synthetic-origin",
      callInSession: async (id, run) => {
        assert.equal(id, "synthetic-origin");
        return run();
      },
      getDatabaseSettings: async () => context.database,
      changeMasterKey: async (...args) => {
        calls.push(args);
        if (context.fail) throw new Error("synthetic rejection");
        return { path: "" };
      },
    },
  });
  vm.runInContext(
    ts.transpileModule(
      `${handler}\n${continueHandler}\nglobalThis.run = submitMasterKeyChange; globalThis.continueWeak = continueWeakMasterKey;`,
      {
        compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None },
      },
    ).outputText,
    context,
  );
  return { context, calls };
}

test("master-key warning preserves credentials on cancel and binds approval to current costs", async () => {
  const f = masterFixture(true, true);
  f.context.database = { kdfParameters: { algorithm: "Aes", rounds: "1" } };
  await f.context.run();
  assert.equal(f.calls.length, 0);
  assert.equal(f.context.showWeakWarning, true);
  assert.equal(f.context.busy, false);
  assert.equal(f.context.newPassword, " synthetic-new ");
  f.context.showWeakWarning = false;
  await f.context.run();
  assert.equal(f.calls.length, 0);
  f.context.weakMasterApproval = f.context.weakWarningSignature;
  f.context.showWeakWarning = false;
  f.context.database = { kdfParameters: { algorithm: "Aes", rounds: "2" } };
  await f.context.run();
  assert.equal(f.calls.length, 0);
  assert.equal(f.context.showWeakWarning, true);
  f.context.weakMasterApproval = f.context.weakWarningSignature;
  f.context.showWeakWarning = false;
  await f.context.run();
  assert.equal(f.calls.length, 1);
  assert.equal(f.context.newPassword, "");
  assert.equal(f.context.weakMasterApproval, null);
});

test("accepting a warning after a tab switch cannot change another database's key", async () => {
  const f = masterFixture(true, true);
  f.context.database = { kdfParameters: { algorithm: "Aes", rounds: "1" } };
  await f.context.run();
  f.context.masterView.activate("synthetic-other");
  f.context.database = { kdfParameters: { algorithm: "Aes", rounds: "600000" } };
  f.context.vault.getActiveSessionId = () => "synthetic-other";
  f.context.continueWeak();
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(f.calls.length, 0);
  assert.equal(f.context.showWeakWarning, false);
  assert.equal(f.context.newPassword, " synthetic-new ");
});

test("weak-warning preference defaults on and persists both explicit choices", async () => {
  assert.equal(DEFAULT_APP_SETTINGS.security.keyTransformWeakWarning, true);
  for (const value of [undefined, null, 0, "false", {}])
    assert.equal(
      normalizeSettings({ security: { keyTransformWeakWarning: value } }).security
        .keyTransformWeakWarning,
      true,
    );
  for (const enabled of [false, true]) {
    appSettings.updateSecurity("keyTransformWeakWarning", enabled);
    assert.equal(get(appSettings).security.keyTransformWeakWarning, enabled);
    await appSettings.flush();
    assert.equal(persisted.security.keyTransformWeakWarning, enabled);
    await appSettings.initialize();
    assert.equal(get(appSettings).security.keyTransformWeakWarning, enabled);
  }
  appSettings.destroy();
});

test("disabling weak warnings preserves the existing master-key flow", async () => {
  const f = masterFixture(true, true);
  f.context.keyTransformWeakWarning = false;
  f.context.vault.getDatabaseSettings = () => assert.fail("disabled preference must not preflight");
  await f.context.run();
  assert.equal(f.calls.length, 1);
});

test("detached and A-B-A master-key preflights never change credentials", async () => {
  for (const detach of [true, false]) {
    const f = masterFixture(true, true);
    let finish;
    f.context.vault.getDatabaseSettings = () => new Promise((resolve) => (finish = resolve));
    const run = f.context.run();
    f.context.masterView.activate(detach ? null : "synthetic-other");
    if (!detach) f.context.masterView.activate("synthetic-origin");
    finish(f.context.database);
    await run;
    assert.equal(f.calls.length, 0);
    assert.equal(f.context.newPassword, " synthetic-new ");
  }
});

test("master-key change enforces current visibility and preference before IPC", async () => {
  for (const onlyWhenHidden of [false, true]) {
    for (const showPassword of [false, true]) {
      const f = masterFixture(onlyWhenHidden, showPassword);
      await f.context.run();
      const allowed = onlyWhenHidden && showPassword;
      assert.equal(f.calls.length, allowed ? 1 : 0);
      if (allowed) {
        assert.equal(f.calls[0][0], " synthetic-new ");
        assert.equal(f.context.newPassword, "");
        assert.equal(f.context.confirmPassword, "");
        assert.equal(f.context.showPassword, false);
      } else assert.equal(f.context.feedback.message, "security.mismatch");
    }
  }
});

test("matching and keyfile-only master changes remain available", async () => {
  for (const keyfileOnly of [false, true]) {
    const f = masterFixture(false, false);
    f.context.newPassword = keyfileOnly ? "" : "synthetic-control";
    f.context.confirmPassword = f.context.newPassword;
    f.context.newKeyfile = keyfileOnly ? "synthetic.key" : null;
    await f.context.run();
    assert.equal(f.calls.length, 1);
    assert.equal(f.calls[0][1], keyfileOnly ? "synthetic.key" : null);
  }
});

test("a failed visible master change retains a retryable draft", async () => {
  const f = masterFixture(true, true);
  f.context.fail = true;
  await f.context.run();
  assert.equal(f.context.newPassword, " synthetic-new ");
  assert.equal(f.context.showPassword, true);
  assert.equal(f.context.busy, false);
  assert.equal(f.context.feedback.message, "Error: synthetic rejection");
});

test("busy master-key change cannot be submitted twice", async () => {
  const f = masterFixture(true, true);
  f.context.busy = true;
  await f.context.run();
  assert.equal(f.calls.length, 0);
});
