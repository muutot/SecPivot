import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const releaseScript = readFileSync(new URL("../scripts/release.mjs", import.meta.url), "utf-8");

test("normal releases never infer a force push from ahead/behind counts", () => {
  assert.doesNotMatch(releaseScript, /rev-list --count --left-right/);
  assert.match(releaseScript, /const branchFlag = forcePush \? "--force-with-lease" : ""/);
  assert.match(releaseScript, /const tagFlag = forcePush \? "--force" : ""/);
});
