import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { register } from "node:module";
import { test, after } from "node:test";
import { get } from "svelte/store";
import vm from "node:vm";
import ts from "typescript";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
const { ActivityLeaseGuard } = await import("../src/lib/utils/session-state.ts");
globalThis.window = globalThis;
globalThis.localStorage = { getItem: () => null, setItem: () => {}, removeItem: () => {} };
let persisted, failClose, pendingEdit, nativeCloseGate;
const sessions = new Map();
const calls = [];
let active = null,
  next = 0;
function snapshot(id) {
  return {
    path: `C:/synthetic/${id}.kdbx`,
    fileName: `${id}.kdbx`,
    revision: 1,
    statusRevision: 1,
    dirty: true,
    readOnly: false,
    root: { uuid: "root", name: "Synthetic", entries: [], children: [] },
    customIcons: {},
  };
}
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args = {}) => {
    calls.push({ command, args });
    switch (command) {
      case "set_config":
        persisted = structuredClone(args.config);
        return structuredClone(persisted);
      case "open_vault": {
        active = `synthetic-${++next}`;
        const state = snapshot(active);
        sessions.set(active, state);
        return { sessionId: active, state: structuredClone(state) };
      }
      case "list_sessions":
        return [...sessions]
          .sort(([a], [b]) => Number(b === active) - Number(a === active))
          .map(([sessionId, state]) => ({
            sessionId,
            path: state.path,
            fileName: state.fileName,
            dirty: state.dirty,
          }));
      case "get_vault_state":
        return structuredClone(sessions.get(args.sessionId ?? active) ?? null);
      case "set_active_session":
        active = args.sessionId;
        return structuredClone(sessions.get(active));
      case "add_entry":
        return new Promise((resolve) => {
          pendingEdit = resolve;
        });
      case "open_attachment_temp":
        return {
          token: "synthetic-temp",
          sessionId: args.sessionId,
          path: "C:/synthetic/temp.txt",
          name: "synthetic.txt",
        };
      case "cleanup_attachment_temp":
      case "clear_saved_credential":
        return;
      case "close_vault":
      case "close_all_vaults": {
        if (nativeCloseGate) await nativeCloseGate;
        assert.equal(
          persisted.security.autoSaveOnClose,
          get(appSettings).security.autoSaveOnClose,
          "current preference reaches native config before close admission",
        );
        if (failClose) {
          if (failClose === "partial") {
            const state = sessions.get(active);
            state.dirty = false;
            state.readOnly = true;
            state.statusRevision++;
            throw new Error("synthetic later save failure");
          }
          throw failClose;
        }
        if (command === "close_all_vaults") {
          sessions.clear();
          active = null;
        } else {
          sessions.delete(args.sessionId);
          if (active === args.sessionId) active = [...sessions.keys()].at(-1) ?? null;
        }
        return;
      }
      default:
        throw new Error(`Unexpected synthetic command: ${command}`);
    }
  },
};
const { appSettings, DEFAULT_APP_SETTINGS, normalizeSettings } =
  await import("../src/lib/services/settings.ts");
const { vault, isVaultClosePending } = await import("../src/lib/services/vault.ts");
const { lockVaultAutomatically } = await import("../src/lib/services/security.ts");
const { closeErrorKind, reportCloseError } = await import("../src/lib/services/vault-close.ts");
const { tips, dismissTip } = await import("../src/lib/services/tips.ts");
after(() => {
  appSettings.destroy();
  for (const tip of get(tips)) dismissTip(tip.id);
});

async function fixture() {
  nativeCloseGate = undefined;
  failClose = undefined;
  await vault.closeAll();
  calls.length = 0;
  appSettings.updateSecurity("autoSaveOnClose", true);
  appSettings.updateSecurity("clearOnLock", false);
  appSettings.updateSecurity("minimizeAfterLocking", false);
  appSettings.updateSecurity("autoLockMinutes", 0);
  await vault.open("synthetic.kdbx", "synthetic-master");
  return active;
}

test("auto-save-close uses official default and normalizes/persists both values", async () => {
  assert.equal(DEFAULT_APP_SETTINGS.security.autoSaveOnClose, false);
  for (const invalid of [null, undefined, "true", 1, {}]) {
    assert.equal(
      normalizeSettings({ security: { autoSaveOnClose: invalid } }).security.autoSaveOnClose,
      false,
    );
  }
  for (const enabled of [true, false]) {
    appSettings.updateSecurity("autoSaveOnClose", enabled);
    await appSettings.flush();
    assert.equal(persisted.security.autoSaveOnClose, enabled);
    assert.equal(normalizeSettings(structuredClone(persisted)).security.autoSaveOnClose, enabled);
  }
});

for (const method of ["close", "closeTab", "closeAll"]) {
  test(`${method} cancellation retains tabs, pending-response ownership and attachment temps`, async () => {
    const id = await fixture();
    await vault.openAttachmentTemp("synthetic-entry", "synthetic.txt");
    const edit = vault.addEntry({
      groupUuid: "root",
      title: "Synthetic",
      username: "",
      password: "",
      url: "",
      notes: "",
    });
    failClose = "VAULT_CLOSE_CANCELLED:";
    calls.length = 0;
    await assert.rejects(
      vault[method](...(method === "closeTab" ? [id] : [])),
      (error) => closeErrorKind(error) === "cancelled",
    );
    assert.equal(get(vault.activeId), id);
    assert.equal(get(vault.tabs).length, 1);
    assert.equal(vault.get().dirty, true);
    assert.equal(
      calls.some(
        ({ command }) =>
          command === "cleanup_attachment_temp" || command === "clear_saved_credential",
      ),
      false,
    );
    const updated = snapshot(id);
    updated.revision = 2;
    pendingEdit(updated);
    assert.equal((await edit).revision, 2);
    assert.equal(vault.get().revision, 2);
    failClose = undefined;
    await vault.closeAll();
    assert.equal(
      calls.some(({ command }) => command === "cleanup_attachment_temp"),
      true,
    );
  });
}

test("partial save failure republishes native status without removing sessions", async () => {
  const id = await fixture();
  await vault.open("second.kdbx", "synthetic-master");
  failClose = "partial";
  await assert.rejects(vault.closeAll(), /synthetic later save failure/);
  assert.equal(get(vault.tabs).length, 2);
  assert.equal(vault.get().dirty, false);
  assert.equal(vault.get().readOnly, true);
  assert.equal(vault.get().statusRevision, 2);
  assert.ok(sessions.has(id));
  failClose = undefined;
});

test("close pending guard spans native wait and resets after cancellation", async () => {
  await fixture();
  let release;
  nativeCloseGate = new Promise((resolve) => {
    release = resolve;
  });
  failClose = "VAULT_CLOSE_CANCELLED:";
  const outcome = Promise.allSettled([vault.closeAll()]);
  for (let i = 0; i < 10 && !isVaultClosePending(); i++) await new Promise(setImmediate);
  assert.equal(isVaultClosePending(), true);
  release();
  assert.equal((await outcome)[0].status, "rejected");
  assert.equal(isVaultClosePending(), false);
  nativeCloseGate = undefined;
  failClose = undefined;
});

test("automatic lock catches cancellation and concurrent-close errors without success or error toast", async () => {
  await fixture();
  for (const marker of ["VAULT_CLOSE_CANCELLED:", "VAULT_CLOSE_BUSY:"]) {
    failClose = marker;
    await lockVaultAutomatically();
    assert.equal(get(tips).length, 0);
    assert.ok(vault.get());
  }
  failClose = new Error("synthetic save failure");
  await lockVaultAutomatically();
  assert.equal(get(tips).at(-1).kind, "error");
  assert.ok(vault.get());
  for (const tip of get(tips)) dismissTip(tip.id);
  failClose = undefined;
});

test("close errors accept native string and Error envelopes and localize changes", () => {
  assert.equal(closeErrorKind(new Error("VAULT_CLOSE_CANCELLED:")), "cancelled");
  assert.equal(closeErrorKind("VAULT_CLOSE_CHANGED:"), "changed");
  reportCloseError("VAULT_CLOSE_CANCELLED:");
  assert.equal(get(tips).length, 0);
  reportCloseError("VAULT_CLOSE_CHANGED:");
  assert.match(get(tips).at(-1).message, /变化/);
  for (const tip of get(tips)) dismissTip(tip.id);
});

test("manual native lock handler has one prompt owner and only announces successful locking", async () => {
  const source = readFileSync("src/routes/+page.svelte", "utf8");
  const handler = source.match(/^  async function handleLock\([^]*?^  \}/m)?.[0];
  assert.ok(handler);
  const context = vm.createContext({
    currentVault: { dirty: true },
    isTauriRuntime: () => true,
    settings: { general: { language: "en" } },
    window: {
      confirm: () => {
        throw new Error("native prompt is sole owner");
      },
    },
    t: (_, key) => key,
    messages: [],
    failures: [],
    flash: (message) => context.messages.push(message),
    reportCloseError: (error) => context.failures.push(error),
    lockVault: async () => {
      if (context.failure) throw context.failure;
    },
  });
  vm.runInContext(
    ts.transpileModule(handler + "\nglobalThis.run = handleLock;", {
      compilerOptions: { target: ts.ScriptTarget.ES2022 },
    }).outputText,
    context,
  );
  context.failure = "VAULT_CLOSE_CANCELLED:";
  await context.run();
  assert.equal(context.messages.length, 0);
  context.failure = new Error("save failure");
  await context.run();
  assert.equal(context.messages.length, 0);
  context.failure = undefined;
  await context.run();
  assert.deepEqual([...context.messages], ["page.locked"]);
});

test("native close dialogs suppress focus lock and pending listener cleanup remains owned", async () => {
  const source = readFileSync("src/lib/services/security.ts", "utf8");
  const ast = ts.createSourceFile("security.ts", source, ts.ScriptTarget.Latest, true);
  const handler = ast.statements.find(
    (node) => ts.isFunctionDeclaration(node) && node.name?.text === "installFocusLock",
  );
  assert.ok(handler);
  const callbacks = new Map(),
    listeners = new Map();
  let locks = 0,
    removed = 0;
  const context = vm.createContext({
    nativeCloseDialogActivity: new ActivityLeaseGuard(),
    isVaultClosePending: () => context.pending,
    tcatoOverlayActivity: new ActivityLeaseGuard(),
    screenCaptureActivity: new ActivityLeaseGuard(),
    get: () => ({ security: { lockOnFocusLoss: true } }),
    appSettings: {},
    vault: { get: () => ({}) },
    isTauriRuntime: () => true,
    lockVaultAutomatically: async () => {
      locks++;
    },
    setTcatoOverlayOpen: () => {},
    window: {
      addEventListener: (name, callback) => listeners.set(name, callback),
      removeEventListener: (name) => listeners.delete(name),
    },
    listen: (name, callback) => {
      callbacks.set(name, callback);
      return Promise.resolve(() => removed++);
    },
  });
  vm.runInContext(
    ts.transpileModule(handler.getText(ast).replace(/^export /, ""), {
      compilerOptions: { target: ts.ScriptTarget.ES2022 },
    }).outputText,
    context,
  );
  const stop = context.installFocusLock();
  context.pending = true;
  listeners.get("blur")();
  assert.equal(locks, 0);
  context.pending = false;
  callbacks.get("vault-close-dialog")({ payload: true });
  listeners.get("blur")();
  assert.equal(locks, 0);
  callbacks.get("vault-close-dialog")({ payload: false });
  listeners.get("blur")();
  assert.equal(locks, 1);
  stop();
  await Promise.resolve();
  assert.equal(removed, 3);
  assert.equal(listeners.size, 0);
});
