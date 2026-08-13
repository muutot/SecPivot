#!/usr/bin/env node
/**
 * release.mjs — Release orchestration.
 *
 * Normal flow:
 *   1. Bump version across all configs
 *   2. Generate changelog from commits since last tag
 *   3. Verify RELEASE.md references the target version (exit for LLM to curate)
 *   4. Commit version files + CHANGELOG.md + RELEASE.md
 *   5. Create git tag
 *   6. Push to origin (triggers CI/CD)
 *
 * Regenerate mode (--regenerate):
 *   Before normal flow, drops the old release commit + tag from history
 *   (preserving other commits' content and timestamps), then runs normal flow.
 *
 * Usage:
 *   node scripts/release.mjs <version|patch|minor|major>
 *   node scripts/release.mjs --regenerate <version>
 *   node scripts/release.mjs --dry-run <version>
 */

import { execSync } from "node:child_process";
import { readFileSync, existsSync } from "node:fs";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { argv, exit } from "node:process";
import { resolveReleaseTarget } from "./versioning.mjs";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, "..");
const RELEASE_PATH = resolve(ROOT, "RELEASE.md");
const BRANCH = execSync("git rev-parse --abbrev-ref HEAD", { cwd: ROOT, encoding: "utf-8" }).trim();

function run(cmd, opts = {}) {
  console.log(`  > ${cmd}`);
  try {
    return execSync(cmd, {
      cwd: ROOT,
      encoding: "utf-8",
      stdio: opts.silent ? "pipe" : "inherit",
      shell: true,
      ...opts,
    });
  } catch (err) {
    console.error(`\n  ERROR: ${err.stderr || err.message}`);
    exit(1);
  }
}

function getVersion() {
  return JSON.parse(readFileSync(resolve(ROOT, "package.json"), "utf-8")).version;
}

function getCommittedVersion() {
  const packageJson = execSync("git show HEAD:package.json", {
    cwd: ROOT,
    encoding: "utf-8",
  });
  return JSON.parse(packageJson).version;
}

function checkReleaseMd(ver) {
  if (!existsSync(RELEASE_PATH)) return false;
  return readFileSync(RELEASE_PATH, "utf-8").includes(`v${ver}`);
}

// --- Parse args ---
const args = argv.slice(2);
const isRegenerate = args.includes("--regenerate");
const isDryRun = args.includes("--dry-run");
const versionArg = args.filter((a) => !a.startsWith("--"))[0];

if (!versionArg) {
  console.log(
    `Usage: node scripts/release.mjs [--regenerate] [--dry-run] <version|patch|minor|major>`,
  );
  console.log(`Current version: ${getVersion()}`);
  exit(1);
}

// --- Regenerate: drop old release commit + tag before normal flow ---
let forcePush = false;
if (isRegenerate) {
  const tagVer = `v${versionArg}`;
  const tagExists =
    execSync(`git tag -l "${tagVer}"`, { cwd: ROOT, encoding: "utf-8" }).trim() === tagVer;

  if (tagExists) {
    const tagCommit = execSync(`git rev-list -n 1 "${tagVer}"`, {
      cwd: ROOT,
      encoding: "utf-8",
    }).trim();
    const shortSha = tagCommit.slice(0, 7);
    const commitMsg = execSync(`git log --format="%s" -1 "${tagCommit}"`, {
      cwd: ROOT,
      encoding: "utf-8",
    }).trim();

    if (commitMsg.includes("chore[release]") || commitMsg.includes("bump version to")) {
      console.log(`\n[Regenerate] Found old release commit ${shortSha}: "${commitMsg}"`);
      if (!isDryRun) {
        const parentSha = execSync(`git rev-list --parents -n 1 "${tagCommit}"`, {
          cwd: ROOT,
          encoding: "utf-8",
        })
          .trim()
          .split(" ")[1];
        console.log(`  Dropping commit ${shortSha} via rebase (onto ${parentSha.slice(0, 7)})...`);
        run(`git rebase --onto ${parentSha} ${tagCommit}`);
        run(`git tag -d ${tagVer}`);
        forcePush = true;
        console.log(`  ✓ Old release commit removed, tag ${tagVer} deleted\n`);
      } else {
        console.log(`  (would drop ${shortSha} and tag ${tagVer} in real run)\n`);
      }
    }
  }
}

// --- Normal flow ---
let currentVersion = getVersion();
let targetVersion;
try {
  targetVersion = resolveReleaseTarget(currentVersion, getCommittedVersion(), versionArg);
} catch (error) {
  console.error(`Release version resolution failed: ${error.message}`);
  exit(1);
}
let tagVersion = `v${targetVersion}`;

if (isDryRun) {
  console.log(`\n[Dry run] ${BRANCH}: ${currentVersion} → ${targetVersion}`);
  console.log("\n[1/6] Version files (preview only)");
  console.log(`  Would set package, Tauri, Cargo, and lockfile versions to ${targetVersion}.`);
  console.log("\n[2/6] Changelog preview");
  run(`node scripts/changelog.mjs --preview --version ${targetVersion}`);
  console.log("\n[3/6] RELEASE.md check");
  console.log(
    checkReleaseMd(targetVersion)
      ? `  ✓ RELEASE.md matches v${targetVersion}`
      : `  RELEASE.md needs update for v${targetVersion}.`,
  );
  console.log("\n[4/6] Commit (skipped in dry-run mode)");
  console.log("\n[5/6] Tag (skipped in dry-run mode)");
  console.log("\n[6/6] Push (skipped in dry-run mode)");
  console.log(`\n✓ Dry run complete. Planned tag: ${tagVersion}`);
  exit(0);
}

// Step 1: Bump version
console.log(`\n[1/6] Bumping version (${BRANCH})...`);
if (currentVersion !== targetVersion) {
  run(`node scripts/version.mjs ${targetVersion}`);
  currentVersion = getVersion();
  run("cargo generate-lockfile --manifest-path src-tauri/Cargo.toml", { silent: true });
  console.log(`  ✓ ${currentVersion}`);
} else {
  console.log(`  ✓ Already at ${currentVersion}`);
}

// Step 2: Generate changelog
console.log("\n[2/6] Generating changelog...");
run("node scripts/changelog.mjs");

// Step 3: RELEASE.md check
console.log("\n[3/6] Checking RELEASE.md...");
if (!checkReleaseMd(currentVersion)) {
  console.log(`  RELEASE.md needs update for v${currentVersion}.`);
  console.log("  → Read CHANGELOG.md and curate RELEASE.md, then re-run.");
  process.exit(0);
}
console.log(`  ✓ RELEASE.md matches v${currentVersion}`);

// Step 4: Commit
console.log("\n[4/6] Committing...");
const changedFiles = execSync("git diff --name-only", { cwd: ROOT, encoding: "utf-8" }).trim();
if (changedFiles) {
  run(`git add ${changedFiles.split("\n").join(" ")}`);
  run(`git commit -m "\u{1F516} chore[release]: bump version to ${currentVersion}"`);
  console.log("  ✓ Committed");
} else {
  console.log("  No changes to commit.");
}

// Step 5: Tag
console.log("\n[5/6] Tagging...");
const exists =
  execSync(`git tag -l "${tagVersion}"`, { cwd: ROOT, encoding: "utf-8" }).trim() === tagVersion;
if (!exists) {
  run(`git tag -a ${tagVersion} -m "Release ${tagVersion}"`);
  console.log(`  ✓ ${tagVersion}`);
} else {
  console.log(`  ✓ Tag ${tagVersion} already exists`);
}

// Step 6: Push
console.log("\n[6/6] Pushing...");

// Only the explicit regenerate flow is authorized to rewrite published
// history. A normal branch being ahead/behind must never silently turn a
// release into a force push; let the regular push fail for manual recovery.
const branchFlag = forcePush ? "--force-with-lease" : "";
const tagFlag = forcePush ? "--force" : "";
run(`git push origin ${BRANCH} ${branchFlag}`.trim());
run(`git push origin ${tagVersion} ${tagFlag}`.trim());
console.log(`  ✓ Pushed ${BRANCH} and ${tagVersion}`);

console.log(`\n✓ Release ${currentVersion} complete! Tag: ${tagVersion}`);
