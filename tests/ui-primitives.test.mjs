import { readFile } from "node:fs/promises";
import { globSync } from "node:fs";
import { test } from "node:test";
import assert from "node:assert/strict";
import path from "node:path";

// Shared-primitive completeness: `.toggle-switch` renders its sliding knob
// through a child `.toggle-knob` span (settings-shared.css). A switch without
// that child shows an empty track — exactly the drift static class-name greps
// cannot catch. Every usage must carry the knob.
test("every toggle-switch usage carries its toggle-knob child", async () => {
  const root = new URL("../src", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
  const files = globSync(path.join(root, "**", "*.svelte"));
  assert.ok(files.length > 0, "expected svelte sources");

  let switches = 0;
  for (const file of files) {
    const text = await readFile(file, "utf8");
    const matches = text.matchAll(
      /<button[^>]*class="[^"]*toggle-switch[^"]*"[^>]*>([\s\S]*?)<\/button>/g,
    );
    for (const match of matches) {
      switches += 1;
      assert.ok(
        match[1].includes("toggle-knob"),
        `${path.relative(root, file)}: a toggle-switch button is missing its <span class="toggle-knob"></span> child`,
      );
    }
  }
  // Guard against silently matching nothing after a future refactor.
  assert.ok(switches >= 4, `expected at least 4 toggle-switch usages, found ${switches}`);
});
