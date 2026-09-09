import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { hibpResultState } from "../src/lib/utils/hibp.ts";

function input(overrides = {}) {
  return {
    started: false,
    running: false,
    failed: false,
    findingCount: 0,
    cancelled: false,
    ...overrides,
  };
}

describe("hibpResultState", () => {
  it("stays idle before the explicit start", () => {
    assert.equal(hibpResultState(input()), "idle");
  });

  it("prefers running, then error", () => {
    assert.equal(hibpResultState(input({ started: true, running: true })), "running");
    assert.equal(hibpResultState(input({ started: true, running: true, failed: true })), "running");
    assert.equal(hibpResultState(input({ started: true, failed: true })), "error");
  });

  it("reports clean only for a completed run with zero findings", () => {
    assert.equal(hibpResultState(input({ started: true })), "clean");
  });

  it("never reports clean for a cancelled run", () => {
    assert.equal(hibpResultState(input({ started: true, cancelled: true })), "cancelled-clean");
  });

  it("marks collected findings as partial after cancel", () => {
    assert.equal(hibpResultState(input({ started: true, findingCount: 3 })), "hits");
    assert.equal(
      hibpResultState(input({ started: true, findingCount: 3, cancelled: true })),
      "cancelled-hits",
    );
  });
});
