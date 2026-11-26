import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.window = globalThis;
let sequence = 0;
const sessions = new Map();
const pending = [];
let active;
function snapshot(id) {
  return {
    path: `C:/synthetic/${id}.kdbx`,
    fileName: `${id}.kdbx`,
    revision: 1,
    dirty: false,
    readOnly: false,
    root: { uuid: "root", name: "", children: [], entries: [] },
    customIcons: {},
  };
}
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args = {}) => {
    switch (command) {
      case "open_vault":
        active = `session-${++sequence}`;
        sessions.set(active, snapshot(active));
        return { sessionId: active, state: sessions.get(active) };
      case "list_sessions":
        return [...sessions.keys()].map((id) => ({
          sessionId: id,
          path: sessions.get(id).path,
          fileName: sessions.get(id).fileName,
          dirty: false,
        }));
      case "set_active_session":
        active = args.sessionId;
        return sessions.get(active);
      case "benchmark_database_kdf":
      case "get_database_settings":
      case "get_kdf_upgrade_offer":
      case "get_master_key_expiry_offer":
      case "upgrade_weak_kdf":
        return new Promise((resolve) => pending.push({ args, resolve }));
      case "close_vault":
        sessions.delete(args.sessionId);
        return;
      case "close_all_vaults":
        sessions.clear();
        return;
      case "set_config":
        return args.config;
      case "clear_saved_credential":
        return;
      default:
        throw new Error(`Unexpected synthetic command: ${command}`);
    }
  },
};
const { vault } = await import("../src/lib/services/vault.ts");
const parameters = { algorithm: "Aes", rounds: "9007199254740993" };

test("expiry assessment IPC stays read-only and bound to the captured origin", async () => {
  await vault.open("C:/synthetic/a.kdbx", "synthetic-only");
  const a = vault.getActiveSessionId();
  const run = vault.getMasterKeyExpiryOffer();
  const request = pending.shift();
  assert.deepEqual(request.args, { sessionId: a });
  await vault.open("C:/synthetic/b.kdbx", "synthetic-only");
  const b = vault.getActiveSessionId();
  const offer = {
    token: "synthetic-expiry-lifetime",
    fileName: "a.kdbx",
    changedAt: null,
    recommended: false,
    required: true,
    forceOnce: true,
    readOnly: false,
    invalidRules: [],
  };
  request.resolve(offer);
  assert.deepEqual(await run, offer);
  assert.equal(vault.getActiveSessionId(), b);
  assert.equal(vault.get().dirty, false);
  const disposed = vault.getMasterKeyExpiryOffer();
  const late = pending.shift();
  const rejection = assert.rejects(disposed);
  await vault.closeAll();
  late.resolve(offer);
  await rejection;
  assert.equal(vault.get(), null);
});
const result = {
  parameters: { algorithm: "Aes", rounds: "1234567" },
  elapsedMs: 1000,
  targetMs: 1000,
  samples: 3,
  targetReached: true,
};

test("benchmark IPC carries only exact parameters, target and originating session across tab switches", async () => {
  await vault.open("C:/synthetic/a.kdbx", "synthetic-only");
  const a = vault.getActiveSessionId();
  const run = vault.benchmarkDatabaseKdf(parameters, 1000);
  const request = pending.shift();
  assert.deepEqual(request.args, { sessionId: a, parameters, targetMs: 1000 });
  await vault.open("C:/synthetic/b.kdbx", "synthetic-only");
  const b = vault.getActiveSessionId();
  request.resolve(result);
  assert.deepEqual(await run, result);
  assert.equal(vault.getActiveSessionId(), b);
  assert.equal(vault.get().dirty, false);
  await vault.closeAll();
});

test("database-settings reads remain bound to the original session lifetime", async () => {
  await vault.open("C:/synthetic/a.kdbx", "synthetic-only");
  const a = vault.getActiveSessionId();
  const run = vault.getDatabaseSettings();
  const request = pending.shift();
  assert.deepEqual(request.args, { sessionId: a });
  await vault.closeAll();
  const rejection = assert.rejects(run);
  request.resolve({ kdfParameters: parameters });
  await rejection;
  assert.equal(vault.get(), null);
});

test("closed-origin benchmark replies are rejected instead of republishing a disposed result", async () => {
  await vault.open("C:/synthetic/a.kdbx", "synthetic-only");
  const run = vault.benchmarkDatabaseKdf(parameters, 1000);
  const rejection = assert.rejects(run);
  const request = pending.shift();
  await vault.closeAll();
  request.resolve(result);
  await rejection;
  assert.equal(vault.get(), null);
});

test("offer and upgrade IPC contain only the explicit origin, native token and exact reviewed costs", async () => {
  await vault.open("C:/synthetic/a.kdbx", "synthetic-only");
  const a = vault.getActiveSessionId();
  const epoch = vault.getSessionEpoch(a);
  assert.equal(typeof epoch, "symbol");
  const read = vault.getKdfUpgradeOffer();
  const request = pending.shift();
  assert.deepEqual(request.args, { sessionId: a });
  const offer = {
    token: "synthetic-lifetime",
    fileName: "synthetic.kdbx",
    parameters: { algorithm: "Aes", rounds: "1" },
    recommended: { algorithm: "Aes", rounds: "600000" },
    readOnly: false,
  };
  request.resolve(offer);
  assert.deepEqual(await read, offer);
  const run = vault.upgradeWeakKdf(offer);
  const upgrade = pending.shift();
  assert.deepEqual(upgrade.args, { sessionId: a, token: offer.token, expected: offer.parameters });
  await vault.open("C:/synthetic/b.kdbx", "synthetic-only");
  const b = vault.getActiveSessionId();
  upgrade.resolve({ ...snapshot(a), dirty: true, revision: 2 });
  assert.equal((await run).dirty, true);
  assert.equal(vault.getActiveSessionId(), b);
  assert.equal(vault.get().dirty, false);
  await vault.closeAll();
  assert.equal(vault.getSessionEpoch(a), undefined);
});

test("closed-origin offer and mutation replies cannot resurrect a vault", async () => {
  for (const mutation of [false, true]) {
    await vault.open("C:/synthetic/a.kdbx", "synthetic-only");
    const a = vault.getActiveSessionId();
    const run = mutation
      ? vault.upgradeWeakKdf({ token: "synthetic", parameters: { algorithm: "Aes", rounds: "1" } })
      : vault.getKdfUpgradeOffer();
    const rejection = assert.rejects(run);
    const request = pending.shift();
    await vault.closeAll();
    request.resolve(
      mutation
        ? { ...snapshot(a), dirty: true, revision: 2 }
        : { token: "synthetic", recommended: null },
    );
    await rejection;
    assert.equal(vault.get(), null);
  }
});
