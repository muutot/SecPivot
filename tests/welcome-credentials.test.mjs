import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";

// Execute actual form handlers at the service boundary without native dialogs,
// real vaults, clipboard or remote systems. KDBX authentication has separate Rust tests.
const source = await readFile(
  new URL("../src/lib/components/VaultWelcome.svelte", import.meta.url),
  "utf8",
);
const cases = [
  ["confirmOpen", "open"],
  ["confirmCreate", "create"],
  ["confirmRemoteOpen", "openRemote"],
  ["confirmRemoteCreate", "createRemote"],
];

function fixture(name, command, password = "", keyfilePath = "synthetic.key") {
  const match = source.match(new RegExp(`^  async function ${name}\\([^]*?^  \\}`, "m"));
  assert.ok(match, `production ${name} must be extracted`);
  const calls = [];
  const remembered = [];
  let opened = 0;
  const context = vm.createContext({
    path: "synthetic.kdbx",
    remoteKey: "synthetic.kdbx",
    remoteMode: "memory",
    password,
    confirm: password,
    keyfilePath,
    isDemo: false,
    pendingName: null,
    pendingPersistable: undefined,
    error: "",
    busy: false,
    modal: "open",
    lang: "en",
    t: (_lang, key) => key,
    appSettings: {},
    get: () => ({ database: { kdf: "Aes", cipher: "Aes256", compression: "None" } }),
    rememberCredential: async () => remembered.push(true),
    onopened: () => opened++,
    vault: {
      [command]: async (...args) => {
        calls.push(args);
        if (context.backendFailure) throw new Error("synthetic open failure");
      },
    },
  });
  vm.runInContext(
    ts.transpileModule(`${match[0]}\nglobalThis.run = ${name};`, {
      compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.None },
    }).outputText,
    context,
  );
  return { context, calls, remembered, opened: () => opened };
}

for (const [name, command] of cases) {
  test(`${name} submits a selected keyfile with no password`, async () => {
    const f = fixture(name, command);
    await f.context.run();
    assert.equal(f.calls.length, 1, "keyfile-only request must reach the vault service");
    const args = f.calls[0];
    if (command === "create") {
      assert.equal(args[0].password, "");
      assert.equal(args[0].keyfile, "synthetic.key");
    } else {
      assert.equal(args[1], "");
      assert.equal(args[command === "createRemote" ? 5 : 2], "synthetic.key");
    }
    assert.equal(f.remembered.length, 0, "keyfile-only requests must not remember a password");
    assert.equal(f.context.error, "");
    assert.equal(f.context.busy, false);
    assert.equal(f.opened(), 1);
  });

  test(`${name} rejects a request without either key component`, async () => {
    const f = fixture(name, command, "", "");
    await f.context.run();
    assert.equal(f.calls.length, 0);
    assert.equal(f.context.error, "welcome.needPassword");
    assert.equal(f.opened(), 0);
    assert.equal(f.context.busy, false);
  });

  test(`${name} preserves password-only submission`, async () => {
    const f = fixture(name, command, "synthetic-control", "");
    await f.context.run();
    assert.equal(f.calls.length, 1);
    assert.equal(f.context.error, "");
    assert.equal(f.opened(), 1);
    assert.equal(f.remembered.length, command === "open" || command === "create" ? 1 : 0);
  });

  test(`${name} keeps a rejected keyfile request in its form`, async () => {
    const f = fixture(name, command);
    f.context.backendFailure = true;
    await f.context.run();
    assert.equal(f.calls.length, 1);
    assert.equal(f.opened(), 0);
    assert.equal(f.context.modal, "open");
    assert.equal(f.context.keyfilePath, "synthetic.key");
    assert.equal(f.context.busy, false);
    assert.equal(f.context.error, "Error: synthetic open failure");
  });
}

test("local creation still rejects mismatched confirmation with a keyfile", async () => {
  const f = fixture("confirmCreate", "create", "synthetic-control");
  f.context.confirm = "synthetic-mismatch";
  await f.context.run();
  assert.equal(f.calls.length, 0);
  assert.equal(f.context.error, "welcome.passwordMismatch");
  assert.equal(f.opened(), 0);
});

test("local opening still requires a selected database", async () => {
  const f = fixture("confirmOpen", "open");
  f.context.path = "";
  await f.context.run();
  assert.equal(f.calls.length, 0);
  assert.equal(f.context.error, "welcome.pickDbFile");
});
