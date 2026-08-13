import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, unlinkSync } from "node:fs";
import { fileURLToPath } from "node:url";

const changelogScript = readFileSync(new URL("../scripts/changelog.mjs", import.meta.url), "utf-8");
const versionScript = readFileSync(new URL("../scripts/version.mjs", import.meta.url), "utf-8");
const changelogPath = fileURLToPath(new URL("../scripts/changelog.mjs", import.meta.url));
const injectionMarker = fileURLToPath(
  new URL("../.changelog-shell-injection-marker", import.meta.url),
);

test("changelog and version subprocesses avoid shell interpolation", () => {
  assert.doesNotMatch(changelogScript, /execSync|shell:\s*true/);
  assert.doesNotMatch(versionScript, /execSync|shell:\s*true/);
  assert.match(changelogScript, /"--end-of-options", \.\.\.revisionArgs/);
  assert.match(changelogScript, /execFileSync\(process\.execPath/);
  assert.match(versionScript, /execFileSync\(process\.execPath/);
});

test("a shell-like changelog revision remains one literal git argument", () => {
  const payload = `HEAD && "${process.execPath}" -e "require('node:fs').writeFileSync('.changelog-shell-injection-marker','injected')" && echo `;

  try {
    execFileSync(process.execPath, [changelogPath, "--preview", "--from", payload], {
      encoding: "utf-8",
      stdio: "pipe",
    });
    assert.equal(existsSync(injectionMarker), false);
  } finally {
    if (existsSync(injectionMarker)) unlinkSync(injectionMarker);
  }
});
