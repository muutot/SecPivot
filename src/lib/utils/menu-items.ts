//! Pure builders for the main window's context menus (entry right-click,
//! blank-area right-click, toolbar overflow). They turn state flags into
//! `ContextMenuItem[]` data — no component, store, or IPC access — so the
//! item/label/disabled matrix is unit-testable and the page only keeps the
//! action dispatchers. Extracted from `+page.svelte`.

import type { ContextMenuItem } from "$lib/components/ContextMenu.svelte";
import type { VaultEntry } from "$lib/types/vault";
import type { Language } from "$lib/types/settings";
import { t } from "$lib/i18n";

/** Options for {@link buildEntryMenuItems}. */
export type EntryMenuInput = {
  entry: Pick<VaultEntry, "username" | "password" | "url" | "favorite">;
  /** Number of currently selected rows (>1 switches to multi-select items). */
  selectedCount: number;
  /** Whether the real desktop backend is available (Tauri runtime). */
  isDesktop: boolean;
  /** UI locale for labels. */
  locale: Language;
};

/** Right-click on an entry row: multi-select actions first when several rows
 *  are selected, then per-entry actions. */
export function buildEntryMenuItems({
  entry,
  selectedCount,
  isDesktop,
  locale,
}: EntryMenuInput): ContextMenuItem[] {
  const multi = selectedCount > 1;
  const items: ContextMenuItem[] = [
    ...(multi
      ? [
          {
            id: "edit-selected",
            label: t(locale, "menu.editSelected", { count: selectedCount }),
            icon: "edit" as const,
          },
          {
            id: "delete-selected",
            label: t(locale, "menu.deleteSelected", { count: selectedCount }),
            icon: "trash" as const,
            destructive: true,
          },
        ]
      : []),
    { id: "edit", label: t(locale, "menu.editEntry"), icon: "edit" },
    {
      id: "copy-username",
      label: t(locale, "menu.copyUsername"),
      icon: "user",
      disabled: !entry.username,
    },
    {
      id: "copy-password",
      label: t(locale, "menu.copyPassword"),
      icon: "copy",
      disabled: !isDesktop && !entry.password,
    },
    { id: "copy-url", label: t(locale, "menu.copyUrl"), icon: "link", disabled: !entry.url },
    { id: "autotype", label: t(locale, "menu.autotype"), icon: "keyboard" },
    { id: "autotype-password", label: t(locale, "menu.autotypePassword"), icon: "key" },
    {
      id: "download-favicon",
      label: multi
        ? t(locale, "menu.downloadFaviconSelected", { count: selectedCount })
        : t(locale, "menu.downloadFavicon"),
      icon: "globe",
      disabled: !isDesktop || (!multi && !entry.url),
    },
    {
      id: "tcato",
      label: t(locale, "menu.tcato"),
      icon: "shield",
      disabled: !isDesktop,
    },
    {
      id: "favorite",
      label: entry.favorite ? t(locale, "menu.unfavorite") : t(locale, "menu.favorite"),
      icon: "star",
    },
    { id: "delete", label: t(locale, "menu.deleteEntry"), icon: "trash", destructive: true },
  ];
  return items;
}

/** Options for {@link buildBlankMenuItems}. */
export type BlankMenuInput = {
  /** Whether any row is visible (select-all needs content). */
  hasVisibleEntries: boolean;
  /** Current vault dirty/read-only flags gate the save action. */
  canSave: boolean;
  /** UI locale for labels. */
  locale: Language;
};

/** Right-click on blank list space: creation and quick database actions.
 *  Import/export and maintenance live in the toolbar ⋯ menu. */
export function buildBlankMenuItems({
  hasVisibleEntries,
  canSave,
  locale,
}: BlankMenuInput): ContextMenuItem[] {
  return [
    { id: "new-entry", label: t(locale, "menu.newEntry"), icon: "plus" },
    { id: "new-group", label: t(locale, "menu.newGroup"), icon: "folder-plus" },
    {
      id: "select-all",
      label: t(locale, "menu.selectAll"),
      icon: "check",
      disabled: !hasVisibleEntries,
    },
    {
      id: "save",
      label: t(locale, "menu.save"),
      icon: "save",
      disabled: !canSave,
    },
    { id: "save-as", label: t(locale, "menu.saveAs"), icon: "copy" },
    { id: "change-timeline", label: t(locale, "menu.timeline"), icon: "undo" },
    { id: "hibp-check", label: t(locale, "menu.hibp"), icon: "globe" },
    { id: "refresh", label: t(locale, "menu.refresh"), icon: "refresh" },
    { id: "db-settings", label: t(locale, "menu.dbSettings"), icon: "settings" },
  ];
}

/** Options for {@link buildToolbarMenuItems}. */
export type ToolbarMenuInput = {
  /** Whether the detail panel is currently shown (drives the toggle label). */
  detailVisible: boolean;
  /** Whether a long operation is running (disables the security report). */
  busy: boolean;
  /** UI locale for labels. */
  locale: Language;
  /** Per-item visibility: when an item is pinned to the toolbar, hide it from the overflow menu. */
  toolbarItems?: {
    saveAs: boolean;
    toggleDetail: boolean;
    securityReport: boolean;
    similarPasswords: boolean;
    hibpCheck: boolean;
    importMenu: boolean;
    exportMenu: boolean;
    expiredEntries: boolean;
    clearHistory: boolean;
    dbSettings: boolean;
    appSettings: boolean;
  };
  /** Optional order for the right toolbar group; overflow items that are still hidden follow this order. */
  toolbarOrder?: string[];
};

/** Import sources as a cascade child list of the toolbar ⋯ menu. */
function importItems(locale: Language): ContextMenuItem[] {
  return [
    { id: "import-csv", label: t(locale, "menu.importCsv"), icon: "upload" },
    { id: "import-xml", label: t(locale, "menu.importXml"), icon: "upload" },
    { id: "import-bitwarden", label: t(locale, "menu.importBitwarden"), icon: "upload" },
    { id: "import-1password", label: t(locale, "menu.import1password"), icon: "upload" },
  ];
}

/** Export targets as a cascade child list of the toolbar ⋯ menu. */
function exportItems(locale: Language): ContextMenuItem[] {
  return [
    { id: "export-csv", label: t(locale, "menu.exportCsv"), icon: "download" },
    { id: "export-xml", label: t(locale, "menu.exportXml"), icon: "download" },
    { id: "export-emergency", label: t(locale, "menu.exportEmergency"), icon: "download" },
  ];
}

/** Toolbar overflow menu (⋯): detail toggle, report, import/export cascades,
 *  maintenance and settings. When `toolbarItems` is provided, items pinned
 *  to the toolbar (true) are omitted from the overflow to avoid duplication. */
export function buildToolbarMenuItems({
  detailVisible,
  busy,
  locale,
  toolbarItems,
  toolbarOrder,
}: ToolbarMenuInput): ContextMenuItem[] {
  const all: ContextMenuItem[] = [
    { id: "save-as", label: t(locale, "menu.saveAs"), icon: "copy" },
    {
      id: "toggle-detail",
      label: detailVisible ? t(locale, "menu.hideDetail") : t(locale, "menu.showDetail"),
      icon: detailVisible ? ("eye-off" as const) : ("eye" as const),
    },
    {
      id: "security-report",
      label: t(locale, "menu.securityReport"),
      icon: "shield",
      disabled: busy,
    },
    { id: "similar-passwords", label: t(locale, "menu.similarPasswords"), icon: "shield" },
    { id: "hibp-check", label: t(locale, "menu.hibp"), icon: "globe" },
    {
      id: "import",
      label: t(locale, "menu.import"),
      icon: "upload",
      children: importItems(locale),
    },
    {
      id: "export",
      label: t(locale, "menu.export"),
      icon: "download",
      children: exportItems(locale),
    },
    { id: "expired-entries", label: t(locale, "menu.expired"), icon: "clock" },
    { id: "clear-history", label: t(locale, "menu.clearHistory"), icon: "trash" },
    { id: "lock", label: t(locale, "menu.lock"), icon: "lock" },
    { id: "db-settings", label: t(locale, "menu.dbSettings"), icon: "settings" },
    { id: "settings", label: t(locale, "menu.settings"), icon: "settings" },
  ];
  if (!toolbarItems) return all;
  const map: Record<string, keyof NonNullable<ToolbarMenuInput["toolbarItems"]>> = {
    "save-as": "saveAs",
    "toggle-detail": "toggleDetail",
    "security-report": "securityReport",
    "similar-passwords": "similarPasswords",
    "hibp-check": "hibpCheck",
    import: "importMenu",
    export: "exportMenu",
    "expired-entries": "expiredEntries",
    "clear-history": "clearHistory",
    "db-settings": "dbSettings",
    settings: "appSettings",
  };
  const filtered = all.filter((item) => {
    const key = map[item.id];
    if (!key) return true;
    return !toolbarItems[key];
  });
  if (!toolbarOrder || toolbarOrder.length === 0) return filtered;
  const orderMap = new Map<string, number>();
  toolbarOrder.forEach((id, idx) => {
    const menuId =
      id === "toggleDetail"
        ? "toggle-detail"
        : id === "securityReport"
          ? "security-report"
          : id === "similarPasswords"
            ? "similar-passwords"
            : id === "hibpCheck"
              ? "hibp-check"
              : id === "importMenu"
                ? "import"
                : id === "exportMenu"
                  ? "export"
                  : id === "expiredEntries"
                    ? "expired-entries"
                    : id === "clearHistory"
                      ? "clear-history"
                      : id === "dbSettings"
                        ? "db-settings"
                        : id === "appSettings"
                          ? "settings"
                          : id;
    orderMap.set(menuId, idx);
  });
  const anchor = new Map<string, number>([
    ["save-as", -1],
    ["lock", 100],
  ]);
  return [...filtered].sort((a, b) => {
    const ai = anchor.has(a.id) ? anchor.get(a.id)! : (orderMap.get(a.id) ?? 50);
    const bi = anchor.has(b.id) ? anchor.get(b.id)! : (orderMap.get(b.id) ?? 50);
    return ai - bi;
  });
}
