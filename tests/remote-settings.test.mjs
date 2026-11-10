import { register } from "node:module";
import test from "node:test";
import assert from "node:assert/strict";
import { get } from "svelte/store";

register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
globalThis.window = globalThis;
let submitted;
let writeCount = 0;
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    assert.equal(command, "set_config");
    writeCount += 1;
    submitted = structuredClone(args.config);
    const masked = structuredClone(args.config);
    for (const profile of masked.remoteProfiles) {
      profile.settings.accessKey = "";
      profile.settings.secretKey = "";
    }
    return masked;
  },
};
const { appSettings, normalizeRemoteSettings, remoteSecretPresence } =
  await import("../src/lib/services/settings.ts");
const { tips, dismissTip } = await import("../src/lib/services/tips.ts");
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

test("rejected and unchanged remote renames retain hints without warning or persistence", async () => {
  try {
    for (const kind of ["s3", "webdav"]) {
      appSettings.addRemoteProfile(kind, "collision");
      await appSettings.flush();
      const path = `${kind}/config_1`;
      appSettings.updateRemote(path, "secretKey", "synthetic-fixture");
      await appSettings.flush();
      for (const name of ["config_1", " config_1 ", "", "   ", "collision"]) {
        for (const tip of get(tips)) dismissTip(tip.id);
        const before = get(appSettings);
        const presenceBefore = get(remoteSecretPresence);
        const writesBefore = writeCount;
        appSettings.renameRemoteProfile(path, name);
        await appSettings.flush();
        assert.equal(get(appSettings), before, "no-op changed settings identity");
        assert.equal(get(remoteSecretPresence), presenceBefore, "no-op changed credential hints");
        assert.equal(get(tips).length, 0, "no-op displayed rename warning");
        assert.equal(writeCount, writesBefore, "no-op persisted settings");
      }
      const before = get(appSettings);
      const writesBefore = writeCount;
      appSettings.renameRemoteProfile(`${kind}/missing`, "valid");
      await appSettings.flush();
      assert.equal(get(appSettings), before);
      assert.equal(writeCount, writesBefore);
      appSettings.renameRemoteProfile(path, "renamed");
      await appSettings.flush();
      assert.ok(
        get(appSettings).remoteProfiles.some(
          (p) => p.settings.kind === kind && p.name === "renamed",
        ),
      );
      assert.equal(get(remoteSecretPresence)[path], undefined);
      assert.equal(get(remoteSecretPresence)[`${kind}/renamed`]?.secretKey ?? false, false);
      assert.equal(get(tips).filter((tip) => tip.kind === "error").length, 1);
      assert.equal(writeCount, writesBefore + 1);
      appSettings.renameRemoteProfile(`${kind}/renamed`, "config_1");
      await appSettings.flush();
      for (const tip of get(tips)) dismissTip(tip.id);
    }
  } finally {
    appSettings.destroy();
    for (const tip of get(tips)) dismissTip(tip.id);
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

test("rejected remote removals retain the final profile and credential hints without persistence", async () => {
  try {
    for (const kind of ["s3", "webdav"]) {
      const path = `${kind}/config_1`;
      const extras = get(appSettings).remoteProfiles.filter(
        (profile) => profile.settings.kind === kind && profile.name !== "config_1",
      );
      for (const profile of extras) {
        appSettings.removeRemoteProfile(`${kind}/${profile.name}`);
        await appSettings.flush();
      }
      appSettings.updateRemote(path, "secretKey", "synthetic-removal-fixture");
      await appSettings.flush();
      assert.equal(get(remoteSecretPresence)[path].secretKey, true);
      for (const rejected of [path, `${kind}/missing`]) {
        const before = get(appSettings);
        const presenceBefore = get(remoteSecretPresence);
        const writesBefore = writeCount;
        appSettings.removeRemoteProfile(rejected);
        await appSettings.flush();
        assert.ok(
          get(remoteSecretPresence) === presenceBefore,
          "rejected removal changed credential hints",
        );
        assert.ok(get(appSettings) === before, "rejected removal changed settings identity");
        assert.equal(writeCount, writesBefore, "rejected removal persisted settings");
      }
    }
  } finally {
    appSettings.destroy();
  }
});

test("accepted remote removal selects a same-protocol replacement and preserves other hints", async () => {
  try {
    for (const kind of ["s3", "webdav"]) {
      const path = `${kind}/removable`;
      appSettings.addRemoteProfile(kind, "removable");
      await appSettings.flush();
      appSettings.updateRemote(path, "secretKey", "synthetic-deleted-fixture");
      await appSettings.flush();
      const before = get(appSettings);
      const presenceBefore = get(remoteSecretPresence);
      const writesBefore = writeCount;
      assert.equal(before.activeRemote, path);
      appSettings.removeRemoteProfile(path);
      await appSettings.flush();
      const after = get(appSettings);
      assert.deepEqual(
        after.remoteProfiles,
        before.remoteProfiles.filter(
          (profile) => !(profile.settings.kind === kind && profile.name === "removable"),
        ),
      );
      assert.equal(after.activeRemote, `${kind}/config_1`);
      assert.equal(get(remoteSecretPresence)[path], undefined);
      for (const [otherPath, presence] of Object.entries(presenceBefore)) {
        if (otherPath !== path) assert.deepEqual(get(remoteSecretPresence)[otherPath], presence);
      }
      assert.equal(writeCount, writesBefore + 1);
      const afterBeforeRetry = get(appSettings);
      const presenceBeforeRetry = get(remoteSecretPresence);
      appSettings.removeRemoteProfile(path);
      await appSettings.flush();
      assert.equal(get(appSettings), afterBeforeRetry);
      assert.equal(get(remoteSecretPresence), presenceBeforeRetry);
      assert.equal(writeCount, writesBefore + 1, "duplicate removal persisted settings");
    }
  } finally {
    appSettings.destroy();
  }
});
