import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { bumpVersion, parseSemver, resolveReleaseTarget } from "../scripts/versioning.mjs";
import { RELEASE_FILES, findUnexpectedReleaseChanges } from "../scripts/release-files.mjs";
import { hasReleaseHeading } from "../scripts/release-document.mjs";
import { isGitAncestor, isReleaseCommitSubject } from "../scripts/release-git.mjs";

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
    'run("cargo", ["update", "-p", "secpivot-desktop"',
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

test("releases never re-resolve the whole dependency tree", () => {
  // `cargo generate-lockfile` re-resolves *every* dependency to the newest
  // compatible version, so the v1.6.1 commit titled "bump version to 1.6.1"
  // silently carried 153 changed `Cargo.lock` lines of unreviewed transitive
  // upgrades that no CVE scan ever looked at. Only our own lock entry may move.
  // (The name is allowed to appear in comments, just never as a `run` argument.)
  assert.doesNotMatch(releaseScript, /run\(\s*"cargo",\s*\[\s*"generate-lockfile"/);
  assert.match(releaseScript, /\["update", "-p", "secpivot-desktop"/);
});

test("pausing for RELEASE.md curation reports failure, not success", () => {
  // Steps 1-2 have already rewritten the version files and CHANGELOG when this
  // branch is taken, so the tree is dirty and nothing was released. Anchor on
  // the step-3 heading so the dry-run preview's own `exit(0)` is not matched.
  const step3 = releaseScript.indexOf("// Step 3: RELEASE.md check");
  assert.ok(step3 > 0, "step 3 must still exist");
  const pause = releaseScript.indexOf("RELEASE.md needs update", step3);
  assert.ok(pause > step3, "step 3 must report the curation pause");
  // The explanatory comment between the message and the exit is long, so scan a
  // generous window rather than trying to jump to the next `console.log`.
  const after = releaseScript.slice(pause, pause + 1200);
  assert.match(after, /process\.exit\((?!0\))/);
  assert.doesNotMatch(after, /process\.exit\(0\)/);
});

test("the Cargo.lock sync runs on every attempt, not only on a version bump", () => {
  // A re-run after the curation pause takes the "Already at <version>" branch.
  // When the lock sync lived inside the bump branch it was skipped, leaving
  // `Cargo.lock` at the previous version while still being staged for commit.
  const lockSync = releaseScript.indexOf('["update", "-p", "secpivot-desktop"');
  const bumpBranch = releaseScript.indexOf("if (currentVersion !== targetVersion) {");
  const bumpBranchEnd = releaseScript.indexOf("} else {", bumpBranch);
  assert.ok(lockSync > bumpBranchEnd, "the lock sync must sit after the bump if/else");
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

test("release notes require the exact target heading instead of a body mention", () => {
  assert.equal(hasReleaseHeading("# SecPivot Desktop v1.3.0\n", "1.3.0"), true);
  assert.equal(hasReleaseHeading("\uFEFF# SecPivot Desktop v1.3.0\r\n", "1.3.0"), true);
  assert.equal(
    hasReleaseHeading("# SecPivot Desktop v1.2.0\n\nUpcoming compatibility: v1.3.0\n", "1.3.0"),
    false,
  );
  assert.match(releaseScript, /hasReleaseHeading\(readFileSync\(RELEASE_PATH/);
});

test("regeneration accepts only the exact release subject", () => {
  assert.equal(isReleaseCommitSubject("🔖 chore[release]: bump version to 1.2.0", "1.2.0"), true);
  assert.equal(
    isReleaseCommitSubject("fix: mention chore[release] and bump version to 1.2.0", "1.2.0"),
    false,
  );
  assert.equal(isReleaseCommitSubject("🔖 chore[release]: bump version to 1.1.0", "1.2.0"), false);
});

test("regeneration rejects a release tag from an unrelated branch", () => {
  const repo = mkdtempSync(join(tmpdir(), "secpivot-release-git-test-"));
  const git = (...args) =>
    execFileSync("git", args, { cwd: repo, encoding: "utf-8", stdio: "pipe" }).trim();

  try {
    git("init");
    git("config", "user.name", "SecPivot Test");
    git("config", "user.email", "test@secpivot.invalid");
    git("config", "commit.gpgsign", "false");
    git("commit", "--allow-empty", "-m", "release");
    const releaseCommit = git("rev-parse", "HEAD");
    git("commit", "--allow-empty", "-m", "after release");
    assert.equal(isGitAncestor(repo, releaseCommit), true);

    git("switch", "--orphan", "unrelated");
    git("commit", "--allow-empty", "-m", "unrelated history");
    assert.equal(isGitAncestor(repo, releaseCommit), false);
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});
