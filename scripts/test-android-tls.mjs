// Exercise the actual Android net.rs bodies on the host with real reqwest,
// rustls and webpki. Only the Android cfg selector is substituted; no API stubs.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  copyFileSync,
  mkdtempSync,
  readFileSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const format = spawnSync(
  "rustfmt",
  [
    "--check",
    "--edition",
    "2021",
    "--config",
    "skip_children=true",
    join(root, "tests/fixtures/android-tls.rs"),
  ],
  { cwd: root, stdio: "inherit" },
);
if (format.error) throw format.error;
assert.equal(format.status, 0, "Android TLS fixture must be formatted");
const provenance = JSON.parse(
  readFileSync(join(root, "tests/fixtures/tls-public-fixtures.json"), "utf8"),
);
for (const sample of provenance.sources) {
  const bytes = readFileSync(join(root, "tests/fixtures", sample.file));
  assert.equal(createHash("sha256").update(bytes).digest("hex"), sample.sha256);
}
const lock = readFileSync(join(root, "src-tauri/Cargo.lock"), "utf8");
function version(name) {
  const entries = lock
    .split("[[package]]")
    .filter((entry) => entry.includes(`\nname = "${name}"\n`));
  assert.equal(entries.length, 1, `Expected one locked ${name}`);
  return entries[0].match(/\nversion = "([\d.]+)"/)[1];
}
const source = readFileSync(join(root, "src-tauri/src/net.rs"), "utf8")
  .replaceAll("\r\n", "\n")
  .replaceAll('target_os = "android"', 'feature = "android-fixture"');
assert.equal((source.match(/feature = "android-fixture"/g) ?? []).length, 5);
const temporaryRoot = realpathSync(tmpdir());
const workspace = mkdtempSync(join(temporaryRoot, "secpivot-android-tls-"));
try {
  writeFileSync(join(workspace, "net.rs"), source);
  copyFileSync(join(root, "tests/fixtures/android-tls.rs"), join(workspace, "fixture.rs"));
  for (const file of [
    "tls-removed-emsign-g3.der",
    "tls-distrusted-spkis.der",
    "tls-localhost-cert.der",
    "tls-localhost-key.der",
  ]) {
    copyFileSync(join(root, "tests/fixtures", file), join(workspace, file));
  }
  writeFileSync(join(workspace, "Cargo.lock"), lock);
  writeFileSync(
    join(workspace, "Cargo.toml"),
    `[package]\nname = "secpivot-android-tls-fixture"\nversion = "0.0.0"\nedition = "2021"\n` +
      `[features]\ndefault = ["android-fixture"]\nandroid-fixture = []\n` +
      `[dependencies]\nreqwest = { version = "=${version("reqwest")}", features = ["rustls", "blocking"] }\n` +
      `rustls = "=${version("rustls")}"\nrustls-webpki = "=${version("rustls-webpki")}"\n` +
      `rustls-pki-types = "=${version("rustls-pki-types")}"\nwebpki-root-certs = "=${version("webpki-root-certs")}"\n` +
      `tokio = { version = "=${version("tokio")}", features = ["rt-multi-thread", "macros"] }\n` +
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
    { cwd: root, stdio: "inherit" },
  );
  if (result.error) throw result.error;
  process.exitCode = result.status ?? 1;
} finally {
  const resolved = realpathSync(workspace);
  assert.equal(resolved, workspace);
  assert(resolved.startsWith(temporaryRoot + sep));
  assert(basename(resolved).startsWith("secpivot-android-tls-"));
  rmSync(resolved, { recursive: true });
}
