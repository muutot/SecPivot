#!/usr/bin/env node
/**
 * Fail closed unless the release tag and every version source describe the
 * exact same semantic version, and unless the checked-out commit is the one
 * the tag points at. The workflow passes RELEASE_TAG for both tag pushes and
 * manual dispatches.
 *
 * The commit binding is what stops a manual dispatch from rebuilding an
 * already published version from later commits: without it, dispatching
 * `1.6.4` on `main` passes the version check and then replaces the published
 * `v1.6.4` assets with binaries that were never released.
 */

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { parseSemver } from "./versioning.mjs";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function readCargoVersion() {
  const manifest = readFileSync(resolve(ROOT, "src-tauri/Cargo.toml"), "utf-8");
  const packageHeader = manifest.match(/^\[package\]\s*$/m);
  if (!packageHeader || packageHeader.index === undefined) {
    throw new Error("Unable to find [package] in src-tauri/Cargo.toml.");
  }
  const packageBody = manifest.slice(packageHeader.index + packageHeader[0].length);
  const nextSection = packageBody.search(/^\[/m);
  const packageSection = nextSection >= 0 ? packageBody.slice(0, nextSection) : packageBody;
  const version = packageSection.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
  if (!version) throw new Error("Unable to read package version from src-tauri/Cargo.toml.");
  return version;
}

export function validateReleaseIdentity(releaseTag = process.env.RELEASE_TAG) {
  if (!releaseTag) throw new Error("RELEASE_TAG is required.");

  const versions = {
    "package.json": JSON.parse(readFileSync(resolve(ROOT, "package.json"), "utf-8")).version,
    "src-tauri/tauri.conf.json": JSON.parse(
      readFileSync(resolve(ROOT, "src-tauri/tauri.conf.json"), "utf-8"),
    ).version,
    "src-tauri/Cargo.toml": readCargoVersion(),
  };
  const uniqueVersions = new Set(Object.values(versions));

  if (uniqueVersions.size !== 1) {
    throw new Error(
      `Release version sources disagree: ${Object.entries(versions)
        .map(([file, version]) => `${file}=${version}`)
        .join(", ")}.`,
    );
  }

  const version = versions["package.json"];
  try {
    parseSemver(version);
  } catch {
    throw new Error(`Repository version is not valid semver: ${version}.`);
  }

  const expectedTag = `v${version}`;
  if (releaseTag !== expectedTag) {
    throw new Error(`Release tag ${releaseTag} does not match repository version ${expectedTag}.`);
  }

  return { releaseTag, version };
}

/** Resolve a git ref to a commit id, or `""` when the ref does not exist. */
function resolveCommit(ref, cwd) {
  try {
    return execFileSync("git", ["rev-parse", "--verify", "--quiet", `${ref}^{commit}`], {
      cwd,
      encoding: "utf-8",
      stdio: "pipe",
    }).trim();
  } catch {
    return "";
  }
}

/**
 * Fail closed unless the checked-out commit is exactly the commit the release
 * tag points at. A manual dispatch on a branch that already moved past the
 * tag would otherwise publish binaries that do not correspond to the tag.
 */
export function validateReleaseCommit(releaseTag = process.env.RELEASE_TAG, { cwd = ROOT } = {}) {
  if (!releaseTag) throw new Error("RELEASE_TAG is required.");

  const tagCommit = resolveCommit(releaseTag, cwd);
  if (!tagCommit) {
    throw new Error(
      `Release tag ${releaseTag} does not exist in this checkout. Only an existing tag can be released.`,
    );
  }

  const head = resolveCommit("HEAD", cwd);
  if (!head) {
    throw new Error("Unable to resolve the checked-out commit (HEAD).");
  }

  if (head !== tagCommit) {
    throw new Error(
      `Checked-out commit ${head.slice(0, 7)} is not the commit of ${releaseTag} (${tagCommit.slice(
        0,
        7,
      )}). Release only the tagged commit.`,
    );
  }

  return { releaseTag, head, tagCommit };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const { releaseTag, version } = validateReleaseIdentity();
    validateReleaseCommit(releaseTag);
    console.log(`Validated release ${releaseTag} against repository version ${version}.`);
  } catch (error) {
    console.error(`Release identity validation failed: ${error.message}`);
    process.exitCode = 1;
  }
}
