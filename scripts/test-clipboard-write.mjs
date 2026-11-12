import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
assert(args.length === 0 || (args.length === 2 && args[0] === "--source-ref"));
const revision = args.length
  ? execFileSync("git", ["rev-parse", "--verify", "--end-of-options", `${args[1]}^{commit}`], {
      cwd: root,
      encoding: "utf8",
    }).trim()
  : null;
function readSource(path) {
  return revision
    ? execFileSync("git", ["show", `${revision}:${path}`], { cwd: root, encoding: "utf8" })
    : readFileSync(join(root, path), "utf8");
}
const source = readSource("src-tauri/src/platform/clipboard.rs");
const normalized = source.replaceAll("\r\n", "\n");
const end = normalized.indexOf("#[cfg(test)]\npub(crate) mod tests");
assert(end > 0, "Missing platform test boundary");
// Every production body remains intact. Only the Win32 import root and cfg
// selector point to the synthetic API; no actual OS clipboard function links.
const extracted = normalized
  .slice(0, end)
  .replaceAll("windows_sys::", "crate::windows_sys::")
  .replaceAll('target_os = "windows"', 'feature = "windows-fixture"');
assert(extracted.includes("pub fn write_clipboard_bytes("));
assert(extracted.includes("pub fn write_clipboard_text("));
const lock = readSource("src-tauri/Cargo.lock");
const version = lock.match(/^name = "zeroize"\r?\nversion = "([0-9.]+)"\r?$/m)?.[1];
assert(version, "Missing locked zeroize");
const fixture = readFileSync(join(root, "tests/fixtures/clipboard-native-write.rs"), "utf8");
assert.equal(fixture.split("// PRODUCTION_CLIPBOARD").length, 2);
const temporaryRoot = realpathSync(tmpdir());
const workspace = mkdtempSync(join(temporaryRoot, "secpivot-clipboard-write-"));
try {
  writeFileSync(join(workspace, "clipboard.rs"), extracted);
  writeFileSync(
    join(workspace, "fixture.rs"),
    fixture.replace("// PRODUCTION_CLIPBOARD", `mod clipboard {\n${extracted}\n}`),
  );
  writeFileSync(join(workspace, "Cargo.lock"), lock);
  writeFileSync(
    join(workspace, "Cargo.toml"),
    `[package]\nname = "secpivot-clipboard-write-fixture"\nversion = "0.0.0"\nedition = "2021"\n` +
      `[features]\ndefault = ["windows-fixture"]\nwindows-fixture = []\n` +
      `[dependencies]\nzeroize = "=${version}"\n` +
      `[lib]\npath = "fixture.rs"\n[profile.dev]\ndebug = 0\nincremental = false\n`,
  );
  const result = spawnSync(
    "cargo",
    [
      "test",
      "--offline",
      "-j",
      "1",
      "--manifest-path",
      join(workspace, "Cargo.toml"),
      "--target-dir",
      join(root, "src-tauri/target"),
      "--lib",
      "--",
      "--test-threads=1",
    ],
    { cwd: root, stdio: "inherit", env: process.env },
  );
  if (result.error) throw result.error;
  process.exitCode = result.status ?? 1;
} finally {
  const resolved = realpathSync(workspace);
  assert.equal(resolved, workspace);
  assert(resolved.startsWith(temporaryRoot + sep));
  assert(basename(resolved).startsWith("secpivot-clipboard-write-"));
  rmSync(resolved, { recursive: true });
}
