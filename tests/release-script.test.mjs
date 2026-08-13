import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { bumpVersion, parseSemver, resolveReleaseTarget } from "../scripts/versioning.mjs";
import { RELEASE_FILES, findUnexpectedReleaseChanges } from "../scripts/release-files.mjs";

const releaseScript = readFileSync(new URL("../scripts/release.mjs", import.meta.url), "utf-8");

test("normal releases never infer a force push from ahead/behind counts", () => {
  assert.doesNotMatch(releaseScript, /rev-list --count --left-right/);
  assert.match(releaseScript, /\["push", "--force-with-lease", "origin", BRANCH\]/);
  assert.match(releaseScript, /\["push", "--force", "origin", tagVersion\]/);
});

test("semantic releases reuse the same uncommitted two-pass target", () => {
  assert.equal(resolveReleaseTarget("1.2.0", "1.2.0", "patch"), "1.2.1");
  assert.equal(resolveReleaseTarget("1.2.1", "1.2.0", "patch"), "1.2.1");
  assert.equal(resolveReleaseTarget("1.2.0", "1.2.0", "minor"), "1.3.0");
  assert.equal(resolveReleaseTarget("1.3.0", "1.2.0", "minor"), "1.3.0");
});

test("semantic releases reject an unrelated working version", () => {
  assert.throws(
    () => resolveReleaseTarget("1.2.2", "1.2.0", "patch"),
    /neither HEAD 1\.2\.0 nor expected target 1\.2\.1/,
  );
});

test("explicit versions use strict semver before release git operations", () => {
  assert.equal(bumpVersion("1.2.0", "v2.0.0-beta.1"), "2.0.0-beta.1");
  for (const invalid of ["1.2", "01.2.3", "1.2.3-01", "1.2.3;echo-pwn", "1.2.3-?"]) {
    assert.throws(() => parseSemver(invalid), /Invalid semver/);
  }
  assert.ok(
    releaseScript.indexOf("targetVersion = resolveReleaseTarget") <
      releaseScript.indexOf('execFileSync("git", ["tag", "-l", tagVersion]'),
  );
  assert.match(releaseScript, /--regenerate requires an explicit semantic version/);
});

test("dry-run exits before every release write", () => {
  const dryRunGuard = releaseScript.indexOf("if (isDryRun) {");
  for (const write of [
    'run(process.execPath, ["scripts/version.mjs"',
    'run("cargo", ["generate-lockfile"',
    'run(process.execPath, ["scripts/changelog.mjs"]',
    'run("git", ["add"',
    'run("git", ["tag", "-a"',
    'run("git", branchPushArgs)',
  ]) {
    assert.ok(releaseScript.indexOf(write) > dryRunGuard, `${write} must remain after the guard`);
  }
  assert.match(
    releaseScript,
    /\["scripts\/changelog\.mjs", "--preview", "--version", targetVersion\]/,
  );
  assert.match(releaseScript, /✓ Dry run complete\. Planned tag/);
});

test("release commits whitelist only canonical release files", () => {
  assert.deepEqual(findUnexpectedReleaseChanges(RELEASE_FILES), []);
  assert.deepEqual(
    findUnexpectedReleaseChanges(["package.json", "src/routes/+page.svelte"], ["TODO.md"]),
    ["TODO.md", "src/routes/+page.svelte"],
  );
  assert.match(releaseScript, /run\("git", \["add", "--", \.\.\.RELEASE_FILES\]\)/);
});

test("release subprocesses pass refs as literal arguments without a shell", () => {
  assert.doesNotMatch(releaseScript, /execSync|shell:\s*true/);
  assert.match(releaseScript, /execFileSync\(command, args/);
  assert.match(releaseScript, /\["push", "origin", BRANCH\]/);
});
