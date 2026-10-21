import test from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import {
  validateReleaseCommit,
  validateReleaseIdentity,
} from "../scripts/validate-release-version.mjs";

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

/** Fixture repository with one tagged commit and (optionally) a later commit. */
function fixtureRepo({ extraCommit = false } = {}) {
  const repo = mkdtempSync(join(tmpdir(), "secpivot-release-identity-"));
  const git = (args) =>
    execFileSync("git", args, { cwd: repo, encoding: "utf-8", stdio: "pipe" }).trim();
  git(["init"]);
  git(["config", "user.name", "SecPivot Test"]);
  git(["config", "user.email", "test@secpivot.invalid"]);
  git(["config", "commit.gpgsign", "false"]);
  git(["commit", "--allow-empty", "-m", "release"]);
  git(["tag", "v1.2.3"]);
  if (extraCommit) git(["commit", "--allow-empty", "-m", "after release"]);
  return { repo, git };
}

test("release commit binding accepts the tagged commit", () => {
  const { repo, git } = fixtureRepo();
  try {
    const result = validateReleaseCommit("v1.2.3", { cwd: repo });
    assert.equal(result.tagCommit, git(["rev-parse", "HEAD"]));
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

test("release commit binding rejects a checkout that moved past the tag", () => {
  const { repo } = fixtureRepo({ extraCommit: true });
  try {
    assert.throws(
      () => validateReleaseCommit("v1.2.3", { cwd: repo }),
      /is not the commit of v1\.2\.3/,
    );
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

test("release commit binding rejects a tag that does not exist", () => {
  const { repo } = fixtureRepo();
  try {
    assert.throws(
      () => validateReleaseCommit("v9.9.9", { cwd: repo }),
      /does not exist in this checkout/,
    );
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});
