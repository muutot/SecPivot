import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { register } from "node:module";

register("./helpers/lib-alias-loader.mjs", import.meta.url);

const { buildBlankMenuItems, buildEntryMenuItems, buildToolbarMenuItems } =
  await import("../src/lib/utils/menu-items.ts");

const baseEntry = { username: "u", password: "p", url: "https://x", favorite: false };
const zh = { locale: "zh-CN" };

describe("buildEntryMenuItems", () => {
  it("switches to multi-select items when several rows are selected", () => {
    const single = buildEntryMenuItems({
      entry: baseEntry,
      selectedCount: 1,
      isDesktop: true,
      ...zh,
    });
    const multi = buildEntryMenuItems({
      entry: baseEntry,
      selectedCount: 3,
      isDesktop: true,
      ...zh,
    });
    const ids = (items) => items.map((item) => item.id);

    assert.ok(!ids(single).includes("edit-selected"));
    assert.ok(ids(multi).includes("edit-selected"));
    assert.ok(ids(multi).includes("delete-selected"));
    // Labels carry the selection count.
    const editSelected = multi.find((item) => item.id === "edit-selected");
    assert.match(editSelected.label, /\(3\)/);
  });

  it("disables desktop-only actions outside Tauri", () => {
    const items = buildEntryMenuItems({
      entry: baseEntry,
      selectedCount: 1,
      isDesktop: false,
      ...zh,
    });
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
      ...zh,
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
      ...zh,
    }).find((item) => item.id === "favorite");
    assert.equal(fav.label, "取消收藏");
  });

  it("renders English labels for the en locale", () => {
    const items = buildEntryMenuItems({
      entry: baseEntry,
      selectedCount: 3,
      isDesktop: true,
      locale: "en",
    });
    const byId = Object.fromEntries(items.map((item) => [item.id, item]));
    assert.equal(byId["edit-selected"].label, "Edit selected entries (3)");
    assert.equal(byId.edit.label, "Edit entry");
    assert.equal(byId.tcato.label, "Fill with TCATO overlay");
  });
});

describe("buildBlankMenuItems", () => {
  it("disables select-all on an empty list and save when clean/read-only", () => {
    const items = buildBlankMenuItems({ hasVisibleEntries: false, canSave: true, ...zh });
    const byId = Object.fromEntries(items.map((item) => [item.id, item]));
    assert.equal(byId["select-all"].disabled, true);
    assert.equal(byId.save.disabled, false);

    const noSave = buildBlankMenuItems({ hasVisibleEntries: true, canSave: false, ...zh });
    const byId2 = Object.fromEntries(noSave.map((item) => [item.id, item]));
    assert.equal(byId2["select-all"].disabled, false);
    assert.equal(byId2.save.disabled, true);
  });

  it("keeps import/export out of the blank menu (they live in the toolbar)", () => {
    const ids = buildBlankMenuItems({ hasVisibleEntries: true, canSave: false, ...zh }).map(
      (item) => item.id,
    );
    assert.ok(!ids.some((id) => id.startsWith("import-")));
    assert.ok(!ids.some((id) => id.startsWith("export-")));
    assert.ok(!ids.includes("lock"));
  });
});

describe("buildToolbarMenuItems", () => {
  it("reflects detail visibility in the toggle label/icon", () => {
    const shown = buildToolbarMenuItems({ detailVisible: true, busy: false, ...zh });
    const toggle = shown.find((item) => item.id === "toggle-detail");
    assert.equal(toggle.label, "隐藏详情面板");

    const hidden = buildToolbarMenuItems({ detailVisible: false, busy: false, ...zh });
    assert.equal(hidden.find((item) => item.id === "toggle-detail").label, "显示详情面板");
  });

  it("disables the security report while busy", () => {
    const items = buildToolbarMenuItems({ detailVisible: true, busy: true, ...zh });
    assert.equal(items.find((item) => item.id === "security-report").disabled, true);
  });

  it("offers import and export as cascades with every source exactly once", () => {
    const items = buildToolbarMenuItems({ detailVisible: true, busy: false, ...zh });
    const byId = Object.fromEntries(items.map((item) => [item.id, item]));

    const importIds = byId["import"].children.map((child) => child.id);
    assert.equal(importIds.length, 4);
    assert.deepEqual([...new Set(importIds)], importIds);

    const exportIds = byId["export"].children.map((child) => child.id);
    assert.equal(exportIds.length, 3);
    assert.deepEqual([...new Set(exportIds)], exportIds);
  });

  it("renders English labels for the en locale", () => {
    const items = buildToolbarMenuItems({ detailVisible: true, busy: false, locale: "en" });
    const byId = Object.fromEntries(items.map((item) => [item.id, item]));
    assert.equal(byId["toggle-detail"].label, "Hide detail pane");
    assert.equal(byId.lock.label, "Lock database");
    assert.equal(
      byId["import"].children.find((child) => child.id === "import-1password").label,
      "Import 1Password (1PIF)",
    );
  });
});
