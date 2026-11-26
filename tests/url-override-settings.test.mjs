import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";
import { get } from "svelte/store";

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

test("legacy integration config loads an empty global template without changing rules", () => {
  const integration = {
    urlOverrideEnabled: false,
    builtInOverridesEnabled: 8,
    customOverrides: [{ enabled: true, scheme: "ssh", urlOverride: "cmd://client {BASE}" }],
  };
  assert.deepEqual(normalizeSettings({ integration }, DEFAULT_APP_SETTINGS).integration, {
    ...integration,
    urlOverride: "",
  });
  assert.equal(
    normalizeSettings({ integration: { urlOverride: 42 } }, DEFAULT_APP_SETTINGS).integration
      .urlOverride,
    "",
  );
});

test("global template publishes immediately and survives disabled persistence/reload exactly", async () => {
  const template = '  cmd://"C:\\Program Files\\client.exe"  "{BASE}"  "{URL}"  ';
  const seen = [];
  const unsubscribe = appSettings.subscribe((value) => seen.push(value.integration));
  try {
    appSettings.updateIntegration("urlOverride", template);
    appSettings.updateIntegration("urlOverrideEnabled", false);
    assert.equal(seen.at(-1).urlOverride, template);
    await appSettings.flush();
    assert.equal(persisted.integration.urlOverride, template);
    assert.equal(persisted.integration.urlOverrideEnabled, false);
    await appSettings.initialize();
    assert.equal(get(appSettings).integration.urlOverride, template);
    assert.equal(get(appSettings).integration.urlOverrideEnabled, false);
    appSettings.updateIntegration("urlOverrideEnabled", true);
    assert.equal(get(appSettings).integration.urlOverride, template);
    appSettings.updateIntegration("urlOverride", "");
    await appSettings.flush();
    await appSettings.initialize();
    assert.equal(get(appSettings).integration.urlOverride, "");
  } finally {
    unsubscribe();
    appSettings.destroy();
  }
});
