import assert from "node:assert/strict";
import { cpSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const packages = {
  keepass: { folder: "keepass-0.13.22", features: "_merge,save_kdbx4", filter: "secpivot_" },
  quickxml: { folder: "quick-xml-0.41.0", features: "serialize", filter: null },
};
const selected = packages[process.argv[2]];
assert(selected, "Select keepass or quickxml");
const temporaryRoot = realpathSync(tmpdir());
const workspace = mkdtempSync(join(temporaryRoot, "secpivot-vendor-"));
try {
  const copied = join(workspace, selected.folder);
  // Cargo may update the standalone lock for the path override. Published
  // vendor locks stay byte-identical by testing only this disposable source copy.
  cpSync(join(root, "src-tauri/vendor", selected.folder), copied, { recursive: true });
  const xml = resolve(root, "src-tauri/vendor/quick-xml-0.41.0").replaceAll("\\", "/");
  const args = [
    "test",
    "-j",
    "1",
    "--manifest-path",
    join(copied, "Cargo.toml"),
    "--target-dir",
    join(root, "src-tauri/target"),
    "--features",
    selected.features,
    "--lib",
    "--config",
    `patch.crates-io.quick-xml.path=${JSON.stringify(xml)}`,
  ];
  if (selected.filter) args.push(selected.filter);
  const result = spawnSync("cargo", args, { cwd: root, stdio: "inherit", env: process.env });
  if (result.error) throw result.error;
  process.exitCode = result.status ?? 1;
} finally {
  const resolved = realpathSync(workspace);
  assert(
    resolved.startsWith(temporaryRoot + sep) && basename(resolved).startsWith("secpivot-vendor-"),
  );
  assert.equal(resolved, workspace);
  rmSync(resolved, { recursive: true });
}
