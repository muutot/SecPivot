import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

test("nested group nodes forward every metadata action", async () => {
  const source = await readFile(
    new URL("../src/lib/components/GroupNode.svelte", import.meta.url),
    "utf8",
  );
  const recursiveBlock = source.match(
    /\{#each group\.children as child \(child\.uuid\)\}([\s\S]*?)\{\/each\}/,
  );

  assert.ok(recursiveBlock, "recursive GroupNode block must exist");
  assert.match(recursiveBlock[1], /\{onmeta\}/);
});
