//! Pure builders for the main window's context menus (entry right-click,
//! blank-area right-click, toolbar overflow). They turn state flags into
//! `ContextMenuItem[]` data — no component, store, or IPC access — so the
//! item/label/disabled matrix is unit-testable and the page only keeps the
//! action dispatchers. Extracted from `+page.svelte`.

import type { ContextMenuItem } from "$lib/components/ContextMenu.svelte";
import type { VaultEntry } from "$lib/types/vault";

/** Options for {@link buildEntryMenuItems}. */
export type EntryMenuInput = {
  entry: Pick<VaultEntry, "username" | "password" | "url" | "favorite">;
  /** Number of currently selected rows (>1 switches to multi-select items). */
  selectedCount: number;
  /** Whether the real desktop backend is available (Tauri runtime). */
  isDesktop: boolean;
};

/** Right-click on an entry row: multi-select actions first when several rows
 *  are selected, then per-entry actions. */
export function buildEntryMenuItems({
  entry,
  selectedCount,
  isDesktop,
}: EntryMenuInput): ContextMenuItem[] {
  const multi = selectedCount > 1;
  const items: ContextMenuItem[] = [
    ...(multi
      ? [
          {
            id: "edit-selected",
            label: `编辑所选条目 (${selectedCount})`,
            icon: "edit" as const,
          },
          {
            id: "delete-selected",
            label: `删除所选条目 (${selectedCount})`,
            icon: "trash" as const,
            destructive: true,
          },
        ]
      : []),
    { id: "edit", label: "编辑条目", icon: "edit" },
    { id: "copy-username", label: "复制用户名", icon: "user", disabled: !entry.username },
    {
      id: "copy-password",
      label: "复制密码",
      icon: "copy",
      disabled: !isDesktop && !entry.password,
    },
    { id: "copy-url", label: "复制网址", icon: "link", disabled: !entry.url },
    { id: "autotype", label: "自动填充", icon: "keyboard" },
    { id: "autotype-password", label: "自动填充密码", icon: "key" },
    {
      id: "download-favicon",
      label: multi ? `下载所选条目图标 (${selectedCount})` : "下载网址图标",
      icon: "globe",
      disabled: !isDesktop || (!multi && !entry.url),
    },
    {
      id: "tcato",
      label: "TCATO 覆盖层填充",
      icon: "shield",
      disabled: !isDesktop,
    },
    { id: "favorite", label: entry.favorite ? "取消收藏" : "收藏条目", icon: "star" },
    { id: "delete", label: "删除条目", icon: "trash", destructive: true },
  ];
  return items;
}

/** Options for {@link buildBlankMenuItems}. */
export type BlankMenuInput = {
  /** Whether any row is visible (select-all needs content). */
  hasVisibleEntries: boolean;
  /** Current vault dirty/read-only flags gate the save action. */
  canSave: boolean;
};

/** Right-click on blank list space: creation, import, maintenance and export
 *  actions over the whole database. */
export function buildBlankMenuItems({
  hasVisibleEntries,
  canSave,
}: BlankMenuInput): ContextMenuItem[] {
  return [
    { id: "new-entry", label: "新建条目", icon: "plus" },
    { id: "new-group", label: "新建分组", icon: "folder-plus" },
    { id: "import-csv", label: "导入 CSV", icon: "upload" },
    { id: "import-xml", label: "导入 XML", icon: "upload" },
    { id: "import-bitwarden", label: "导入 Bitwarden", icon: "upload" },
    { id: "import-1password", label: "导入 1Password (1PIF)", icon: "upload" },
    { id: "select-all", label: "全选条目", icon: "check", disabled: !hasVisibleEntries },
    {
      id: "save",
      label: "保存数据库",
      icon: "save",
      disabled: !canSave,
    },
    { id: "save-as", label: "另存为…", icon: "copy" },
    { id: "similar-passwords", label: "相似密码检查", icon: "shield" },
    { id: "expired-entries", label: "过期条目", icon: "clock" },
    { id: "change-timeline", label: "变更时间线", icon: "undo" },
    { id: "hibp-check", label: "HIBP 泄露检查", icon: "globe" },
    { id: "clear-history", label: "清理全部历史", icon: "trash" },
    { id: "lock", label: "锁定数据库", icon: "lock" },
    { id: "refresh", label: "刷新", icon: "refresh" },
    { id: "export-csv", label: "导出 CSV", icon: "download" },
    { id: "export-xml", label: "导出 KeePass XML", icon: "download" },
    { id: "export-emergency", label: "导出 HTML 应急表", icon: "download" },
    { id: "db-settings", label: "数据库设置", icon: "settings" },
  ];
}

/** Options for {@link buildToolbarMenuItems}. */
export type ToolbarMenuInput = {
  /** Whether the detail panel is currently shown (drives the toggle label). */
  detailVisible: boolean;
  /** Whether a long operation is running (disables the security report). */
  busy: boolean;
};

/** Toolbar overflow menu (⋯): report/export/settings shortcuts plus the
 *  detail-panel visibility toggle. */
export function buildToolbarMenuItems({
  detailVisible,
  busy,
}: ToolbarMenuInput): ContextMenuItem[] {
  return [
    { id: "save-as", label: "另存为…", icon: "copy" },
    {
      id: "toggle-detail",
      label: detailVisible ? "隐藏详情面板" : "显示详情面板",
      icon: detailVisible ? ("eye-off" as const) : ("eye" as const),
    },
    { id: "security-report", label: "安全报告", icon: "shield", disabled: busy },
    { id: "similar-passwords", label: "相似密码检查", icon: "shield" },
    { id: "hibp-check", label: "HIBP 泄露检查", icon: "globe" },
    { id: "export-csv", label: "导出 CSV", icon: "download" },
    { id: "export-xml", label: "导出 KeePass XML", icon: "download" },
    { id: "export-emergency", label: "导出 HTML 应急表", icon: "download" },
    { id: "db-settings", label: "数据库设置", icon: "settings" },
    { id: "settings", label: "设置", icon: "settings" },
  ];
}
