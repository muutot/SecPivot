#!/usr/bin/env node
/**
 * Fail closed unless the release tag and every version source describe the
 * exact same semantic version. The workflow passes RELEASE_TAG for both tag
 * pushes and manual dispatches.
 */

import { readFileSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const SEMVER = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/;

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
  if (!SEMVER.test(version)) throw new Error(`Repository version is not valid semver: ${version}.`);

  const expectedTag = `v${version}`;
  if (releaseTag !== expectedTag) {
    throw new Error(`Release tag ${releaseTag} does not match repository version ${expectedTag}.`);
  }

  return { releaseTag, version };
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    const { releaseTag, version } = validateReleaseIdentity();
    console.log(`Validated release ${releaseTag} against repository version ${version}.`);
  } catch (error) {
    console.error(`Release identity validation failed: ${error.message}`);
    process.exitCode = 1;
  }
}
