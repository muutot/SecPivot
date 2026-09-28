#!/usr/bin/env node
// Frontend test runner for `node --test`.
//
// Why this exists instead of `node --test tests/*.test.mjs`:
//   * `node --test` on Node 24 does not expand globs itself, and a pattern that
//     matches nothing (renamed files, moved subdirectory, a typo) reports
//     "0 tests" and exits 0. The gate was therefore *fail-open*: a pull request
//     could delete or rename every frontend test and still go green.
//   * `node --test tests/` is not portable either — it treats the directory as
//     a single test file on Node 24.
//   * The glob was also non-recursive, so tests moved into a subdirectory were
//     silently dropped.
//
// This runner enumerates the files itself, refuses to run when the suite is
// unexpectedly small, and then hands the explicit list to `node --test`.

import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const TEST_DIR = join(ROOT, "tests");
const SUFFIX = ".test.mjs";
/** Floor on the number of *test files* discovered (currently 26). This guards
 *  discovery, not coverage: if the files stop being found — renamed, moved into
 *  a subdirectory, excluded by a bad ignore rule — the suite would otherwise
 *  report success having run nothing. Lower it only together with a deliberate
 *  test-file removal, never to make the gate pass. */
const MIN_TEST_FILES = 20;

/** Every `*.test.mjs` under `tests/`, recursively and deterministically. */
function collectTests(dir) {
  const found = [];
  for (const entry of readdirSync(dir, { withFileTypes: true }).sort((a, b) =>
    a.name < b.name ? -1 : a.name > b.name ? 1 : 0,
  )) {
    const full = join(dir, entry.name);
    if (entry.isDirectory()) found.push(...collectTests(full));
    else if (entry.name.endsWith(SUFFIX)) found.push(full);
  }
  return found;
}

let files;
try {
  files = collectTests(TEST_DIR);
} catch (error) {
  console.error(`test:frontend: cannot read ${relative(ROOT, TEST_DIR)}: ${error.message}`);
  process.exit(1);
}

if (files.length === 0) {
  console.error(`test:frontend: no *${SUFFIX} files under ${relative(ROOT, TEST_DIR)}`);
  process.exit(1);
}
if (files.length < MIN_TEST_FILES) {
  console.error(
    `test:frontend: only ${files.length} test files discovered, expected at least ${MIN_TEST_FILES}. ` +
      `The suite was truncated — fix the discovery before shipping.`,
  );
  process.exit(1);
}

console.log(`test:frontend: running ${files.length} test files`);
const result = spawnSync(process.execPath, ["--test", ...files], {
  cwd: ROOT,
  stdio: "inherit",
});
process.exit(result.status ?? 1);
