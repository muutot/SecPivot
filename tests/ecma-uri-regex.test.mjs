import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import test from "node:test";

test("independent JavaScript RegExp oracle remains exact for the native URI fixture", () => {
  const cases = JSON.parse(
    readFileSync(new URL("./fixtures/ecma-uri-regex.json", import.meta.url), "utf8"),
  );
  assert.equal(cases.length, 32);
  for (const [pattern, text, expected] of cases) {
    assert.equal(new RegExp(pattern, "i").test(text), expected);
  }
});
