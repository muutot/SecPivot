import { describe, it, beforeEach, afterEach, mock } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";

// Touch long-press → context menu. The entry row and the list both receive
// the same `pointerdown` (pointer events bubble), so a tracker that keeps a
// single "pending timer" slot loses the first registration: the orphaned
// timer fires after the finger lifted and opens the context menu on a plain
// tap. These tests pin the single-owner gesture contract.
const root = new URL("../src", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1");
const tablePath = path.join(root, "lib", "components", "EntryTable.svelte");

const { createLongPressTracker } = await import("../src/lib/utils/long-press.ts");

describe("createLongPressTracker", () => {
  beforeEach(() => {
    mock.timers.enable({ apis: ["setTimeout"] });
  });

  afterEach(() => {
    mock.timers.reset();
  });

  it("does not fire when the finger lifts before the threshold", () => {
    const fired = [];
    const tracker = createLongPressTracker();
    tracker.start("touch", { x: 10, y: 20 }, (point) => fired.push(point));
    tracker.cancel();
    mock.timers.tick(1000);
    assert.deepEqual(fired, []);
  });

  it("fires once with the press point when the threshold elapses", () => {
    const fired = [];
    const tracker = createLongPressTracker();
    tracker.start("touch", { x: 10, y: 20 }, (point) => fired.push(point));
    mock.timers.tick(500);
    assert.deepEqual(fired, [{ x: 10, y: 20 }]);
  });

  it("keeps one armed press per gesture so a bubbled pointerdown cannot arm a second timer", () => {
    // The entry row registers first, the list's blank-area handler second.
    const fired = [];
    const tracker = createLongPressTracker();
    tracker.start("touch", { x: 10, y: 20 }, (point) => fired.push(["row", point]));
    tracker.start("touch", { x: 10, y: 20 }, (point) => fired.push(["list", point]));
    mock.timers.tick(500);
    assert.deepEqual(fired, [["row", { x: 10, y: 20 }]]);
  });

  it("cancels every armed press when the finger lifts, leaving no orphaned timer", () => {
    const fired = [];
    const tracker = createLongPressTracker();
    tracker.start("touch", { x: 10, y: 20 }, (point) => fired.push(["row", point]));
    tracker.start("touch", { x: 10, y: 20 }, (point) => fired.push(["list", point]));
    tracker.cancel();
    mock.timers.tick(1000);
    assert.deepEqual(fired, []);
  });

  it("abandons the press when the finger travels beyond the tolerance", () => {
    const fired = [];
    const tracker = createLongPressTracker();
    tracker.start("touch", { x: 10, y: 20 }, (point) => fired.push(point));
    tracker.move({ x: 30, y: 20 });
    mock.timers.tick(1000);
    assert.deepEqual(fired, []);
  });

  it("keeps the press while the finger stays inside the tolerance", () => {
    const fired = [];
    const tracker = createLongPressTracker();
    tracker.start("touch", { x: 10, y: 20 }, (point) => fired.push(point));
    tracker.move({ x: 14, y: 24 });
    mock.timers.tick(500);
    assert.deepEqual(fired, [{ x: 10, y: 20 }]);
  });

  it("swallows the click that follows a long press, exactly once", () => {
    const tracker = createLongPressTracker();
    tracker.start("touch", { x: 10, y: 20 }, () => {});
    mock.timers.tick(500);
    tracker.cancel();
    assert.equal(tracker.consumeSuppressedClick(), true);
    assert.equal(tracker.consumeSuppressedClick(), false);
  });

  it("does not swallow a later unrelated tap after a long press", () => {
    const tracker = createLongPressTracker();
    tracker.start("touch", { x: 10, y: 20 }, () => {});
    mock.timers.tick(500);
    tracker.cancel();
    assert.equal(tracker.consumeSuppressedClick(), true);
    // The next gesture starts fresh: its click must reach the row.
    tracker.start("touch", { x: 10, y: 20 }, () => {});
    tracker.cancel();
    assert.equal(tracker.consumeSuppressedClick(), false);
  });

  it("never long-presses on a mouse", () => {
    const fired = [];
    const tracker = createLongPressTracker();
    tracker.start("mouse", { x: 10, y: 20 }, (point) => fired.push(point));
    mock.timers.tick(1000);
    assert.deepEqual(fired, []);
  });
});

describe("EntryTable long-press wiring", () => {
  it("uses the shared tracker instead of a component-local press timer", async () => {
    const text = await readFile(tablePath, "utf8");
    assert.ok(
      text.includes('import { createLongPressTracker } from "$lib/utils/long-press"'),
      "EntryTable must import createLongPressTracker from utils/long-press",
    );
    assert.ok(
      !text.includes("pressTimer") && !text.includes("pressStart"),
      "EntryTable must not keep its own long-press timer state",
    );
    assert.ok(
      text.includes("longPress.consumeSuppressedClick()"),
      "the row click must consult the tracker before selecting the entry",
    );
  });
});
