import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";

// Run the production handlers with a deterministic layout/focus seam. Native
// selector matching, CSS visibility and Tab traversal are verified in Edge.
const source = readFileSync(
  new URL("../src/lib/components/ModalShell.svelte", import.meta.url),
  "utf8",
);
const script = source.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
const ast = ts.createSourceFile("modal.ts", script, ts.ScriptTarget.Latest, true);
const selector = ast.statements.find(
  (node) =>
    ts.isVariableStatement(node) &&
    node.declarationList.declarations[0].name.getText(ast) === "FOCUSABLE_SELECTOR",
);
assert.ok(selector);
const functions = ["focusables", "handleKeydown"].map((name) => {
  const node = ast.statements.find(
    (node) => ts.isFunctionDeclaration(node) && node.name?.text === name,
  );
  assert.ok(node);
  return node.getText(ast);
});
const js = ts.transpileModule([selector.getText(ast), ...functions].join("\n"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022 },
}).outputText;

function fixture() {
  const doc = { activeElement: null };
  const candidates = [];
  function element(options = {}) {
    const el = {
      tabIndex: 0,
      visibility: "visible",
      rendered: true,
      disabled: false,
      inert: false,
      ...options,
      matches: () => el.disabled,
      closest: () => (el.inert ? el : null),
      getClientRects: () => (el.rendered ? [{}] : []),
      focus() {
        if (el.rendered && el.visibility === "visible" && !el.disabled && !el.inert)
          doc.activeElement = el;
      },
    };
    candidates.push(el);
    return el;
  }
  const root = element({ tabIndex: -1 });
  candidates.length = 0;
  root.querySelectorAll = () => candidates;
  const context = {
    document: doc,
    dialogRef: root,
    escapeScope: { isTop: () => true },
    closeOnEscape: true,
    closes: 0,
    onclose: () => context.closes++,
    getComputedStyle: (el) => ({ visibility: el.visibility }),
  };
  runInNewContext(js, context);
  function key(key = "Tab", extras = {}) {
    const event = {
      key,
      shiftKey: false,
      defaultPrevented: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      ...extras,
    };
    context.handleKeydown(event);
    return event;
  }
  return { doc, root, element, context, key };
}

test("attachment file input must not defeat either edge of the modal Tab trap", () => {
  const f = fixture();
  const save = f.element();
  const add = f.element();
  f.element({ tabIndex: -1, rendered: false });
  save.focus();
  assert.equal(f.key("Tab", { shiftKey: true }).defaultPrevented, true);
  assert.equal(f.doc.activeElement, add);
  assert.equal(f.key().defaultPrevented, true);
  assert.equal(f.doc.activeElement, save);
});

test("negative tabindex controls remain programmatically focusable without joining Tab order", () => {
  const f = fixture();
  const skip = f.element({ tabIndex: -2 });
  const first = f.element();
  const last = f.element();
  f.root.focus();
  f.key();
  assert.equal(f.doc.activeElement, first);
  skip.focus();
  f.key("Tab", { shiftKey: true });
  assert.equal(f.doc.activeElement, last);
});

test("hidden, collapsed, disabled-fieldset and inert controls cannot become wrap targets", () => {
  for (const excluded of [
    { rendered: false },
    { visibility: "hidden" },
    { visibility: "collapse" },
    { disabled: true },
    { inert: true },
  ]) {
    const f = fixture();
    const first = f.element();
    const last = f.element();
    f.element(excluded);
    first.focus();
    f.key("Tab", { shiftKey: true });
    assert.equal(f.doc.activeElement, last, JSON.stringify(excluded));
  }
});

test("an empty enabled Tab order retains focus on the dismissible modal surface", () => {
  const f = fixture();
  f.element({ rendered: false });
  f.root.focus();
  const event = f.key();
  assert.equal(event.defaultPrevented, true);
  assert.equal(f.doc.activeElement, f.root);
  f.key("Escape");
  assert.equal(f.context.closes, 1);
});

test("interior Tab remains native and candidates are recomputed after a control disappears", () => {
  const f = fixture();
  const first = f.element();
  const middle = f.element();
  const last = f.element();
  first.focus();
  assert.equal(f.key().defaultPrevented, false);
  middle.focus();
  assert.equal(f.key().defaultPrevented, false);
  last.rendered = false;
  assert.equal(f.key().defaultPrevented, true);
  assert.equal(f.doc.activeElement, first);
});

test("consumed keys and the lower modal cannot steal keyboard ownership", () => {
  const f = fixture();
  f.element();
  f.root.focus();
  f.key("Tab", { defaultPrevented: true });
  assert.equal(f.doc.activeElement, f.root);
  f.context.escapeScope.isTop = () => false;
  assert.equal(f.key().defaultPrevented, false);
  f.key("Escape");
  assert.equal(f.context.closes, 0);
  f.context.escapeScope.isTop = () => true;
  f.key("Escape");
  assert.equal(f.context.closes, 1);
});
