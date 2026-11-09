import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const vendor = resolve(root, "src-tauri/vendor/keepass-0.13.22");
const provenance = JSON.parse(
  readFileSync(resolve(root, "src-tauri/vendor/keepass-patch-provenance.json"), "utf8"),
);
assert.equal(provenance.version, "0.13.22");
assert.equal(
  provenance.archive_sha256,
  "a845a36b656474a0e73e71bfb7dec34250244df364815b77abe1067f17e59e8e",
);
const patched = new Set([
  "src/db/types/entry.rs",
  "src/db/types/mod.rs",
  "src/format/xml_db/mod.rs",
]);
assert.equal(provenance.files.length, 60);
const seen = new Set();
for (const file of provenance.files) {
  assert(!seen.has(file.path), `Duplicate provenance path: ${file.path}`);
  seen.add(file.path);
  const path = resolve(vendor, file.path);
  assert(path.startsWith(vendor + sep), "Provenance path leaves the vendor package");
  const actual = createHash("sha256").update(readFileSync(path)).digest("hex");
  assert.equal(actual, file.patched_sha256 ?? file.upstream_sha256, `Vendor drift: ${file.path}`);
  assert.equal(Boolean(file.patched_sha256), patched.has(file.path), `Patch scope: ${file.path}`);
}
for (const path of patched) assert(seen.has(path), `Missing patched source: ${path}`);
const manifest = readFileSync(resolve(root, "src-tauri/Cargo.toml"), "utf8");
assert.match(manifest, /keepass\s*=\s*\{\s*version\s*=\s*"=0\.13\.22"/);
assert.match(
  manifest,
  /\[patch\.crates-io\][\s\S]*keepass\s*=\s*\{\s*path\s*=\s*"vendor\/keepass-0\.13\.22"/,
);
for (const path of patched) {
  const source = readFileSync(resolve(vendor, path), "utf8");
  // stdin formatting checks only this file; formatting mod.rs by path also
  // traverses and rewrites untouched upstream modules.
  const formatted = spawnSync("rustfmt", ["--edition", "2021", "--emit", "stdout"], {
    cwd: root,
    input: source,
    encoding: "utf8",
  });
  if (formatted.error) throw formatted.error;
  assert.equal(formatted.status, 0, `rustfmt failed for ${path}: ${formatted.stderr}`);
  assert.equal(
    formatted.stdout.replaceAll("\r\n", "\n"),
    source.replaceAll("\r\n", "\n"),
    `Patched source needs formatting: ${path}`,
  );
}
console.log("keepass 0.13.22 provenance: 60 verified files, 3 documented patches; format passed");
