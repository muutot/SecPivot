import { describe, it, mock } from "node:test";
import assert from "node:assert/strict";
import {
  dispatchShortcut,
  effectiveShortcuts,
  matchesShortcut,
} from "../src/lib/services/keyboard.ts";

function keyEvent(partial) {
  return {
    key: "",
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...partial,
  };
}

describe("effectiveShortcuts", () => {
  it("falls back to action defaults when no binding is stored", () => {
    const merged = effectiveShortcuts({});
    assert.equal(merged.save, "Ctrl+S");
    assert.equal(merged["copy-password"], "Ctrl+Shift+C");
  });

  it("keeps stored bindings and still fills missing ones", () => {
    const merged = effectiveShortcuts({ save: "Ctrl+Shift+S" });
    assert.equal(merged.save, "Ctrl+Shift+S");
    assert.equal(merged.lock, "Ctrl+L");
  });
});

describe("matchesShortcut", () => {
  it("matches plain keys case-insensitively", () => {
    assert.equal(matchesShortcut(keyEvent({ key: "s" }), "S"), true);
    assert.equal(matchesShortcut(keyEvent({ key: "F" }), "F2"), false);
  });

  it("requires every combo modifier to be pressed", () => {
    assert.equal(
      matchesShortcut(keyEvent({ key: "c", ctrlKey: true, shiftKey: true }), "Ctrl+Shift+C"),
      true,
    );
    assert.equal(matchesShortcut(keyEvent({ key: "c", ctrlKey: true }), "Ctrl+Shift+C"), false);
  });

  it("rejects extra pressed modifiers not in the combo", () => {
    assert.equal(
      matchesShortcut(keyEvent({ key: "s", ctrlKey: true, altKey: true }), "Ctrl+S"),
      false,
    );
  });

  it("accepts modifiers in any order", () => {
    assert.equal(
      matchesShortcut(keyEvent({ key: "c", ctrlKey: true, shiftKey: true }), "Shift+Ctrl+C"),
      true,
    );
  });

  it("normalizes space to the canonical Space name", () => {
    assert.equal(matchesShortcut(keyEvent({ key: " " }), "Space"), true);
    assert.equal(matchesShortcut(keyEvent({ key: "Space" }), "Space"), true);
    assert.equal(matchesShortcut(keyEvent({ key: " " }), "Enter"), false);
  });

  it("never matches a modifier-only combo against a modifier press", () => {
    assert.equal(matchesShortcut(keyEvent({ key: "Control", ctrlKey: true }), "Ctrl"), false);
  });
});

describe("dispatchShortcut", () => {
  const keyEvent = (partial) => ({
    key: "",
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    preventDefault: mock.fn(),
    ...partial,
  });

  it("dispatches the matching handler and consumes the event", () => {
    const saved = mock.fn();
    const locked = mock.fn();
    const event = keyEvent({ key: "s", ctrlKey: true });
    dispatchShortcut(event, { save: "Ctrl+S", lock: "Ctrl+L" }, { save: saved, lock: locked });
    assert.equal(saved.mock.callCount(), 1);
    assert.equal(locked.mock.callCount(), 0);
    assert.equal(event.preventDefault.mock.callCount(), 1);
  });

  it("does nothing when no combo matches and leaves the event untouched", () => {
    const handler = mock.fn();
    const event = keyEvent({ key: "x" });
    dispatchShortcut(event, { save: "Ctrl+S" }, { save: handler });
    assert.equal(handler.mock.callCount(), 0);
    assert.equal(event.preventDefault.mock.callCount(), 0);
  });

  it("skips unbound actions (empty combo) without consuming the event", () => {
    const unbound = mock.fn();
    const fallback = mock.fn();
    const event = keyEvent({ key: "s", ctrlKey: true });
    // `save` is unbound; a later binding on the same combo still fires.
    dispatchShortcut(
      event,
      { save: "", "second-save": "Ctrl+S" },
      { save: unbound, "second-save": fallback },
    );
    assert.equal(unbound.mock.callCount(), 0);
    assert.equal(fallback.mock.callCount(), 1);
  });

  it("ignores handlers registered for unknown action ids", () => {
    const event = keyEvent({ key: "s", ctrlKey: true });
    assert.doesNotThrow(() =>
      dispatchShortcut(event, { save: "Ctrl+S" }, { totallyDifferent: mock.fn() }),
    );
  });
});
