import assert from "node:assert/strict";
import { register } from "node:module";
import { after, test } from "node:test";
import { get } from "svelte/store";
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
test("global key-expiry strings retain disabled defaults and survive actual settings persistence", async () => {
  for (const key of ["masterKeyExpiryRec", "masterKeyExpiryForce"]) {
    assert.equal(DEFAULT_APP_SETTINGS.security[key], "");
    for (const invalid of [undefined, null, 30, {}, true]) {
      assert.equal(normalizeSettings({ security: { [key]: invalid } }).security[key], "");
    }
    for (const value of [" P30D ", "2026-01-01T00:00:00Z", "invalid", ""]) {
      appSettings.updateSecurity(key, value);
      await appSettings.flush();
      assert.equal(persisted.security[key], value);
      await appSettings.initialize();
      assert.equal(get(appSettings).security[key], value);
    }
  }
});
