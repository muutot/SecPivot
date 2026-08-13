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

test("entry detail invalidates detached secret-copy consumers", async () => {
  const source = await readFile(
    new URL("../src/lib/components/EntryDetail.svelte", import.meta.url),
    "utf8",
  );
  const destroyBlock = source.match(/onDestroy\(\(\) => \{([\s\S]*?)\n  \}\);/);
  const guardedConsumers = source.match(/consumeCurrentView\(\s*detailView,\s*view,/g);

  assert.ok(destroyBlock, "EntryDetail must clean up when detached");
  assert.match(destroyBlock[1], /detailView\.activate\(null\)/);
  assert.match(destroyBlock[1], /clearTimeout\(copiedTimer\)/);
  assert.equal(guardedConsumers?.length, 2);
  assert.match(source, /ensurePassword\(view\)/);
  assert.match(source, /ensureCustomField\(name, view\)/);
});

test("entry attachment save rejects stale native-picker results", async () => {
  const source = await readFile(
    new URL("../src/lib/components/EntryDetail.svelte", import.meta.url),
    "utf8",
  );
  const saveBlock = source.match(
    /async function saveAttachment\(name: string\): Promise<void> \{([\s\S]*?)\n  \}/,
  );

  assert.ok(saveBlock, "EntryDetail saveAttachment must exist");
  assert.match(saveBlock[1], /awaitCurrentView\(detailView, view, \(\) =>\s*saveDialog/);
  assert.match(saveBlock[1], /if \(!picked\.current \|\| !picked\.value\) return/);
  assert.match(saveBlock[1], /const dest = picked\.value/);
  assert.match(saveBlock[1], /vault\.saveAttachment\(uuid, name, dest\)/);
});
