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

test("browser write event stays aligned across backend and page listener", async () => {
  const [vaultSource, pageSource] = await Promise.all([
    readFile(new URL("../src-tauri/src/vault/mod.rs", import.meta.url), "utf8"),
    readFile(new URL("../src/routes/+page.svelte", import.meta.url), "utf8"),
  ]);
  const event = vaultSource.match(/BROWSER_VAULT_CHANGED_EVENT:\s*&str\s*=\s*"([^"]+)"/);

  assert.ok(event, "backend browser-write event constant must exist");
  assert.match(pageSource, new RegExp(`listen\\("${event[1]}"`));
});

test("global auto-type emits picker events after releasing the vault lock", async () => {
  const source = await readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8");
  const lockedPreparation = source.match(/let action = \{([\s\S]*?)\n    \};/);
  const unlockedDispatch = source.match(/match action \{([\s\S]*?)\r?\n    \}\r?\n\}/);

  assert.ok(lockedPreparation, "global hotkey locked preparation must exist");
  assert.match(lockedPreparation[1], /session\.lock\(\)/);
  assert.doesNotMatch(lockedPreparation[1], /app\.emit|get_webview_window/);
  assert.ok(unlockedDispatch, "global hotkey unlocked dispatch must exist");
  assert.match(unlockedDispatch[1], /app\.emit\("autotype-pick-request"/);
  assert.match(unlockedDispatch[1], /get_webview_window\("main"\)/);
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
  assert.match(destroyBlock[1], /clearTimeout\(notesSaveTimer\)/);
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

test("database metadata rejects detached dialog completions", async () => {
  const dialog = await readFile(
    new URL("../src/lib/components/DbMetaDialog.svelte", import.meta.url),
    "utf8",
  );
  const saveBlock = dialog.match(/async function save\(\): Promise<void> \{([\s\S]*?)\n  \}/);

  assert.match(dialog, /sessionResourceKey\(sessionId, "database-meta"\)/);
  assert.match(dialog, /onDestroy\(\(\) => dialogView\.activate\(null\)\)/);
  assert.match(dialog, /showClose=\{!saving\}/);
  assert.match(dialog, /closeOnEscape=\{!saving\}/);
  assert.ok(saveBlock, "DbMetaDialog save must exist");
  assert.match(saveBlock[1], /const view = dialogView\.capture\(\)/);
  assert.match(saveBlock[1], /if \(!dialogView\.isCurrent\(view\)\) return/);
  assert.match(saveBlock[1], /if \(dialogView\.isCurrent\(view\)\) error =/);
  assert.match(saveBlock[1], /if \(dialogView\.isCurrent\(view\)\) saving = false/);
  assert.doesNotMatch(saveBlock[1], /vault\.getActiveSessionId\(\)/);
});

test("database settings reject detached load and save completions", async () => {
  const dialog = await readFile(
    new URL("../src/lib/components/DatabaseSettingsDialog.svelte", import.meta.url),
    "utf8",
  );
  const saveBlock = dialog.match(/async function save\(\): Promise<void> \{([\s\S]*?)\n  \}/);

  assert.match(dialog, /sessionResourceKey\(sessionId, "database-settings"\)/);
  assert.match(dialog, /onDestroy\(\(\) => dialogView\.activate\(null\)\)/);
  assert.match(dialog, /const view = dialogView\.capture\(\)/);
  assert.match(dialog, /\.then\(\(value\) => \{\s*if \(!dialogView\.isCurrent\(view\)\) return/);
  assert.match(dialog, /if \(dialogView\.isCurrent\(view\)\) loading = false/);
  assert.match(dialog, /closeOnEscape=\{!saving\}/);
  assert.match(dialog, /let cipher = \$state<WritableDatabaseCipher \| null>\(null\)/);
  assert.match(dialog, /value\.cipher === "Twofish" \? null : value\.cipher/);
  assert.match(dialog, /class:active=\{cipher === null\}/);
  assert.match(dialog, /onclick=\{\(\) => \(cipher = null\)\}/);
  assert.match(dialog, /\["Aes256", "ChaCha20"\] as const/);
  assert.match(
    dialog,
    /if \(cipher !== null && cipher !== settings\.cipher\) patch\.cipher = cipher/,
  );
  assert.doesNotMatch(dialog, /\["Aes256", "Twofish", "ChaCha20"\] as const/);
  assert.ok(saveBlock, "DatabaseSettingsDialog save must exist");
  assert.match(saveBlock[1], /const view = dialogView\.capture\(\)/);
  assert.match(saveBlock[1], /if \(!dialogView\.isCurrent\(view\)\) return/);
  assert.match(saveBlock[1], /if \(dialogView\.isCurrent\(view\)\) error =/);
  assert.match(saveBlock[1], /if \(dialogView\.isCurrent\(view\)\) saving = false/);
  assert.doesNotMatch(dialog, /vault\.getActiveSessionId\(\) !== sessionId/);
  assert.doesNotMatch(dialog, /vault\.getActiveSessionId\(\) === sessionId/);
});

test("attachment preview binds every async path to its resource view", async () => {
  const dialog = await readFile(
    new URL("../src/lib/components/AttachmentPreviewDialog.svelte", import.meta.url),
    "utf8",
  );
  const saveBlock = dialog.match(/async function saveToDisk\(\): Promise<void> \{([\s\S]*?)\n  \}/);
  const openBlock = dialog.match(
    /async function openExternal\(\): Promise<void> \{([\s\S]*?)\n  \}/,
  );
  const importBlock = dialog.match(
    /async function importChanges\(\): Promise<void> \{([\s\S]*?)\n  \}/,
  );

  assert.match(dialog, /sessionResourceKey\(sessionId, `\$\{uuid\}\\0\$\{name\}`\)/);
  assert.match(dialog, /dialogView\.activate\(null\)/);
  assert.match(dialog, /if \(!dialogView\.isCurrent\(view\)\) return/);
  assert.match(dialog, /if \(dialogView\.isCurrent\(view\)\) loading = false/);
  assert.match(dialog, /closeOnEscape=\{!importing\}/);
  assert.ok(saveBlock, "AttachmentPreviewDialog saveToDisk must exist");
  assert.match(saveBlock[1], /awaitCurrentView\(dialogView, view, \(\) => save/);
  assert.match(saveBlock[1], /if \(!picked\.current \|\| !picked\.value\) return/);
  assert.match(saveBlock[1], /if \(!dialogView\.isCurrent\(view\)\) return/);
  assert.ok(openBlock, "AttachmentPreviewDialog openExternal must exist");
  assert.match(openBlock[1], /if \(!dialogView\.isCurrent\(view\)\)/);
  assert.match(openBlock[1], /vault\.cleanupAttachmentTemp\(ref\.token\)/);
  assert.ok(importBlock, "AttachmentPreviewDialog importChanges must exist");
  assert.match(importBlock[1], /if \(!dialogView\.isCurrent\(view\)\) return/);
  assert.doesNotMatch(dialog, /vault\.getActiveSessionId\(\) !== sessionId/);
});

test("attachment temp cleanup forgets tokens only after backend success", async () => {
  const source = await readFile(new URL("../src/lib/services/vault.ts", import.meta.url), "utf8");
  const cleanupMethod = source.match(
    /async cleanupAttachmentTemp\(token: string\): Promise<void> \{([\s\S]*?)\n  \},/,
  );
  const cleanupSession = source.match(
    /async function discardTempAttachmentsForSession\([\s\S]*?\n\}/,
  );
  const closeTab = source.match(
    /async closeTab\(sessionId: string\): Promise<void> \{([\s\S]*?)\n  \},/,
  );

  assert.ok(cleanupMethod, "cleanupAttachmentTemp must exist");
  assert.match(
    cleanupMethod[1],
    /await backendInvoke\("cleanup_attachment_temp", \{ token \}\);\s*tempAttachmentTokens\.delete\(token\)/,
  );
  assert.doesNotMatch(cleanupMethod[1], /^\s*tempAttachmentTokens\.delete\(token\)/);
  assert.ok(cleanupSession, "session cleanup helper must exist");
  assert.match(cleanupSession[0], /catch \{\s*\/\/ Keep the token/);
  assert.ok(closeTab, "closeTab must exist");
  assert.match(closeTab[1], /await discardTempAttachmentsForSession\(sessionId\)/);
});

test("remaining async dialogs reset or unmount with their owning view", async () => {
  const page = await readFile(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
  const detail = await readFile(
    new URL("../src/lib/components/EntryDetail.svelte", import.meta.url),
    "utf8",
  );
  const sessionSwitch = page.match(
    /const unsubActive = vault\.activeId\.subscribe\(\(value\) => \{([\s\S]*?)\n    \}\);/,
  );
  const detailReset = detail.match(/\$effect\(\(\) => \{([\s\S]*?)\n  \}\);/);

  assert.ok(sessionSwitch, "page active-session reset must exist");
  for (const state of ["similarOpen", "expiredOpen", "hibpOpen"]) {
    assert.match(sessionSwitch[1], new RegExp(`${state} = false`));
  }

  assert.ok(detailReset, "EntryDetail resource reset effect must exist");
  for (const state of [
    "passwordLoading",
    "customFieldLoading",
    "historyLoading",
    "storageLoading",
    "viewingVersion",
    "previewAttachmentName",
    "attachmentDragActive",
  ]) {
    assert.match(detailReset[1], new RegExp(`${state} = (?:false|\\{\\}|null)`));
  }
});

test("detail-pane drag-and-drop attachment add stays IPC-aligned", async () => {
  const [detail, service, lib, session] = await Promise.all([
    readFile(new URL("../src/lib/components/EntryDetail.svelte", import.meta.url), "utf8"),
    readFile(new URL("../src/lib/services/vault.ts", import.meta.url), "utf8"),
    readFile(new URL("../src-tauri/src/lib.rs", import.meta.url), "utf8"),
    readFile(new URL("../src-tauri/src/vault/entries.rs", import.meta.url), "utf8"),
  ]);

  // EntryDetail dropzone calls the service method in the captured session.
  const dropzone = detail.match(
    /class="attachment-dropzone"[\s\S]*?ondrop=\{handleAttachmentDrop\}/,
  );
  assert.ok(dropzone, "attachment dropzone must be wired");
  assert.match(
    detail,
    /vault\.callInSession\(sessionId, \(\) => vault\.addAttachments\(uuid, attachments\)\)/,
  );

  // The service method invokes the backend command by its exact name.
  const method = service.match(
    /async addAttachments\(uuid: string, attachments: AttachmentInput\[\]\): Promise<VaultState> \{([\s\S]*?)\n  \},/,
  );
  assert.ok(method, "vault.addAttachments must exist");
  assert.match(method[1], /invokeSessionState\("add_attachments", \{ uuid, attachments \}\)/);

  // The command is registered and its session method records history.
  assert.match(lib, /commands::add_attachments/);
  assert.match(
    session,
    /pub fn add_attachments\([\s\S]*?&mut self,[\s\S]*?uuid: &str,[\s\S]*?attachments: &\[AttachmentInput\],[\s\S]*?\)/,
  );
  assert.match(session, /track_changes\(\)/);
});
