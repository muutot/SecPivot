import { register } from "node:module";
import test from "node:test";
import assert from "node:assert/strict";
import { get } from "svelte/store";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
globalThis.window = globalThis;
let submitted;
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    assert.equal(command, "set_config");
    submitted = structuredClone(args.config);
    const masked = structuredClone(args.config);
    for (const profile of masked.remoteProfiles) {
      profile.settings.accessKey = "";
      profile.settings.secretKey = "";
    }
    return masked;
  },
};
const { appSettings, normalizeRemoteSettings } = await import("../src/lib/services/settings.ts");
const samples = ["", "fixture", " fixture", "fixture ", " fixture ", "   ", "\u00a0fixture\u00a0"];

test("remote secret normalization preserves content while trimming endpoint text", () => {
  for (const kind of ["s3", "webdav"]) {
    for (const sample of samples) {
      const normalized = normalizeRemoteSettings(
        { secretKey: sample, endpoint: "  https://synthetic.invalid  " },
        kind,
      );
      assert.ok(normalized.secretKey === sample, "secret value changed during normalization");
      assert.equal(normalized.endpoint, "https://synthetic.invalid");
    }
    for (const secretKey of [undefined, null, 42]) {
      assert.equal(normalizeRemoteSettings({ secretKey }, kind).secretKey, "");
    }
  }
});

test("queued native settings keep the exact secret and accept masked acknowledgements", async () => {
  try {
    for (const kind of ["s3", "webdav"]) {
      const path = `${kind}/config_1`;
      for (const sample of samples) {
        appSettings.updateRemote(path, "secretKey", sample);
        await appSettings.flush();
        const profile = submitted.remoteProfiles.find((p) => p.settings.kind === kind);
        assert.ok(profile.settings.secretKey === sample, "queued settings changed secret content");
        const current = get(appSettings).remoteProfiles.find((p) => p.settings.kind === kind);
        assert.equal(current.settings.secretKey, "", "stored secret returned to renderer");
      }
    }
  } finally {
    appSettings.destroy();
  }
});
