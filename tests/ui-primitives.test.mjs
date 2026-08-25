import { readFile } from "node:fs/promises";
import { globSync } from "node:fs";
import { test } from "node:test";
import assert from "node:assert/strict";
import path from "node:path";

// Template-first doctrine: control styling lives inside
// src/lib/components/templates/**; feature components compose templates and
// never re-declare primitive markup or styles. Retired shared-CSS primitives
// are listed here once their usages are fully migrated; the list grows as the
// migration proceeds and each entry forbids reintroduction.
const RETIRED_CLASSES = [
  "toggle-switch",
  "toggle-knob",
  "text-input",
  "modal-button",
  "menu-item",
  "settings-input",
  "settings-action-button",
  "settings-feedback",
];

const root = new URL("../src", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const templatesRoot = path.join(root, "lib", "components", "templates");

test("the Toggle template owns its knob structure", async () => {
  const text = await readFile(path.join(templatesRoot, "form", "Toggle.svelte"), "utf8");
  assert.ok(text.includes('class="knob"'), "Toggle.svelte must render its knob child");
});

test("retired primitive classes are not reintroduced outside templates", async () => {
  const files = globSync(path.join(root, "**", "*.svelte")).filter(
    (file) => !file.startsWith(templatesRoot),
  );
  assert.ok(files.length > 0, "expected svelte sources");

  const retiredPattern = new RegExp(
    String.raw`class="[^"]*\b(` + RETIRED_CLASSES.join("|") + String.raw`)\b`,
  );
  const violations = [];
  for (const file of files) {
    const text = await readFile(file, "utf8");
    if (retiredPattern.test(text)) {
      violations.push(path.relative(root, file));
    }
  }
  assert.deepEqual(violations, [], "retired classes must only exist inside templates/");
});
