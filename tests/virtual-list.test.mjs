import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { computeVirtualRange } from "../src/lib/utils/virtual-list.ts";

function range(overrides = {}) {
  return {
    itemCount: 100,
    itemHeight: 30,
    scrollTop: 0,
    viewportHeight: 300,
    overscan: 6,
    ...overrides,
  };
}

describe("computeVirtualRange", () => {
  it("mounts nothing for an empty list", () => {
    assert.deepEqual(computeVirtualRange(range({ itemCount: 0 })), { start: 0, end: 0 });
  });

  it("adds the overscan buffer around the visible window", () => {
    // Visible rows 0..10 plus 6 overscan rows per edge.
    assert.deepEqual(computeVirtualRange(range()), { start: 0, end: 16 });
  });

  it("slides the window with the scroll offset", () => {
    // Visible rows 10..20 plus overscan.
    assert.deepEqual(computeVirtualRange(range({ scrollTop: 300 })), { start: 4, end: 26 });
  });

  it("clamps negative scroll offsets to the top", () => {
    assert.deepEqual(computeVirtualRange(range({ scrollTop: -50 })), { start: 0, end: 16 });
  });

  it("clamps over-scrolled offsets to the bottom", () => {
    // maxScrollTop = 100*30-300 = 2700: visible rows 90..100 plus overscan.
    assert.deepEqual(computeVirtualRange(range({ scrollTop: 99999 })), {
      start: 84,
      end: 100,
    });
  });

  it("honors a zero overscan as the exact window", () => {
    assert.deepEqual(computeVirtualRange(range({ overscan: 0 })), { start: 0, end: 10 });
  });

  it("degrades safely on non-finite or negative inputs", () => {
    assert.deepEqual(computeVirtualRange(range({ itemCount: Number.NaN })), {
      start: 0,
      end: 0,
    });
    assert.deepEqual(computeVirtualRange(range({ itemHeight: 0 })), {
      start: 0,
      end: 100,
    });
    assert.deepEqual(computeVirtualRange(range({ viewportHeight: -5 })), {
      start: 0,
      end: 7,
    });
  });
});
