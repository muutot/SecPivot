import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";
import vm from "node:vm";
import ts from "typescript";

// Execute the production handler bodies. Service and view seams use only
// synthetic data; no DOM/appearance or real WebView claim is made here.
const source = readFileSync("src/lib/components/EntryDetail.svelte", "utf8");
const selection = { index: 1, selectionToken: "synthetic:4:entry:1", modified: null };
function fixture(name, { fails = false, confirms = true } = {}) {
  const match = source.match(new RegExp(`^  async function ${name}\\([^]*?^  \\}`, "m"));
  assert.ok(match);
  const calls = [];
  const notices = [];
  let current = true;
  const context = vm.createContext({
    entry: { uuid: "synthetic-entry" },
    dateLocale: "en-US",
    lang: "en",
    historyLoadedUuid: "synthetic-entry",
    viewingVersion: selection,
    detailSessionId: () => "synthetic-owner",
    detailView: { capture: () => ({}), isCurrent: () => current },
    window: { confirm: () => confirms },
    t: (_lang, key) => key,
    loadHistory: async () => calls.push("reload"),
    flash: (kind) => notices.push(kind),
    showTip: (message, kind) => notices.push({ message, kind }),
    vault: {
      callInSession: async (id, run) => {
        assert.equal(id, "synthetic-owner");
        return run();
      },
      restoreEntryVersion: async (uuid, selected) => {
        calls.push({ uuid, selected });
        if (fails) throw vm.runInContext('new Error("Synthetic stale selection")', context);
      },
      deleteEntryHistory: async (uuid, selected) => {
        calls.push({ uuid, selected });
        if (fails) throw vm.runInContext('new Error("Synthetic stale selection")', context);
      },
    },
  });
  vm.runInContext(
    ts.transpile(`${match[0]}\nglobalThis.run = ${name};`, { target: ts.ScriptTarget.ES2022 }),
    context,
  );
  return {
    context,
    calls,
    notices,
    obsolete: () => {
      current = false;
    },
  };
}

test("history detail forwards the exact selected object through both operations", async () => {
  for (const name of ["restoreVersion", "deleteVersion"]) {
    const f = fixture(name);
    await f.context.run(selection);
    assert.equal(f.calls[0].selected, selection);
    assert.equal(f.calls[0].uuid, "synthetic-entry");
    assert.equal(f.calls[1], "reload");
    assert.deepEqual(f.notices, [name === "restoreVersion" ? "restored" : "deleted"]);
  }
});

test("stale selection failures close the old preview and invalidate the list", async () => {
  for (const name of ["restoreVersion", "deleteVersion"]) {
    const f = fixture(name, { fails: true });
    await f.context.run(selection);
    assert.equal(f.context.historyLoadedUuid, null);
    assert.equal(f.context.viewingVersion, null);
    assert.deepEqual(f.notices, [{ message: "Synthetic stale selection", kind: "error" }]);
    assert.equal(f.calls.length, 1);
  }
});

test("cancelled history confirmations do not mutate or reload", async () => {
  for (const name of ["restoreVersion", "deleteVersion"]) {
    const f = fixture(name, { confirms: false });
    await f.context.run(selection);
    assert.deepEqual(f.calls, []);
    assert.deepEqual(f.notices, []);
  }
});

test("obsolete history operation responses cannot notify a new detail view", async () => {
  for (const name of ["restoreVersion", "deleteVersion"]) {
    for (const fails of [false, true]) {
      const f = fixture(name, { fails });
      const request = f.context.run(selection);
      f.obsolete();
      await request;
      assert.deepEqual(f.notices, []);
      assert.equal(f.calls.length, 1);
      assert.equal(f.context.viewingVersion, selection);
    }
  }
});
