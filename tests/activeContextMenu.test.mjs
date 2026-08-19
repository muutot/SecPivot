import { test } from "node:test";
import assert from "node:assert/strict";
import {
  activeContextMenu,
  closeContextMenu,
  openContextMenu,
} from "../src/lib/stores/activeContextMenu.svelte.ts";

test("opening a second context menu replaces the active owner", () => {
  let owner = null;
  const unsub = activeContextMenu.subscribe((value) => (owner = value));
  try {
    openContextMenu("group");
    assert.equal(owner, "group");
    openContextMenu("page");
    assert.equal(owner, "page", "a newly opened menu must become the sole active owner");
  } finally {
    unsub();
  }
});

test("a stale close from a non-active owner must not clear the active menu", () => {
  let owner = null;
  const unsub = activeContextMenu.subscribe((value) => (owner = value));
  try {
    openContextMenu("group");
    openContextMenu("page");
    closeContextMenu("group");
    assert.equal(owner, "page", "closing the stale owner must leave the active one open");
    closeContextMenu("page");
    assert.equal(owner, null);
  } finally {
    unsub();
  }
});

test("closeContextMenu() without an owner clears unconditionally", () => {
  let owner = null;
  const unsub = activeContextMenu.subscribe((value) => (owner = value));
  try {
    openContextMenu("page");
    closeContextMenu();
    assert.equal(owner, null);
  } finally {
    unsub();
  }
});
