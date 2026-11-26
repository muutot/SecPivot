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
