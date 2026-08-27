import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  buildBlankMenuItems,
  buildEntryMenuItems,
  buildToolbarMenuItems,
} from "../src/lib/utils/menu-items.ts";

const baseEntry = { username: "u", password: "p", url: "https://x", favorite: false };

describe("buildEntryMenuItems", () => {
  it("switches to multi-select items when several rows are selected", () => {
    const single = buildEntryMenuItems({ entry: baseEntry, selectedCount: 1, isDesktop: true });
    const multi = buildEntryMenuItems({ entry: baseEntry, selectedCount: 3, isDesktop: true });
    const ids = (items) => items.map((item) => item.id);

    assert.ok(!ids(single).includes("edit-selected"));
    assert.ok(ids(multi).includes("edit-selected"));
    assert.ok(ids(multi).includes("delete-selected"));
    // Labels carry the selection count.
    const editSelected = multi.find((item) => item.id === "edit-selected");
    assert.match(editSelected.label, /\(3\)/);
  });

  it("disables desktop-only actions outside Tauri", () => {
    const items = buildEntryMenuItems({ entry: baseEntry, selectedCount: 1, isDesktop: false });
    const disabledIds = items.filter((item) => item.disabled).map((item) => item.id);
    assert.ok(disabledIds.includes("tcato"));
    assert.ok(disabledIds.includes("download-favicon"));
    assert.ok(!disabledIds.includes("copy-password"), "browser demo has demo passwords");
  });

  it("disables copy actions for empty fields", () => {
    const items = buildEntryMenuItems({
      entry: { username: "", password: "", url: "", favorite: false },
      selectedCount: 1,
      isDesktop: true,
    });
    const disabledIds = items.filter((item) => item.disabled).map((item) => item.id);
    assert.ok(disabledIds.includes("copy-username"));
    assert.ok(disabledIds.includes("copy-url"));
  });

  it("labels favorite by current state", () => {
    const fav = buildEntryMenuItems({
      entry: { ...baseEntry, favorite: true },
      selectedCount: 1,
      isDesktop: true,
    }).find((item) => item.id === "favorite");
    assert.equal(fav.label, "取消收藏");
  });
});

describe("buildBlankMenuItems", () => {
  it("disables select-all on an empty list and save when clean/read-only", () => {
    const items = buildBlankMenuItems({ hasVisibleEntries: false, canSave: true });
    const byId = Object.fromEntries(items.map((item) => [item.id, item]));
    assert.equal(byId["select-all"].disabled, true);
    assert.equal(byId.save.disabled, false);

    const noSave = buildBlankMenuItems({ hasVisibleEntries: true, canSave: false });
    const byId2 = Object.fromEntries(noSave.map((item) => [item.id, item]));
    assert.equal(byId2["select-all"].disabled, false);
    assert.equal(byId2.save.disabled, true);
  });

  it("keeps import/export out of the blank menu (they live in the toolbar)", () => {
    const ids = buildBlankMenuItems({ hasVisibleEntries: true, canSave: false }).map(
      (item) => item.id,
    );
    assert.ok(!ids.some((id) => id.startsWith("import-")));
    assert.ok(!ids.some((id) => id.startsWith("export-")));
    assert.ok(!ids.includes("lock"));
  });
});

describe("buildToolbarMenuItems", () => {
  it("reflects detail visibility in the toggle label/icon", () => {
    const shown = buildToolbarMenuItems({ detailVisible: true, busy: false });
    const toggle = shown.find((item) => item.id === "toggle-detail");
    assert.equal(toggle.label, "隐藏详情面板");

    const hidden = buildToolbarMenuItems({ detailVisible: false, busy: false });
    assert.equal(hidden.find((item) => item.id === "toggle-detail").label, "显示详情面板");
  });

  it("disables the security report while busy", () => {
    const items = buildToolbarMenuItems({ detailVisible: true, busy: true });
    assert.equal(items.find((item) => item.id === "security-report").disabled, true);
  });

  it("offers import and export as cascades with every source exactly once", () => {
    const items = buildToolbarMenuItems({ detailVisible: true, busy: false });
    const byId = Object.fromEntries(items.map((item) => [item.id, item]));

    const importIds = byId["import"].children.map((child) => child.id);
    assert.equal(importIds.length, 4);
    assert.deepEqual([...new Set(importIds)], importIds);

    const exportIds = byId["export"].children.map((child) => child.id);
    assert.equal(exportIds.length, 3);
    assert.deepEqual([...new Set(exportIds)], exportIds);
  });
});
