import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolveReleaseTarget } from "../scripts/versioning.mjs";
import { RELEASE_FILES, findUnexpectedReleaseChanges } from "../scripts/release-files.mjs";

const releaseScript = readFileSync(new URL("../scripts/release.mjs", import.meta.url), "utf-8");

test("normal releases never infer a force push from ahead/behind counts", () => {
  assert.doesNotMatch(releaseScript, /rev-list --count --left-right/);
  assert.match(releaseScript, /const branchFlag = forcePush \? "--force-with-lease" : ""/);
  assert.match(releaseScript, /const tagFlag = forcePush \? "--force" : ""/);
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

test("dry-run exits before every release write", () => {
  const dryRunGuard = releaseScript.indexOf("if (isDryRun) {");
  for (const write of [
    "node scripts/version.mjs",
    "cargo generate-lockfile",
    'run("node scripts/changelog.mjs")',
    "run(`git add",
    "run(`git tag -a",
    "run(`git push",
  ]) {
    assert.ok(releaseScript.indexOf(write) > dryRunGuard, `${write} must remain after the guard`);
  }
  assert.match(releaseScript, /changelog\.mjs --preview --version \$\{targetVersion\}/);
  assert.match(releaseScript, /✓ Dry run complete\. Planned tag/);
});

test("release commits whitelist only canonical release files", () => {
  assert.deepEqual(findUnexpectedReleaseChanges(RELEASE_FILES), []);
  assert.deepEqual(
    findUnexpectedReleaseChanges(["package.json", "src/routes/+page.svelte"], ["TODO.md"]),
    ["TODO.md", "src/routes/+page.svelte"],
  );
  assert.match(releaseScript, /git add -- \$\{RELEASE_FILES\.join\(" "\)\}/);
  assert.doesNotMatch(releaseScript, /git add \$\{changedFiles/);
});
