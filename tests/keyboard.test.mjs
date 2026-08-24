import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { effectiveShortcuts, matchesShortcut } from "../src/lib/services/keyboard.ts";

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
