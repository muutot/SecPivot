import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

function topLevelFunction(source, name) {
  const startPattern = new RegExp(`^  (?:async )?function ${name}\\(`, "m");
  const start = source.search(startPattern);
  if (start === -1) return null;
  const tail = source.slice(start);
  const next = tail.slice(1).search(/^  (?:async )?function \w+\(/m);
  return next === -1 ? tail : tail.slice(0, next + 1);
}

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

test("TCATO open attempts release only their own focus-lock lease", async () => {
  const source = await readFile(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
  const openBlock = source.match(
    /async function openTcatoOverlay\(entry: VaultEntry\): Promise<void> \{([\s\S]*?)\n  \}/,
  );

  assert.ok(openBlock, "openTcatoOverlay must exist");
  assert.match(openBlock[1], /const focusLockLease = beginTcatoOverlayOpen\(\)/);
  assert.match(openBlock[1], /const view = sessionView\.capture\(\)/);
  assert.match(openBlock[1], /const operation = tcatoOperations\.begin\(\)/);
  assert.match(openBlock[1], /focusLockLease\.confirm\(\)/);
  assert.match(
    openBlock[1],
    /sessionView\.isCurrent\(view\) && tcatoOperations\.isCurrent\(operation\)/,
  );
  assert.match(openBlock[1], /finally \{\s*focusLockLease\.release\(\)/);
  assert.doesNotMatch(openBlock[1], /setTcatoOverlayOpen\(/);
});

test("page mutations gate completion UI by the originating view epoch", async () => {
  const source = await readFile(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
  const asyncActions = [
    "toggleFavorite",
    "handleClearHistory",
    "renameGroup",
    "saveGroupMeta",
    "restoreGroup",
    "restoreEntry",
    "moveEntriesTo",
    "toggleGroupExpanded",
    "toggleGroupsExpanded",
    "copyEntryValue",
    "runAutoType",
  ];
  const confirmedActions = [
    "askDeleteGroup",
    "askEmptyRecycleBin",
    "askDeleteEntry",
    "askDeleteEntries",
  ];

  for (const name of asyncActions) {
    const block = topLevelFunction(source, name);
    assert.ok(block, `${name} must exist`);
    assert.match(block, /const view = sessionView\.capture\(\)/, `${name} must capture a view`);
    assert.match(block, /sessionView\.isCurrent\(view\)/, `${name} must gate completion`);
    assert.doesNotMatch(block, /vault\.getActiveSessionId\(\)/);
  }

  for (const name of confirmedActions) {
    const block = topLevelFunction(source, name);
    assert.ok(block, `${name} must exist`);
    assert.match(block, /const view = sessionView\.capture\(\)/, `${name} must capture a view`);
    assert.match(
      block,
      /onconfirm: async \(\) => \{\s*if \(!sessionView\.isCurrent\(view\)\) return/,
    );
    assert.doesNotMatch(block, /vault\.getActiveSessionId\(\)/);
  }
});

test("group metadata waits for the parent save before closing", async () => {
  const dialog = await readFile(
    new URL("../src/lib/components/GroupMetaDialog.svelte", import.meta.url),
    "utf8",
  );
  const page = await readFile(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
  const saveBlock = topLevelFunction(page, "saveGroupMeta");

  assert.match(dialog, /onsaved: \([^]*?\) => Promise<boolean>/);
  assert.match(dialog, /const current = await onsaved\(/);
  assert.match(dialog, /if \(current\) onclose\(\)/);
  assert.match(dialog, /closeOnEscape=\{!saving\}/);
  assert.match(dialog, /onclick=\{onclose\} disabled=\{saving\}/);
  assert.ok(saveBlock, "saveGroupMeta must exist");
  assert.match(saveBlock, /Promise<boolean>/);
  assert.match(saveBlock, /groupMetaUuid !== uuid/);
  assert.match(page, /onsaved=\{saveGroupMeta\}/);
});

test("group auto-type rejects detached or replaced dialog completions", async () => {
  const dialog = await readFile(
    new URL("../src/lib/components/GroupAutoTypeDialog.svelte", import.meta.url),
    "utf8",
  );
  const saveBlock = dialog.match(/async function save\(\): Promise<void> \{([\s\S]*?)\n  \}/);

  assert.match(dialog, /const dialogView = new KeyedViewGuard\(\)/);
  assert.match(dialog, /sessionResourceKey\(sessionId, group\.uuid\)/);
  assert.match(dialog, /onDestroy\(\(\) => dialogView\.activate\(null\)\)/);
  assert.match(dialog, /closeOnEscape=\{!saving\}/);
  assert.ok(saveBlock, "GroupAutoTypeDialog save must exist");
  assert.match(saveBlock[1], /const view = dialogView\.capture\(\)/);
  assert.match(saveBlock[1], /if \(!dialogView\.isCurrent\(view\)\) return/);
  assert.match(saveBlock[1], /if \(dialogView\.isCurrent\(view\)\) error =/);
  assert.match(saveBlock[1], /if \(dialogView\.isCurrent\(view\)\) saving = false/);
  assert.doesNotMatch(saveBlock[1], /vault\.getActiveSessionId\(\)/);
});
