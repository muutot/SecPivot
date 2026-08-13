import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

import { validateReleaseIdentity } from "../scripts/validate-release-version.mjs";

const repositoryVersion = JSON.parse(
  readFileSync(new URL("../package.json", import.meta.url)),
).version;

test("release identity accepts the repository version tag", () => {
  assert.deepEqual(validateReleaseIdentity(`v${repositoryVersion}`), {
    releaseTag: `v${repositoryVersion}`,
    version: repositoryVersion,
  });
});

test("release identity rejects a manual or pushed tag for another version", () => {
  assert.throws(
    () => validateReleaseIdentity("v9.9.9"),
    new RegExp(`does not match repository version v${repositoryVersion.replaceAll(".", "\\.")}`),
  );
});
