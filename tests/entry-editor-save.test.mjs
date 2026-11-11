import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import { runInNewContext } from "node:vm";
import { compileModule } from "svelte/compiler";
import ts from "typescript";

// Execute the production rune module through the actual compiler. Only its
// service/locale boundaries are seams; no save orchestration is copied here.
const source = readFileSync("src/lib/composables/useEntryEditor.svelte.ts", "utf8");
const javascript = ts.transpile(source, { target: ts.ScriptTarget.ESNext });
const compiled = compileModule(javascript, { generate: "server" }).js.code;
const commonjs = ts.transpile(compiled, { module: ts.ModuleKind.CommonJS });
function fixture(failAt = "flags") {
  let live = { root: { entries: [{ uuid: "synthetic-entry", title: "Before" }] } };
  const calls = [];
  const requests = [];
  const notices = [];
  let current = true;
  const vault = {
    callInSession: async (id, run) => {
      assert.equal(id, "synthetic-owner");
      return run();
    },
    saveEntry: async (request) => {
      requests.push(request);
      calls.push("save");
      assert.equal(request.input.title, "After");
      assert.equal(request.autoType.enabled, false);
      if (failAt) throw new Error("Synthetic transaction refusal");
      const uuid = request.target.mode === "create" ? "created-exact-uuid" : request.target.uuid;
      live = { root: { entries: [{ uuid, title: request.input.title }] } };
      return { uuid, state: live };
    },
    updateEntries: async (uuids) => {
      assert.equal(uuids.length, 2);
      calls.push("batch");
      return live;
    },
  };
  const exports = {};
  runInNewContext(commonjs, {
    exports,
    require: (name) => {
      if (name === "$lib/services/vault") return { vault };
      if (name === "$lib/services/settings") return { appSettings: {} };
      if (name === "svelte/store") return { get: () => ({ general: { language: "en" } }) };
      if (name === "$lib/i18n") return { t: (_lang, key) => key };
      if (name === "svelte/internal/server") return {};
      throw new Error(`Unexpected compiler import: ${name}`);
    },
  });
  const editor = exports.useEntryEditor({
    sessionView: { capture: () => ({ sessionId: "synthetic-owner" }), isCurrent: () => current },
    notify: (message, kind) => notices.push({ message, kind }),
    findEntry: (state, uuid) => state.root.entries.find((entry) => entry.uuid === uuid),
    setSingleSelection: () => calls.push("selection"),
    setSelectedEntry: () => calls.push("highlight"),
    getSelectedEntry: () => live.root.entries[0],
  });
  editor.openEdit(live.root.entries[0]);
  return {
    editor,
    calls,
    requests,
    notices,
    getLive: () => live,
    obsolete: () => {
      current = false;
    },
  };
}
const input = { groupUuid: "root", title: "After", username: "", password: "", url: "", notes: "" };
const autoType = { enabled: false, associations: [] };
const flags = { foregroundColor: "invalid" };

test("single edit and creation forward website rules in the same transaction", async () => {
  const policy = {
    mode: "set",
    rules: { version: 1, rules: [{ url: "https://synthetic.example", mode: "host" }] },
  };
  for (const create of [false, true]) {
    const f = fixture(null);
    if (create) f.editor.openCreate();
    await f.editor.handleSave(input, null, autoType, {}, policy);
    assert.equal(f.requests.length, 1);
    assert.equal(f.requests[0].websiteRules, policy);
    assert.deepEqual(f.calls, ["save", "selection"]);
  }
});

test("failed editor save leaves the whole draft uncommitted", async () => {
  const f = fixture();
  await f.editor.handleSave(input, null, autoType, flags);
  assert.equal(f.notices[0].message, "editor.opFailed");
  assert.equal(f.editor.editorOpen, true);
  assert.equal(f.getLive().root.entries[0].title, "Before");
  assert.deepEqual(f.calls, ["save"]);
});
test("primary refusal leaves content unchanged", async () => {
  const f = fixture("content");
  await f.editor.handleSave(input, null, autoType, flags);
  assert.equal(f.getLive().root.entries[0].title, "Before");
  assert.equal(f.editor.editorOpen, true);
});
test("successful editor save closes and selects the saved entry", async () => {
  const f = fixture(null);
  await f.editor.handleSave(input, null, autoType, { foregroundColor: "#112233" });
  assert.equal(f.getLive().root.entries[0].title, "After");
  assert.equal(f.editor.editorOpen, false);
  assert.equal(f.notices[0].message, "editor.savedChanges");
  assert.deepEqual(f.calls, ["save", "selection"]);
});

test("failed creation remains retryable without a partial entry", async () => {
  const f = fixture();
  f.editor.openCreate();
  await f.editor.handleSave(input, null, autoType, flags);
  assert.equal(f.getLive().root.entries.length, 1);
  assert.equal(f.getLive().root.entries[0].title, "Before");
  assert.equal(f.editor.editorOpen, true);
  assert.equal(f.notices[0].message, "editor.opFailed");
});

test("creation selects the transaction UUID through one call", async () => {
  const f = fixture(null);
  f.editor.openCreate();
  await f.editor.handleSave(input, null, autoType, { foregroundColor: "#112233" });
  assert.equal(f.getLive().root.entries[0].uuid, "created-exact-uuid");
  assert.equal(f.editor.editorOpen, false);
  assert.deepEqual(f.calls, ["save", "selection"]);
  assert.equal(f.notices[0].message, "editor.createdEntry");
});

test("obsolete edit completion cannot select or close the new view", async () => {
  const f = fixture(null);
  f.obsolete();
  await f.editor.handleSave(input, null, autoType, {});
  assert.equal(f.editor.editorOpen, true);
  assert.deepEqual(f.calls, ["save"]);
  assert.equal(f.notices.length, 0);
});

test("multi-edit keeps one batch call and the highlighted-row rebind", async () => {
  const f = fixture(null);
  const first = f.getLive().root.entries[0];
  f.editor.openEdit(first, [first, { ...first, uuid: "second" }]);
  await f.editor.handleSave(null, { title: "After" }, null, null);
  assert.deepEqual(f.calls, ["batch", "highlight"]);
  assert.equal(f.editor.editorOpen, false);
  assert.equal(f.notices[0].message, "editor.updatedEntries");
});
