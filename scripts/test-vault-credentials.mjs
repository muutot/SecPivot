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
const preparationSource = readSource("src-tauri/src/vault/persist.rs");
const preparedAlias = preparationSource.match(/^pub\(crate\) type PreparedKeyfile = [^;]+;/m);
const preparedGuarded = Boolean(preparedAlias);
if (preparedGuarded) {
  assert.equal(preparedAlias[0], "pub(crate) type PreparedKeyfile = Zeroizing<Option<Vec<u8>>>;");
}
const keyfileType = preparedGuarded
  ? "type PreparedKeyfile = zeroize::Zeroizing<Option<ObservedBytes>>;"
  : "type PreparedKeyfile = Option<ObservedBytes>;";
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
const masterSource = readSource("src-tauri/src/commands/vault.rs");
const masterMatches = [
  ...masterSource.matchAll(/^pub\(crate\) async fn change_master_key\([^]*?^}/gm),
];
assert.equal(masterMatches.length, 1);
const masterHandler = masterMatches[0][0];
const masterParameters = [...masterHandler.matchAll(/\bpassword: (String|VaultPassword),/g)];
assert.equal(masterParameters.length, 1);
const masterGuarded = masterParameters[0][1] === "VaultPassword";
assert(!masterGuarded || guarded, "Master-change guard requires the shared credential type");
const completion = readSource("src-tauri/src/vault/session.rs").match(
  /pub\(crate\) fn complete_change\([^]*?\) -> Result<VaultState, String>/,
);
assert(completion, "Missing actual master-change completion contract");
const completionBorrowed = /password: &str,/.test(completion[0]);
assert.equal(completionBorrowed, masterGuarded);
const masterTypes = masterGuarded
  ? "use zeroize::Zeroizing;\ntype Password = super::credentials::VaultPassword<Secret>;\nfn observed(value: Secret) -> Password { super::credentials::VaultPassword::new(value) }"
  : "type Password = Secret;\nfn observed(value: Secret) -> Password { value }";
const masterCompletion = completionBorrowed
  ? `fn complete_change(&mut self, password: &str, keyfile: Option<&[u8]>, _: u64, _: ()) -> Result<VaultState, String> {
      check_adoption(password, keyfile); self.copied_password = password.to_owned(); self.copied_keyfile = keyfile.map(Vec::from); finish_adoption()
    }`
  : `fn complete_change(&mut self, password: Password, keyfile: Option<SecretBytes>, _: u64, _: ()) -> Result<VaultState, String> {
      check_adoption(&password, keyfile.as_deref()); self.owned_password = Some(password); self.owned_keyfile = keyfile; finish_adoption()
    }`;
const masterFixture = readFileSync(
  join(root, "tests/fixtures/master-change-credentials.rs"),
  "utf8",
);
for (const marker of ["// MASTER_HANDLER", "// MASTER_TYPES", "// MASTER_COMPLETION"]) {
  assert.equal(masterFixture.split(marker).length, 2);
}
const masterModule = masterFixture
  .replace(
    "// MASTER_HANDLER",
    masterHandler.replace(/\bpassword: (String|VaultPassword),/, "password: Password,"),
  )
  .replace("// MASTER_TYPES", masterTypes)
  .replace("// MASTER_COMPLETION", masterCompletion);
const sessionSource = readSource("src-tauri/src/vault/session.rs");
function sessionMethod(name) {
  const matches = [
    ...sessionSource.matchAll(
      new RegExp(`^    (?:pub(?:\\([^)]*\\))? )?fn ${name}\\([^]*?^    }`, "gm"),
    ),
  ];
  assert.equal(matches.length, 1, `Missing complete production session ${name}`);
  return matches[0][0];
}
function sessionHelper(name, required = true) {
  const matches = [...sessionSource.matchAll(new RegExp(`^fn ${name}\\([^]*?^}`, "gm"))];
  assert.equal(matches.length, required ? 1 : Number(matches.length > 0));
  return matches[0]?.[0] ?? "";
}
const secretHelpers = readSource("src-tauri/src/vault/helpers.rs");
const wipeHelpers = ["wipe_secret_string", "wipe_secret_bytes"].map((name) => {
  const matches = [
    ...secretHelpers.matchAll(new RegExp(`^pub\\(crate\\) fn ${name}\\([^]*?^}`, "gm")),
  ];
  assert.equal(matches.length, 1);
  return matches[0][0];
});
const replacementFixture = readFileSync(
  join(root, "tests/fixtures/keyfile-replacement.rs"),
  "utf8",
);
for (const marker of ["// SESSION_METHODS", "// SESSION_HELPERS"]) {
  assert.equal(replacementFixture.split(marker).length, 2);
}
const replacementModule = replacementFixture
  .replace(
    "// SESSION_METHODS",
    ["adopt_local", "adopt_remote", "replace", "complete_change", "close_impl"]
      .map(sessionMethod)
      .join("\n\n"),
  )
  .replace(
    "// SESSION_HELPERS",
    [
      ...wipeHelpers,
      sessionHelper("wipe_replaced_password"),
      sessionHelper("wipe_replaced_keyfile", false),
    ].join("\n\n"),
  );
const preparationFixture = readFileSync(
  join(root, "tests/fixtures/keyfile-preparation.rs"),
  "utf8",
);
for (const marker of ["// PREPARED_KEYFILE_TYPE", "// PREPARATION_FUNCTIONS"]) {
  assert.equal(preparationFixture.split(marker).length, 2);
}
const preparationFunctions = [
  "prepare_local_open",
  "prepare_local_create",
  "prepare_remote_open",
  "prepare_remote_create",
].map((name) => {
  const matches = [
    ...preparationSource.matchAll(new RegExp(`^pub\\(crate\\) fn ${name}\\([^]*?^}`, "gm")),
  ];
  assert.equal(matches.length, 1, `Missing complete production ${name}`);
  return matches[0][0].replaceAll("Option<Vec<u8>>", "Option<ObservedBytes>");
});
const preparationModule = preparationFixture
  .replace("// PREPARATION_IMPORTS", preparedGuarded ? "use zeroize::Zeroizing;" : "")
  .replace("// PREPARED_KEYFILE_TYPE", keyfileType)
  .replace("// PREPARATION_FUNCTIONS", preparationFunctions.join("\n\n"));
const jobFixture = readFileSync(
  join(root, "tests/fixtures/persistence-job-credentials.rs"),
  "utf8",
);
const jobDeclarations = ["SaveJob", "RemoteRefreshJob", "RemoteMergeJob"].map((name) => {
  const matches = [
    ...preparationSource.matchAll(new RegExp(`^pub\\(crate\\) struct ${name} \\{[^]*?^}`, "gm")),
  ];
  assert.equal(matches.length, 1, `Missing complete production ${name}`);
  return matches[0][0]
    .replaceAll("pub password: String,", "pub password: ObservedString,")
    .replaceAll("pub password: Zeroizing<String>,", "pub password: Zeroizing<ObservedString>,")
    .replaceAll("pub keyfile: Option<Vec<u8>>,", "pub keyfile: Option<ObservedBytes>,")
    .replaceAll(
      "pub keyfile: Zeroizing<Option<Vec<u8>>>,",
      "pub keyfile: Zeroizing<Option<ObservedBytes>>,",
    );
});
const jobConsumers = [
  "persist_save",
  "persist_save_with_db",
  "persist_remote_refresh",
  "persist_remote_merge",
].map((name) => {
  const matches = [
    ...preparationSource.matchAll(new RegExp(`^pub\\(crate\\) fn ${name}\\([^]*?^}`, "gm")),
  ];
  assert.equal(matches.length, 1, `Missing complete production ${name}`);
  return matches[0][0];
});
for (const marker of [
  "// JOB_DECLARATIONS",
  "// WIPE_HELPERS",
  "// PERSISTENCE_CONSUMERS",
  "// JOB_FACTORIES",
]) {
  assert.equal(jobFixture.split(marker).length, 2);
}
const jobModule = jobFixture
  .replace("// JOB_DECLARATIONS", jobDeclarations.join("\n\n"))
  .replace(
    "// WIPE_HELPERS",
    wipeHelpers
      .map((body) =>
        body
          .replace("&mut String", "&mut ObservedString")
          .replace("&mut Vec<u8>", "&mut ObservedBytes"),
      )
      .join("\n\n"),
  )
  .replace("// PERSISTENCE_CONSUMERS", jobConsumers.join("\n\n"))
  .replace(
    "// JOB_FACTORIES",
    [
      "prepare_save",
      "prepare_save_as",
      "prepare_remote_refresh",
      "prepare_database_settings_update",
      "prepare_remote_merge",
    ]
      .map(sessionMethod)
      .join("\n\n"),
  );
const fixture = readFileSync(join(root, "tests/fixtures/vault-command-credentials.rs"), "utf8");
const registrySource = readSource("src-tauri/src/vault/sessions.rs");
const registryMatches = [...registrySource.matchAll(/^    pub fn open\([^]*?^    }/gm)];
assert.equal(registryMatches.length, 1, "Missing complete production registry open");
for (const marker of [
  "// PRODUCTION_HANDLERS",
  "// PASSWORD_IMPLEMENTATION",
  "// PASSWORD_TYPE",
  "// PREPARED_KEYFILE_TYPE",
  "// REGISTRY_OPEN",
]) {
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
      .replace("// PASSWORD_TYPE", passwordType)
      .replace("// REGISTRY_OPEN", registryMatches[0][0])
      .replace("// PREPARED_KEYFILE_TYPE", keyfileType) +
      `\nmod master_change {\n${masterModule}\n}\nmod keyfile_replacement {\n${replacementModule}\n}\nmod keyfile_preparation {\n${preparationModule}\n}\nmod persistence_jobs {\n${jobModule}\n}\nmod crypto { pub fn sha256_bytes(_: &[u8]) -> [u8; 32] { [0; 32] } }\n`,
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
