import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";

const source = readFileSync(
  new URL("../src/lib/components/templates/form/RadioGroup.svelte", import.meta.url),
  "utf8",
);
const script = source.match(/<script[^>]*>([\s\S]*?)<\/script>/)[1];
const ast = ts.createSourceFile("radio.ts", script, ts.ScriptTarget.Latest, true);
const handlers = ["select", "handleKeydown"].map((name) => {
  const node = ast.statements.find(
    (node) => ts.isFunctionDeclaration(node) && node.name?.text === name,
  );
  assert.ok(node, `missing production handler: ${name}`);
  return node.getText(ast);
});
const js = ts.transpileModule(handlers.join("\n"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022 },
}).outputText;

function fixture(values = ["Aes", "Argon2", "Argon2id"]) {
  const changes = [];
  const focus = [];
  const peers = values.map((_, index) => ({ focus: () => focus.push(index) }));
  const parent = { querySelectorAll: () => peers };
  const context = {
    value: values[0],
    options: values.map((value) => ({ value, label: value })),
    disabled: false,
    onchange: (value) => changes.push(value),
  };
  runInNewContext(js, context);
  function press(key, index = 0, extras = {}) {
    const event = {
      key,
      defaultPrevented: false,
      altKey: false,
      ctrlKey: false,
      metaKey: false,
      currentTarget: { parentElement: parent },
      stopped: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopPropagation() {
        this.stopped = true;
      },
      ...extras,
    };
    context.handleKeydown(event, index);
    return event;
  }
  return { context, changes, focus, press };
}

test("radio arrows select and focus the neighboring value, wrapping in both directions", () => {
  for (const [key, index, target] of [
    ["ArrowRight", 0, 1],
    ["ArrowDown", 2, 0],
    ["ArrowLeft", 0, 2],
    ["ArrowUp", 2, 1],
  ]) {
    const f = fixture();
    const event = f.press(key, index);
    assert.equal(f.context.value, f.context.options[target].value);
    assert.deepEqual(f.changes, [f.context.value]);
    assert.deepEqual(f.focus, [target]);
    assert.equal(event.defaultPrevented, true);
    assert.equal(event.stopped, true);
  }
});

test("radio Home and End keep boundary focus and selection aligned", () => {
  const f = fixture();
  f.press("End", 0);
  f.press("Home", 2);
  assert.deepEqual(f.changes, ["Argon2id", "Aes"]);
  assert.deepEqual(f.focus, [2, 0]);
});

test("radio leaves Tab, Escape and native Enter/Space activation to their owners", () => {
  for (const key of ["Tab", "Escape", "Enter", " ", "a"]) {
    const f = fixture();
    const event = f.press(key);
    assert.equal(event.defaultPrevented, false);
    assert.equal(event.stopped, false);
    assert.equal(f.context.value, "Aes");
    assert.deepEqual(f.changes, []);
    assert.deepEqual(f.focus, []);
  }
  const f = fixture();
  f.context.select("Argon2"); // Native click/Enter/Space share this production callback.
  assert.equal(f.context.value, "Argon2");
  assert.deepEqual(f.changes, ["Argon2"]);
});

test("radio does not take consumed or modified navigation keys", () => {
  for (const extras of [
    { defaultPrevented: true },
    { altKey: true },
    { ctrlKey: true },
    { metaKey: true },
  ]) {
    const f = fixture();
    f.press("ArrowRight", 0, extras);
    assert.deepEqual(f.changes, []);
    assert.deepEqual(f.focus, []);
  }
});

test("disabled, empty and singleton radio groups have bounded navigation", () => {
  const disabled = fixture();
  disabled.context.disabled = true;
  assert.equal(disabled.press("ArrowRight").defaultPrevented, false);
  assert.deepEqual(disabled.changes, []);
  const empty = fixture([]);
  assert.equal(empty.press("ArrowRight").defaultPrevented, false);
  assert.deepEqual(empty.focus, []);
  const single = fixture(["Only"]);
  single.press("ArrowLeft");
  assert.equal(single.context.value, "Only");
  assert.deepEqual(single.focus, [0]);
});
