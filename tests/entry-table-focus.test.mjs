import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import test from "node:test";
import { runInNewContext } from "node:vm";
import { findNearestEntryIndex } from "../src/lib/utils/entry-nav.ts";

const require = createRequire(import.meta.url);
const ts = require("typescript");
const source = readFileSync(
  new URL("../src/lib/components/EntryTable.svelte", import.meta.url),
  "utf8",
);
const ast = ts.createSourceFile(
  "EntryTable.ts",
  source.match(/<script lang="ts">([^]*?)<\/script>/)[1],
  ts.ScriptTarget.Latest,
  true,
);
const focus = ast.statements.find(
  (node) => ts.isFunctionDeclaration(node) && node.name?.text === "focusRowAt",
);
const clamp = ast.statements.find(
  (node) => ts.isExpressionStatement(node) && node.getText(ast).includes("const maxScrollTop"),
);
assert.ok(focus && clamp, "execute the full production focus handler and scroll-clamp effect");
const js = ts.transpileModule(
  focus.getText(ast) +
    "\nexports.focus = focusRowAt; exports.clamp = () => {\n" +
    clamp.getText(ast) +
    "\n};",
  {
    compilerOptions: { target: ts.ScriptTarget.ESNext },
  },
).outputText;

// Model the independently known DOM flow: header, fixed-height rows and 16px
// bottom padding. Actual browser geometry/native keys are checked separately.
function fixture(headerHeight, rowHeight, viewportHeight, count = 100) {
  let scroll = 0;
  const focused = [];
  const context = {
    exports: {},
    rows: Array.from({ length: count }, (_, i) => ({ kind: "entry", entry: { uuid: `row-${i}` } })),
    headerHeight,
    rowHeight,
    viewportHeight,
    scrollTop: 0,
    lastFocusedIndex: 0,
    findNearestEntryIndex,
    tick: async () => {},
    $effect: (run) => run(),
  };
  const table = {
    clientHeight: viewportHeight,
    get scrollTop() {
      return scroll;
    },
    set scrollTop(value) {
      scroll = Math.max(
        0,
        Math.min(value, headerHeight + context.rows.length * rowHeight + 16 - viewportHeight),
      );
    },
    querySelector(selector) {
      const index = Number(selector.match(/data-entry-index="(\d+)"/)[1]);
      return { focus: (options) => focused.push({ index, options }) };
    },
  };
  context.entryTableEl = table;
  runInNewContext(js, context);
  return {
    ...context.exports,
    context,
    table,
    focused,
    bounds(index) {
      return {
        top: headerHeight + index * rowHeight - scroll,
        bottom: headerHeight + (index + 1) * rowHeight - scroll,
      };
    },
  };
}

for (const [name, headerHeight, rowHeight, viewportHeight] of [
  ["desktop", 28, 30, 150],
  ["narrow full columns", 28, 36, 200],
  ["narrow summary", 0, 36, 200],
]) {
  test(`${name}: End target is fully visible and Home clears the sticky header`, async () => {
    const f = fixture(headerHeight, rowHeight, viewportHeight);
    await f.focus(99, -1);
    const last = f.bounds(99);
    assert.ok(last.top >= headerHeight, "focused row must be below the sticky header");
    assert.ok(last.bottom <= viewportHeight, "focused row bottom must remain inside the table");
    assert.equal(f.focused.at(-1).index, 99);
    assert.equal(f.focused.at(-1).options.preventScroll, true);
    await f.focus(0, 1);
    assert.equal(f.bounds(0).top, headerHeight);
    assert.equal(f.table.scrollTop, 0);
  });
}

test("clamping after filtering preserves enough scrolling space for the final row", async () => {
  const f = fixture(28, 30, 150);
  await f.focus(99, -1);
  f.context.rows = f.context.rows.slice(0, 20);
  f.clamp();
  assert.equal(f.table.scrollTop, 478);
  assert.equal(f.bounds(19).bottom, 150);
  await f.focus(19, -1);
  assert.equal(f.bounds(19).bottom, 150);
});

test("focus uses clientHeight when the resize observer has not populated viewport height", async () => {
  const f = fixture(28, 30, 150);
  f.context.viewportHeight = 0;
  await f.focus(99, -1);
  assert.equal(f.bounds(99).bottom, 150);
});
