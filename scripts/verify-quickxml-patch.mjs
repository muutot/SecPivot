import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { dirname, resolve, sep } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const vendor = resolve(root, "src-tauri/vendor/quick-xml-0.41.0");
const provenance = JSON.parse(
  readFileSync(resolve(root, "src-tauri/vendor/quick-xml-patch-provenance.json"), "utf8"),
);
assert.equal(provenance.version, "0.41.0");
assert.equal(
  provenance.archive_sha256,
  "e660451e55124f798a69a5af3f49ccfbefbd41910eefd25caf2393e1f3473ec1",
);
assert.equal(provenance.files.length, 43);
const seen = new Set();
for (const file of provenance.files) {
  assert(!seen.has(file.path), `Duplicate provenance path: ${file.path}`);
  seen.add(file.path);
  const path = resolve(vendor, file.path);
  assert(path.startsWith(vendor + sep), "Provenance path leaves the vendor package");
  const actual = createHash("sha256").update(readFileSync(path)).digest("hex");
  assert.equal(actual, file.patched_sha256 ?? file.upstream_sha256, `Vendor drift: ${file.path}`);
  assert.equal(Boolean(file.patched_sha256), file.path === "src/de/map.rs");
}
assert(seen.has("src/de/map.rs"));
const manifest = readFileSync(resolve(root, "src-tauri/Cargo.toml"), "utf8");
assert.match(manifest, /quick-xml\s*=\s*"=0\.41\.0"/);
assert.match(
  manifest,
  /\[patch\.crates-io\][\s\S]*quick-xml\s*=\s*\{\s*path\s*=\s*"vendor\/quick-xml-0\.41\.0"/,
);
const attributes = readFileSync(resolve(root, ".gitattributes"), "utf8");
assert.match(
  attributes,
  /^\/src-tauri\/vendor\/quick-xml-0\.41\.0\/\*\* -text -eol whitespace=cr-at-eol$/m,
);
assert.match(
  attributes,
  /^\/src-tauri\/vendor\/quick-xml-local\.patch -text -eol whitespace=cr-at-eol$/m,
);
const lock = readFileSync(resolve(root, "src-tauri/Cargo.lock"), "utf8");
const packages = lock.split("[[package]]").filter((entry) => /\nname = "quick-xml"\n/.test(entry));
const local = packages.find((entry) => /\nversion = "0\.41\.0"\n/.test(entry));
assert(local && !/^source =/m.test(local), "QuickXML 0.41 must use the local reviewed source");
const registry = packages.find((entry) => /\nversion = "0\.42\.0"\n/.test(entry));
assert(
  registry && /^source = "registry\+/m.test(registry),
  "QuickXML 0.42 remains a registry dependency",
);
const source = readFileSync(resolve(vendor, "src/de/map.rs"), "utf8");
const formatted = spawnSync("rustfmt", ["--edition", "2021", "--emit", "stdout"], {
  cwd: root,
  input: source,
  encoding: "utf8",
});
if (formatted.error) throw formatted.error;
assert.equal(formatted.status, 0, formatted.stderr);
assert.equal(formatted.stdout.replaceAll("\r\n", "\n"), source.replaceAll("\r\n", "\n"));
console.log("quick-xml 0.41.0 provenance: 43 verified files, 1 documented patch; format passed");
