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
const lock = readSource("src-tauri/Cargo.lock");
function version(name) {
  const match = lock.match(new RegExp(`^name = "${name}"\\r?\\nversion = "([0-9.]+)"\\r?$`, "m"));
  assert(match, `Missing locked ${name} version`);
  return match[1];
}
const parameterTypes = [];
const handlers = [
  ["vault", "open_vault"],
  ["vault", "create_vault"],
  ["remote", "open_remote_vault"],
  ["remote", "create_remote_vault"],
].map(([file, name]) => {
  const source = readSource(`src-tauri/src/commands/${file}.rs`);
  // These module-level functions end at column zero; nested blocks do not.
  const matches = [
    ...source.matchAll(new RegExp(`^pub\\(crate\\) async fn ${name}\\([^]*?^}`, "gm")),
  ];
  assert.equal(matches.length, 1, `Cannot extract complete production ${name}`);
  const original = matches[0][0];
  const parameters = [...original.matchAll(/\bpassword: (String|VaultPassword),/g)];
  assert.equal(parameters.length, 1);
  parameterTypes.push(parameters[0][1]);
  // Only the password parameter's type is substituted. Its safe destructor
  // observes clearing while still alive; all production control flow is retained.
  return original.replace(/\bpassword: (String|VaultPassword),/, "password: ObservedPassword,");
});
assert.equal(new Set(parameterTypes).size, 1, "All four password boundaries must agree");
const guarded = parameterTypes[0] === "VaultPassword";
const implementation = guarded
  ? `mod credentials {\n${readSource("src-tauri/src/commands/credentials.rs")}\n}`
  : "use zeroize::Zeroizing;";
const passwordType = guarded
  ? "type ObservedPassword = credentials::VaultPassword<ObservedSecret>;\ntype WirePassword = credentials::VaultPassword;\nfn observed(value: ObservedSecret) -> ObservedPassword { credentials::VaultPassword::new(value) }"
  : "type ObservedPassword = ObservedSecret;\ntype WirePassword = String;\nfn observed(value: ObservedSecret) -> ObservedPassword { value }";
const fixture = readFileSync(join(root, "tests/fixtures/vault-command-credentials.rs"), "utf8");
for (const marker of ["// PRODUCTION_HANDLERS", "// PASSWORD_IMPLEMENTATION", "// PASSWORD_TYPE"]) {
  assert.equal(fixture.split(marker).length, 2);
}
const temporaryRoot = realpathSync(tmpdir());
console.log(`credential fixture source: ${revision ?? "current workspace"}`);
const workspace = mkdtempSync(join(temporaryRoot, "secpivot-credentials-"));
try {
  writeFileSync(
    join(workspace, "lib.rs"),
    fixture
      .replace("// PRODUCTION_HANDLERS", handlers.join("\n\n"))
      .replace("// PASSWORD_IMPLEMENTATION", implementation)
      .replace("// PASSWORD_TYPE", passwordType),
  );
  writeFileSync(
    join(workspace, "Cargo.toml"),
    `[package]\nname = "secpivot-credential-fixture"\nversion = "0.0.0"\nedition = "2021"\n` +
      `[lib]\npath = "lib.rs"\n[dependencies]\nzeroize = "=${version("zeroize")}"\n` +
      `serde = "=${version("serde")}"\nserde_json = "=${version("serde_json")}"\n` +
      `tokio = { version = "=${version("tokio")}", features = ["rt-multi-thread", "sync", "time", "macros"] }\n`,
  );
  // Cargo may prune the private copy, but shared packages start at the exact
  // repository lock versions instead of picking newer cached transitive crates.
  writeFileSync(join(workspace, "Cargo.lock"), lock);
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
  const path = realpathSync(workspace);
  assert(
    path.startsWith(temporaryRoot + sep) && basename(path).startsWith("secpivot-credentials-"),
  );
  assert.equal(path, workspace);
  rmSync(path, { recursive: true });
}
