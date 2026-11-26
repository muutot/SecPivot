import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
import { runInNewContext } from "node:vm";
import {
  findNextEntryIndex,
  pageDownTargetIndex,
  pageUpTargetIndex,
} from "../src/lib/utils/entry-nav.ts";

const require = createRequire(import.meta.url);
const ts = require("typescript");
const source = readFileSync(
  new URL("../src/lib/components/EntryTable.svelte", import.meta.url),
  "utf8",
);
const script = source.match(/<script lang="ts">([^]*?)<\/script>/)[1];
const ast = ts.createSourceFile("EntryTable.ts", script, ts.ScriptTarget.Latest, true);
const handlers = ast.statements.filter(
  (node) =>
    ts.isFunctionDeclaration(node) &&
    ["handleRowKeydown", "handleListKeydown"].includes(node.name?.text),
);
assert.equal(handlers.length, 2);
const js = ts.transpileModule(
  handlers.map((node) => node.getText(ast)).join("\n") +
    "\nexports.row = handleRowKeydown; exports.list = handleListKeydown;",
  { compilerOptions: { target: ts.ScriptTarget.ESNext } },
).outputText;

// Complete production key handlers plus the real navigation helpers. Native
// button click/default activation is left to the browser, not simulated here.
function fixture() {
  const selected = [];
  const focused = [];
  const exports = {};
  let all = 0;
  runInNewContext(js, {
    exports,
    viewportHeight: 300,
    rowHeight: 30,
    lastFocusedIndex: 0,
    rows: ["A", "B"].map((uuid) => ({ kind: "entry", entry: { uuid } })),
    onselectentry: (entry) => selected.push(entry.uuid),
    onselectall: () => all++,
    focusRowAt: (index) => focused.push(index),
    findNextEntryIndex,
    pageDownTargetIndex,
    pageUpTargetIndex,
  });
  return { ...exports, selected, focused, all: () => all };
}
function event(key, target, currentTarget = target, ctrlKey = false) {
  return {
    key,
    target,
    currentTarget,
    ctrlKey,
    metaKey: false,
    defaultPrevented: false,
    stopped: false,
    preventDefault() {
      this.defaultPrevented = true;
    },
    stopPropagation() {
      this.stopped = true;
    },
  };
}

test("inline action activation never selects a row or consumes native Enter/Space", () => {
  const f = fixture();
  const row = {};
  const list = {};
  for (const action of ["favorite", "copyUsername", "copyPassword"]) {
    const button = { action };
    for (const key of ["Enter", " "]) {
      const e = event(key, button, row);
      f.row(e, 0, { uuid: "A" });
      assert.equal(e.defaultPrevented, false, "native button activation must remain available");
      e.currentTarget = list;
      f.list(e);
    }
  }
  assert.deepEqual(f.selected, []);
  assert.deepEqual(f.focused, []);
});

test("focused rows still select on Enter, navigate and select all", () => {
  const f = fixture();
  const row = {};
  f.row(event("Enter", row), 0, { uuid: "A" });
  assert.deepEqual(f.selected, ["A"]);
  const down = event("ArrowDown", row);
  f.row(down, 0, { uuid: "A" });
  assert.deepEqual(f.focused, [1]);
  assert.equal(down.defaultPrevented, true);
  assert.equal(down.stopped, true);
  const all = event("a", row, row, true);
  f.row(all, 0, { uuid: "A" });
  assert.equal(f.all(), 1);
  assert.equal(all.defaultPrevented, true);
  assert.equal(all.stopped, true);
});

test("inline buttons retain focus on arrows and keep the existing list select-all shortcut", () => {
  const f = fixture();
  const row = {};
  const button = {};
  const down = event("ArrowDown", button, row);
  f.row(down, 0, { uuid: "A" });
  assert.deepEqual(f.focused, []);
  assert.equal(down.defaultPrevented, false);
  f.row(event("a", button, row, true), 0, { uuid: "A" });
  assert.equal(f.all(), 1);
});
