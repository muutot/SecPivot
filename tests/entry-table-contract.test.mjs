import { readFile } from "node:fs/promises";
import { test } from "node:test";
import assert from "node:assert/strict";
import path from "node:path";

// EntryTable prop contract: the always-on compact density removed the
// `compact` toggle in favor of `entryRowHeight`, and `DisplayRow` has a
// single source of truth in `utils/display-rows.ts`. These static guards
// fail if either invariant regresses. (Separator key stability is covered
// alongside the key fix; see the P3 performance batch.)
const root = new URL("../src", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const tablePath = path.join(root, "lib", "components", "EntryTable.svelte");

test("EntryTable takes entryRowHeight instead of a compact toggle", async () => {
  const text = await readFile(tablePath, "utf8");
  assert.ok(
    text.includes("entryRowHeight: number"),
    "EntryTable Props must declare entryRowHeight: number",
  );
  assert.ok(!text.includes("compact"), "EntryTable must not reference compact anymore");
});

test("EntryTable reuses the shared DisplayRow type", async () => {
  const text = await readFile(tablePath, "utf8");
  assert.ok(
    text.includes('import type { DisplayRow } from "$lib/utils/display-rows"'),
    "EntryTable must import DisplayRow from utils/display-rows",
  );
  assert.ok(
    !text.includes("export type DisplayRow") && !text.includes("type DisplayRow ="),
    "EntryTable must not declare its own DisplayRow",
  );
  assert.ok(text.includes("rows: DisplayRow[]"), "EntryTable rows prop must be DisplayRow[]");
});
