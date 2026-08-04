<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { get } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { vault } from "$lib/services/vault";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import type { EntryColumnState } from "$lib/types/settings";
  import ColumnConfigMenu, {
    type ColumnMenuSection,
  } from "$lib/components/ColumnConfigMenu.svelte";
  import { effectiveShortcuts } from "$lib/services/keyboard";
  import { syncCompactShellClass } from "$lib/services/settings-bootstrap";
  import { armIdleLock, lockVault, copyValue } from "$lib/services/security";
  import type {
    EntryInput,
    EntryPatch,
    VaultEntry,
    VaultGroup,
    VaultState,
    SecurityReport,
    FaviconProgress,
  } from "$lib/types/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import type { IconName } from "$lib/components/AppIcon.svelte";
  import {
    KEEPASS_ICON_CHOICES,
    keepassIconName,
    keepassGroupIconName,
  } from "$lib/utils/keepass-icons";
  import ContextMenu, { type ContextMenuItem } from "$lib/components/ContextMenu.svelte";
  import VaultWelcome from "$lib/components/VaultWelcome.svelte";
  import LockScreen from "$lib/components/LockScreen.svelte";
  import GroupTree from "$lib/components/GroupTree.svelte";
  import EntryDetail from "$lib/components/EntryDetail.svelte";
  import EntryEditorDialog from "$lib/components/EntryEditorDialog.svelte";
  import SecurityReportDialog from "$lib/components/SecurityReportDialog.svelte";
  import EntryTotpBadge from "$lib/components/EntryTotpBadge.svelte";
  import TcatoOverlay from "$lib/components/TcatoOverlay.svelte";
  import WindowControls from "$lib/components/WindowControls.svelte";
  import { buildCsv, parseCsv, parseCsvRows } from "$lib/utils/csv";
  import { parseKdbxXml } from "$lib/utils/kdbx-xml";
  import { formatDateOnly } from "$lib/utils/date";
  import { collectGroups, collectEntries, findEntry } from "$lib/utils/tree";

  /** The TCATO overlay window loads the same SPA with a `#/tcato` hash. */
  const isTcatoOverlay =
    typeof window !== "undefined" && window.location.hash.startsWith("#/tcato");

  let currentVault = $state<VaultState | null>(null);
  let rememberedPath = $state<{ path: string; fileName: string } | null>(null);
  let search = $state("");
  let selectedGroup = $state<string | null>(null);
  let selectedEntry = $state<VaultEntry | null>(null);
  let selectedUuids = $state<Set<string>>(new Set());
  let selectionAnchor = $state<string | null>(null);
  let editorOpen = $state(false);
  let editorMode: "create" | "edit" | "edit-multi" = $state("create");
  let editEntry: VaultEntry | null = $state(null);
  let editEntries: VaultEntry[] = $state([]);
  let groupModalOpen = $state(false);
  let groupModalParent = $state<string | null>(null);
  let newGroupName = $state("");
  let groupIconIndex = $state<number | null>(null);
  let confirmState = $state<{ message: string; onconfirm: () => void } | null>(null);
  let statusMsg = $state("");
  let busy = $state(false);
  let reportOpen = $state(false);
  let securityReport = $state<SecurityReport | null>(null);
  let faviconDialog = $state<{
    phase: "working" | "done";
    progress: FaviconProgress;
    result: string;
    error: boolean;
  } | null>(null);

  let statusTimer: ReturnType<typeof setTimeout> | undefined = $state();
  let expiredNotifiedPath = $state<string | null>(null);

  function countExpiredEntries(group: VaultGroup): number {
    return collectEntries(group).filter((e) => e.expired).length;
  }

  function entryIconName(entry: VaultEntry): IconName {
    return keepassIconName(entry.icon);
  }

  /** Data URL of the entry's database-stored custom icon (favicon), if any. */
  function customIconUrl(entry: VaultEntry): string | undefined {
    return entry.customIcon ? currentVault?.customIcons?.[entry.customIcon] : undefined;
  }

  function groupIconName(index: number): IconName {
    return keepassGroupIconName(index);
  }

  onMount(() => {
    // The TCATO overlay window loads this SPA with a `#/tcato` hash; it must
    // not run any of the main-window side effects (vault subscription, idle
    // auto-lock, window-size tracking) or it could lock the real session or
    // resize the fixed-size overlay.
    if (isTcatoOverlay) return;
    const unsubscribe = vault.subscribe((value) => {
      currentVault = value;
      if (value && value.path !== expiredNotifiedPath) {
        expiredNotifiedPath = value.path;
        const expired = countExpiredEntries(value.root);
        if (expired > 0) {
          setTimeout(() => flash(`有 ${expired} 个条目已过期,请及时更新密码`), 300);
        }
      }
      if (!value) {
        selectedEntry = null;
        selectedUuids = new Set();
        selectionAnchor = null;
        editorOpen = false;
        editEntries = [];
      } else {
        selectedEntry = findEntryByUuid(value, selectedEntry?.uuid ?? null);
      }
      armIdleLock();
    });
    const unsubRemembered = vault.remembered((value) => {
      rememberedPath = value;
    });
    // A browser extension write (AddLogin/UpdateLogin) lands straight into the
    // vault in memory; refresh so the entry list shows it without a reopen.
    let unlistenVaultChanged: UnlistenFn | undefined;
    if (isTauriRuntime()) {
      void listen("rpc-vault-changed", () => void vault.refresh()).then(
        (stop) => (unlistenVaultChanged = stop),
      );
    }
    void vault.refresh();
    const rememberWindowSize = (): void => {
      if (!currentVault) return;
      if (windowResizeTimer) clearTimeout(windowResizeTimer);
      windowResizeTimer = setTimeout(() => {
        appSettings.updateGeneral("windowWidth", window.innerWidth);
        appSettings.updateGeneral("windowHeight", window.innerHeight);
      }, 300);
    };
    window.addEventListener("resize", rememberWindowSize);
    return () => {
      unsubscribe();
      unsubRemembered();
      void unlistenVaultChanged?.();
      window.removeEventListener("resize", rememberWindowSize);
      if (windowResizeTimer) clearTimeout(windowResizeTimer);
    };
  });

  function flash(message: string): void {
    statusMsg = message;
    if (statusTimer) clearTimeout(statusTimer);
    statusTimer = setTimeout(() => {
      statusMsg = "";
      statusTimer = undefined;
    }, 1800);
  }

  /** Reactive mirror of the settings store. `$derived(get(appSettings))` would
   * evaluate once and freeze (get() is untracked in Svelte 5); subscribing to
   * a $state mirror keeps every derived below fresh. */
  let settings = $state(get(appSettings));
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      settings = value;
    });
    return unsubscribe;
  });

  const compactMode = $derived(settings.general.compactMode);
  const groupDensity = $derived(settings.general.density);
  const iconOnlyButtons = $derived(settings.general.iconOnlyButtons);
  const showDescriptions = $derived(settings.general.showDescriptions);
  const showLockScreen = $derived(
    !currentVault && rememberedPath !== null && settings.general.rememberLastDatabase,
  );
  $effect(() => {
    syncCompactShellClass(compactMode);
  });

  const WELCOME_WINDOW_SIZE = { width: 620, height: 480 };
  let lastAppliedSize = $state("");
  let windowResizeTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() => {
    const g = get(appSettings).general;
    const view = currentVault === null ? (showLockScreen ? "lock" : "welcome") : "main";
    if (!isTauriRuntime() || isTcatoOverlay || view === "lock") return;
    const width = view === "welcome" ? WELCOME_WINDOW_SIZE.width : g.windowWidth;
    const height = view === "welcome" ? WELCOME_WINDOW_SIZE.height : g.windowHeight;
    const key = `${width}x${height}`;
    if (lastAppliedSize === key) return;
    lastAppliedSize = key;
    void getCurrentWindow().setSize(new LogicalSize(width, height));
  });

  const allGroups = $derived.by((): VaultGroup[] => {
    if (!currentVault) return [];
    return collectGroups(currentVault.root);
  });

  const allEntries = $derived.by((): VaultEntry[] => {
    if (!currentVault) return [];
    const list: VaultEntry[] = [];
    for (const group of allGroups) list.push(...group.entries);
    return list;
  });

  const reportEntries = $derived.by(() => {
    const rows: { entry: VaultEntry; path: string }[] = [];
    for (const group of allGroups) {
      for (const entry of group.entries) {
        rows.push({ entry, path: pathOf(entry.groupUuid) });
      }
    }
    return rows;
  });

  const parentMap = $derived.by((): Map<string, VaultGroup> => {
    const map = new Map<string, VaultGroup>();
    for (const group of allGroups) {
      for (const child of group.children) map.set(child.uuid, group);
    }
    return map;
  });

  function pathOf(groupUuid: string): string {
    const parts: string[] = [];
    let current = allGroups.find((g) => g.uuid === groupUuid);
    let guard = 0;
    while (current && current.uuid !== currentVault?.root.uuid && guard < 50) {
      parts.unshift(current.name);
      current = parentMap.get(current.uuid);
      guard++;
    }
    return parts.join(" / ");
  }

  /** Whether the given group uuid is the recycle bin or nested inside it. */
  function groupInBin(groupUuid: string): boolean {
    let current = allGroups.find((g) => g.uuid === groupUuid);
    let guard = 0;
    while (current && current.uuid !== currentVault?.root.uuid && guard < 50) {
      if (current.isRecycleBin) return true;
      current = parentMap.get(current.uuid);
      guard++;
    }
    return false;
  }

  function selectedGroupInBin(uuid: string): boolean {
    return groupInBin(uuid);
  }

  function entryInBin(entryUuid: string): boolean {
    const entry = allEntries.find((e) => e.uuid === entryUuid);
    if (!entry) return false;
    return groupInBin(entry.groupUuid);
  }

  const selectedSubtree = $derived.by((): VaultGroup[] => {
    if (!currentVault) return [];
    if (selectedGroup === null) return allGroups.filter((g) => !groupInBin(g.uuid));
    const group = allGroups.find((g) => g.uuid === selectedGroup);
    if (!group) return allGroups;
    return collectGroups(group);
  });

  const filteredEntries = $derived.by((): { entry: VaultEntry }[] => {
    if (!currentVault) return [];
    const query = search.trim().toLowerCase();
    const result: { entry: VaultEntry }[] = [];
    for (const group of selectedSubtree) {
      for (const entry of group.entries) {
        const text = [entry.title, entry.username, entry.url, entry.notes].join(" ").toLowerCase();
        if (query && !text.includes(query)) continue;
        result.push({ entry });
      }
    }
    result.sort((a, b) => Number(b.entry.favorite) - Number(a.entry.favorite));
    return result;
  });

  type SortCol = string;
  let sortCol = $state<SortCol>("title");
  let sortDir = $state<"asc" | "desc">("asc");
  /** Resizable column width bounds — match config.rs entry-column clamps. */
  const COL_WIDTH_MIN = 30;
  const COL_WIDTH_MAX = 400;
  /** Title column width when `width` is `0` (auto sentinel, see settings.ts). */
  const COL_TITLE_DEFAULT = 200;
  /** Built-in entry-table columns, in default display order. */
  const BUILTIN_COLUMNS: { id: string; label: string; sortable?: boolean }[] = [
    { id: "title", label: "标题" },
    { id: "username", label: "用户名" },
    { id: "password", label: "密码", sortable: false },
    { id: "url", label: "网址" },
    { id: "totp", label: "验证码", sortable: false },
    { id: "notes", label: "备注" },
    { id: "tags", label: "标签" },
    { id: "created", label: "创建时间" },
    { id: "modified", label: "修改时间" },
    { id: "expires", label: "过期时间" },
  ];
  /** Custom-field column names present in the vault, most frequent first. */
  const customColumnNames = $derived.by(() => {
    if (!currentVault) return [];
    const names = new Map<string, number>();
    for (const e of collectEntries(currentVault.root)) {
      for (const f of e.customFields ?? []) {
        names.set(f.name, (names.get(f.name) ?? 0) + 1);
      }
    }
    return [...names.entries()].sort((a, b) => b[1] - a[1]).map(([name]) => name);
  });
  let entryColumns = $state<EntryColumnState[]>(
    get(appSettings).general.entryColumns.map((c) => ({ ...c })),
  );
  $effect(() => {
    entryColumns = get(appSettings).general.entryColumns.map((c) => ({ ...c }));
  });
  /** Persisted state of a column id (built-in or `custom:<name>`). */
  function colState(id: string): EntryColumnState {
    return (
      entryColumns.find((c) => c.id === id) ?? {
        id,
        visible: false,
        width: id === "title" ? 0 : 140,
      }
    );
  }
  /** Visible columns in render order (icon/actions columns excluded). */
  const visibleCols = $derived.by(() => {
    const out: { id: string; label: string; width: number; sortable: boolean }[] = [];
    for (const def of BUILTIN_COLUMNS) {
      const st = colState(def.id);
      if (st.visible) {
        out.push({
          id: def.id,
          label: def.label,
          width: st.width,
          sortable: def.sortable !== false,
        });
      }
    }
    for (const name of customColumnNames) {
      const id = `custom:${name}`;
      if (colState(id).visible) {
        out.push({ id, label: name, width: colState(id).width, sortable: true });
      }
    }
    return out;
  });
  /** CSS grid template for the entry table (icon + visible columns + actions). */
  const entryGridCols = $derived.by(() => {
    const cols = ["34px"];
    for (const c of visibleCols) {
      const w = c.id === "title" && c.width <= 0 ? COL_TITLE_DEFAULT : c.width;
      cols.push(`${w}px`);
    }
    cols.push("70px");
    return cols.join(" ");
  });
  /** Sort key of an entry for a column id (password is never sortable). */
  function sortValue(entry: VaultEntry, colId: string): string {
    if (colId === "totp") return entry.hasTotp ? "1" : "0";
    return colText(entry, colId);
  }
  /** Display text of an entry cell for a column id. */
  function colText(entry: VaultEntry, colId: string): string {
    if (colId.startsWith("custom:")) {
      const name = colId.slice("custom:".length);
      return entry.customFields?.find((f) => f.name === name)?.value ?? "";
    }
    if (colId === "created" || colId === "modified" || colId === "expires") {
      return formatDateOnly(entry[colId] as string | undefined);
    }
    if (colId === "password") return entry.password ? "••••••" : "";
    return String(entry[colId as keyof VaultEntry] ?? "");
  }
  let groupWidth = $state(get(appSettings).general.panelWidths.group);
  let detailWidth = $state(get(appSettings).general.panelWidths.detail);
  let detailVisible = $state(false);

  $effect(() => {
    const p = settings.general.panelWidths;
    groupWidth = p.group;
    detailWidth = p.detail;
  });

  $effect(() => {
    if (selectedEntry) {
      detailVisible = true;
    } else {
      detailVisible = false;
    }
  });

  const sortedEntries = $derived.by(() => {
    const dir = sortDir === "asc" ? 1 : -1;
    const col = sortCol;
    return [...filteredEntries].sort((a, b) => {
      const fav = Number(b.entry.favorite) - Number(a.entry.favorite);
      if (fav !== 0) return fav;
      const av = sortValue(a.entry, col);
      const bv = sortValue(b.entry, col);
      return av.localeCompare(bv, "zh-CN", { numeric: true }) * dir;
    });
  });

  function cycleSort(col: SortCol): void {
    if (sortCol === col) {
      sortDir = sortDir === "asc" ? "desc" : "asc";
    } else {
      sortCol = col;
      sortDir = "asc";
    }
  }

  function startColResize(e: PointerEvent, colId: string): void {
    e.preventDefault();
    e.stopPropagation();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const startX = e.clientX;
    const cellEl = target.parentElement;
    const renderW = cellEl ? Math.round(cellEl.getBoundingClientRect().width) : 0;
    const storedW = colState(colId).width;
    const startW = storedW > 0 ? storedW : Math.max(renderW, COL_WIDTH_MIN);
    document.body.classList.add("resizing-column");
    const onMove = (ev: PointerEvent): void => {
      const width = Math.min(
        COL_WIDTH_MAX,
        Math.max(COL_WIDTH_MIN, startW + (ev.clientX - startX)),
      );
      entryColumns = entryColumns.map((c) => (c.id === colId ? { ...c, width } : c));
    };
    const onUp = (ev: PointerEvent): void => {
      if (target.hasPointerCapture(ev.pointerId)) target.releasePointerCapture(ev.pointerId);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      document.body.classList.remove("resizing-column");
      saveLayout();
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  /** Right-click a table column header → column config menu. */
  let columnMenu = $state<{ x: number; y: number } | null>(null);
  const columnMenuSections = $derived.by(() => {
    const sections: ColumnMenuSection[] = [
      {
        label: "内置条目",
        items: BUILTIN_COLUMNS.map((def) => ({
          id: def.id,
          label: def.label,
          visible: colState(def.id).visible,
        })),
      },
    ];
    if (customColumnNames.length > 0) {
      sections.push({
        label: "自定义条目",
        items: customColumnNames.map((name) => {
          const id = `custom:${name}`;
          return { id, label: name, visible: colState(id).visible };
        }),
      });
    }
    return sections;
  });
  function openColumnMenu(e: MouseEvent): void {
    e.preventDefault();
    columnMenu = { x: e.clientX, y: e.clientY };
  }
  function toggleColumn(id: string): void {
    const existing = entryColumns.find((c) => c.id === id);
    if (existing) {
      entryColumns = entryColumns.map((c) => (c.id === id ? { ...c, visible: !c.visible } : c));
    } else {
      entryColumns = [...entryColumns, { id, visible: true, width: id === "title" ? 0 : 140 }];
    }
    appSettings.updateGeneral(
      "entryColumns",
      entryColumns.map((c) => ({ ...c })),
    );
  }

  function startDetailResize(e: PointerEvent): void {
    e.preventDefault();
    e.stopPropagation();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const startX = e.clientX;
    const startW = detailWidth;
    document.body.classList.add("resizing-column");
    const onMove = (ev: PointerEvent): void => {
      detailWidth = Math.min(640, Math.max(260, startW - (ev.clientX - startX)));
    };
    const onUp = (ev: PointerEvent): void => {
      if (target.hasPointerCapture(ev.pointerId)) target.releasePointerCapture(ev.pointerId);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      document.body.classList.remove("resizing-column");
      saveLayout();
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function startGroupResize(e: PointerEvent): void {
    e.preventDefault();
    e.stopPropagation();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const startX = e.clientX;
    const startW = groupWidth;
    document.body.classList.add("resizing-column");
    const onMove = (ev: PointerEvent): void => {
      groupWidth = Math.min(320, Math.max(140, startW + (ev.clientX - startX)));
    };
    const onUp = (ev: PointerEvent): void => {
      if (target.hasPointerCapture(ev.pointerId)) target.releasePointerCapture(ev.pointerId);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      document.body.classList.remove("resizing-column");
      saveLayout();
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function saveLayout(): void {
    appSettings.updateGeneral("panelWidths", {
      group: groupWidth,
      detail: detailWidth,
      urlCol: colState("url").width || 200,
    });
    appSettings.updateGeneral(
      "entryColumns",
      entryColumns.map((c) => ({ ...c })),
    );
  }

  function findEntryByUuid(state: VaultState | null, uuid: string | null): VaultEntry | null {
    if (!state || !uuid) return null;
    return findEntry(state.root, uuid);
  }

  function setSingleSelection(entry: VaultEntry | null): void {
    selectedUuids = entry ? new Set([entry.uuid]) : new Set();
    selectionAnchor = entry?.uuid ?? null;
    selectedEntry = entry;
  }

  function handleRowClick(event: MouseEvent, entry: VaultEntry): void {
    if (event.shiftKey && selectionAnchor) {
      const uuids = sortedEntries.map((r) => r.entry.uuid);
      const start = uuids.indexOf(selectionAnchor);
      const end = uuids.indexOf(entry.uuid);
      if (start !== -1 && end !== -1) {
        const [lo, hi] = start <= end ? [start, end] : [end, start];
        selectedUuids = new Set(uuids.slice(lo, hi + 1));
        selectionAnchor = entry.uuid;
        selectedEntry = entry;
        return;
      }
    }
    if (event.ctrlKey || event.metaKey) {
      const next = new Set(selectedUuids);
      if (next.has(entry.uuid)) {
        next.delete(entry.uuid);
      } else {
        next.add(entry.uuid);
      }
      selectedUuids = next;
      selectionAnchor = entry.uuid;
      selectedEntry = entry;
      return;
    }
    setSingleSelection(entry);
  }

  async function toggleFavorite(entry: VaultEntry): Promise<void> {
    try {
      const saved = await vault.toggleFavorite(entry.uuid);
      if (selectedEntry?.uuid === entry.uuid) {
        selectedEntry = findEntryByUuid(saved, entry.uuid);
      }
    } catch (e) {
      flash(`收藏失败：${e}`);
    }
  }

  async function handleSave(): Promise<void> {
    if (!currentVault || !currentVault.dirty) {
      flash("没有需要保存的修改");
      return;
    }
    busy = true;
    try {
      const saved = await vault.save();
      selectedEntry = findEntryByUuid(saved, selectedEntry?.uuid ?? null);
      flash("已保存到数据库");
    } catch (e) {
      flash(`保存失败：${e}`);
    } finally {
      busy = false;
    }
  }

  /** Save As: pick a new local path, persist there and switch to it. */
  async function handleSaveAs(): Promise<void> {
    if (!currentVault) return;
    try {
      if (isTauriRuntime()) {
        const ext = get(appSettings).database.fileExtension;
        const baseName = (currentVault.fileName.replace(/\.\w+$/i, "") || "keyvault") + "." + ext;
        const selected = await save({
          defaultPath: baseName,
          filters: [{ name: "KeePass 数据库", extensions: [ext] }],
        });
        if (!selected) return;
        await vault.saveAs(String(selected));
      } else {
        flash("浏览器预览不支持另存为");
        return;
      }
      flash("已另存为数据库");
    } catch (e) {
      flash(`另存为失败：${e}`);
    }
  }

  async function handleExportCsv(): Promise<void> {
    if (!currentVault) return;
    try {
      if (isTauriRuntime()) {
        const selected = await save({
          defaultPath: (currentVault.fileName.replace(/\.kdbx$/i, "") || "keyvault") + ".csv",
          filters: [{ name: "CSV 文件", extensions: ["csv"] }],
        });
        if (!selected) return;
        await invoke("export_csv", { path: String(selected) });
      } else {
        const rows = reportEntries.map(({ entry, path }) => ({
          group: path,
          title: entry.title,
          username: entry.username,
          password: entry.password ?? "",
          url: entry.url,
          notes: entry.notes,
          totp: entry.totp ?? "",
          favorite: entry.favorite === true,
        }));
        const csv = buildCsv(rows);
        const blob = new Blob([csv], { type: "text/csv;charset=utf-8" });
        const url = URL.createObjectURL(blob);
        const anchor = document.createElement("a");
        anchor.href = url;
        anchor.download = "keyvault-export.csv";
        anchor.click();
        URL.revokeObjectURL(url);
      }
      flash("已导出 CSV");
    } catch (e) {
      flash(`导出失败：${e}`);
    }
  }

  async function handleOpenReport(): Promise<void> {
    if (reportOpen || busy || !currentVault) return;
    busy = true;
    try {
      securityReport = await vault.securityReport();
      reportOpen = true;
    } catch (e) {
      flash(`安全分析失败：${e}`);
    } finally {
      busy = false;
    }
  }

  async function handleDownloadFavicons(): Promise<void> {
    await runFaviconDownload(undefined, "没有可下载的网址图标");
  }

  /** Download icons for the selected entries only (context menu, multi-select aware). */
  async function downloadSelectedFavicons(entry: VaultEntry): Promise<void> {
    const uuids = selectedUuids.size > 1 ? Array.from(selectedUuids) : [entry.uuid];
    await runFaviconDownload(uuids, "所选条目没有可下载的网址图标");
  }

  async function runFaviconDownload(
    uuids: string[] | undefined,
    noneMessage: string,
  ): Promise<void> {
    if (busy) return;
    if (!isTauriRuntime()) {
      flash("浏览器预览不支持下载图标");
      return;
    }
    busy = true;
    faviconDialog = {
      phase: "working",
      progress: { done: 0, total: 0 },
      result: "正在连接站点…",
      error: false,
    };
    const unlisten = await listen<FaviconProgress>("favicon-progress", (e) => {
      faviconDialog = {
        phase: "working",
        progress: e.payload,
        result: `正在下载，已完成 ${e.payload.done}/${e.payload.total}`,
        error: false,
      };
    });
    try {
      const report = await vault.downloadFavicons(uuids);
      faviconDialog = {
        phase: "done",
        progress: { done: report.attempted, total: report.attempted },
        result:
          report.attempted === 0
            ? noneMessage
            : `已下载 ${report.downloaded}/${report.attempted} 个网址图标`,
        error: false,
      };
    } catch (e) {
      faviconDialog = {
        phase: "done",
        progress: { done: 0, total: 0 },
        result: `图标下载失败：${e}`,
        error: true,
      };
    } finally {
      unlisten();
      busy = false;
    }
  }

  let progressPct = $derived(
    faviconDialog && faviconDialog.progress.total > 0
      ? `${Math.round((faviconDialog.progress.done / faviconDialog.progress.total) * 100)}%`
      : "0%",
  );

  async function copyEntryPassword(entry: VaultEntry): Promise<void> {
    try {
      const password = await vault.getEntryPassword(entry.uuid);
      await copyEntryValue(password, "密码", true);
    } catch {
      flash("复制失败");
    }
  }

  function readPickedFile(): Promise<string | null> {
    return new Promise((resolve) => {
      const input = document.createElement("input");
      input.type = "file";
      input.accept = ".csv,text/csv,.xml,text/xml";
      input.onchange = () => {
        const file = input.files?.[0];
        if (!file) {
          resolve(null);
          return;
        }
        const reader = new FileReader();
        reader.onload = () => resolve(String(reader.result ?? ""));
        reader.onerror = () => resolve(null);
        reader.readAsText(file);
      };
      input.click();
    });
  }

  /** Walk a "A / B" group path, creating missing subgroups, and return the leaf uuid. */
  async function resolveImportGroup(path: string, startState: VaultState): Promise<string> {
    const parts = path
      .split("/")
      .map((p) => p.trim())
      .filter(Boolean);
    let state = startState;
    let parentUuid: string | null = selectedGroup ?? null;
    for (const name of parts) {
      const existing = collectGroups(state.root).find(
        (g) => g.parentUuid === parentUuid && g.name === name,
      );
      if (existing) {
        parentUuid = existing.uuid;
      } else {
        state = await vault.addGroup({ parentUuid, name });
        const created = collectGroups(state.root).find(
          (g) => g.parentUuid === parentUuid && g.name === name,
        );
        if (!created) throw new Error("创建分组失败");
        parentUuid = created.uuid;
      }
    }
    return parentUuid ?? state.root.uuid;
  }

  async function handleImportCsv(): Promise<void> {
    if (!currentVault) return;
    let text: string;
    try {
      if (isTauriRuntime()) {
        const selected = await open({
          multiple: false,
          filters: [{ name: "CSV 文件", extensions: ["csv"] }],
        });
        if (!selected) return;
        text = await invoke<string>("read_text_file", { path: String(selected) });
      } else {
        const picked = await readPickedFile();
        if (picked === null) return;
        text = picked;
      }
    } catch (e) {
      flash(`读取文件失败：${e}`);
      return;
    }
    const rows = parseCsvRows(parseCsv(text));
    if (rows.length === 0) {
      flash("CSV 中没有可导入的条目");
      return;
    }
    busy = true;
    try {
      let state = currentVault;
      for (const row of rows) {
        const groupUuid = await resolveImportGroup(row.group, state);
        state = await vault.addEntry({
          groupUuid,
          title: row.title,
          username: row.username,
          password: row.password,
          url: row.url,
          notes: row.notes,
          totp: row.totp || undefined,
          customFields: [],
          attachments: [],
        });
      }
      flash(`已导入 ${rows.length} 个条目`);
    } catch (e) {
      flash(`导入失败：${e}`);
    } finally {
      busy = false;
    }
  }

  async function handleImportXml(): Promise<void> {
    if (!currentVault) return;
    let text: string;
    try {
      if (isTauriRuntime()) {
        const selected = await open({
          multiple: false,
          filters: [
            { name: "KeePass XML 文件", extensions: ["xml"] },
            { name: "CSV 文件", extensions: ["csv"] },
          ],
        });
        if (!selected) return;
        text = await invoke<string>("read_text_file", { path: String(selected) });
      } else {
        const picked = await readPickedFile();
        if (picked === null) return;
        text = picked;
      }
    } catch (e) {
      flash(`读取文件失败：${e}`);
      return;
    }
    const entries = parseKdbxXml(text);
    if (entries.length === 0) {
      flash("XML 中没有可导入的条目");
      return;
    }
    busy = true;
    try {
      let state = currentVault;
      for (const entry of entries) {
        const groupUuid = await resolveImportGroup(entry.group, state);
        state = await vault.addEntry({
          groupUuid,
          title: entry.title,
          username: entry.username,
          password: entry.password,
          url: entry.url,
          notes: entry.notes,
          totp: entry.totp || undefined,
          customFields: entry.customFields,
          attachments: [],
        });
      }
      flash(`已导入 ${entries.length} 个条目`);
    } catch (e) {
      flash(`导入失败：${e}`);
    } finally {
      busy = false;
    }
  }

  function openSettings(): void {
    void goto("/settings");
  }

  async function handleLock(): Promise<void> {
    await lockVault();
    flash("数据库已锁定");
  }

  function openCreateEntry(): void {
    editorMode = "create";
    editEntry = null;
    editEntries = [];
    editorOpen = true;
  }

  let searchInputEl = $state<HTMLInputElement | null>(null);

  /** True when the event's pressed modifiers match `combo` ("Ctrl+Shift+C"). */
  function matchesShortcut(event: KeyboardEvent, combo: string): boolean {
    const mods: [string, boolean][] = [
      ["Ctrl", event.ctrlKey],
      ["Alt", event.altKey],
      ["Shift", event.shiftKey],
      ["Meta", event.metaKey],
    ];
    const parts = combo.split("+").map((p) => p.trim());
    let keyPart = "";
    for (const part of parts) {
      if (part === "Ctrl" || part === "Alt" || part === "Shift" || part === "Meta") continue;
      keyPart = part;
    }
    for (const [name, pressed] of mods) {
      if (parts.includes(name) !== pressed) return false;
    }
    if (!keyPart) return false;
    const eventKey =
      event.key === " " ? "Space" : event.key.length === 1 ? event.key.toUpperCase() : event.key;
    return eventKey === keyPart;
  }

  /** Dispatch recorded app shortcuts; skipped while typing or modals are open. */
  function handleShortcutKeydown(event: KeyboardEvent): void {
    if (isTcatoOverlay || !currentVault) return;
    if (editorOpen || groupModalOpen || reportOpen || confirmState || entryMenu || blankMenu)
      return;
    const target = event.target as HTMLElement | null;
    if (
      target &&
      (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
    ) {
      return;
    }
    const shortcuts = effectiveShortcuts(get(appSettings).keyboard.shortcuts);
    for (const [actionId, combo] of Object.entries(shortcuts)) {
      if (!combo || !matchesShortcut(event, combo)) continue;
      event.preventDefault();
      switch (actionId) {
        case "save":
          void handleSave();
          break;
        case "lock":
          void handleLock();
          break;
        case "edit":
          if (selectedEntry) openEditEntry(selectedEntry);
          break;
        case "copy-password":
          if (selectedEntry) void copyEntryPassword(selectedEntry);
          break;
        case "new-entry":
          openCreateEntry();
          break;
        case "focus-search":
          searchInputEl?.focus();
          break;
      }
      return;
    }
  }

  /** Collect the fully-populated entries behind the current selection. */
  function selectedEntries(): VaultEntry[] {
    if (!currentVault) return [];
    return collectEntries(currentVault.root).filter((e) => selectedUuids.has(e.uuid));
  }

  function openEditEntry(entry: VaultEntry): void {
    if (selectedUuids.size > 1 && selectedUuids.has(entry.uuid)) {
      editorMode = "edit-multi";
      editEntry = null;
      editEntries = selectedEntries();
      if (editEntries.length < 2) {
        setSingleSelection(entry);
        editorMode = "edit";
        editEntry = entry;
        editEntries = [];
      }
      editorOpen = true;
      return;
    }
    editorMode = "edit";
    editEntry = entry;
    editEntries = [];
    editorOpen = true;
  }

  async function handleEditorSave(
    input: EntryInput | null,
    patch: EntryPatch | null,
  ): Promise<void> {
    try {
      if (editorMode === "create" && input) {
        const state = await vault.addEntry(input);
        setSingleSelection(findEntryByUuid(state, null));
        editorOpen = false;
        flash("已创建条目");
      } else if (editorMode === "edit-multi" && patch && editEntries.length > 0) {
        const uuids = editEntries.map((e) => e.uuid);
        const state = await vault.updateEntries(uuids, patch);
        selectedEntry = findEntryByUuid(state, selectedEntry?.uuid ?? null);
        editorOpen = false;
        flash(`已更新 ${uuids.length} 个条目`);
      } else if (editorMode === "edit" && input && editEntry) {
        const state = await vault.updateEntry(editEntry.uuid, input);
        setSingleSelection(findEntryByUuid(state, editEntry.uuid));
        editorOpen = false;
        flash("已保存修改");
      }
    } catch (e) {
      flash(`操作失败：${e}`);
    }
  }

  function openGroupModal(parentUuid: string | null): void {
    groupModalParent = parentUuid;
    newGroupName = "";
    groupIconIndex = null;
    groupModalOpen = true;
  }

  async function confirmCreateGroup(): Promise<void> {
    const name = newGroupName.trim();
    if (!name) return;
    try {
      await vault.addGroup({
        parentUuid: groupModalParent,
        name,
        icon: groupIconIndex ?? undefined,
      });
      groupModalOpen = false;
      flash("已创建分组");
    } catch (e) {
      flash(`创建失败：${e}`);
    }
  }

  async function renameGroup(uuid: string, name: string): Promise<void> {
    try {
      await vault.renameGroup(uuid, name);
      flash("已重命名分组");
    } catch (e) {
      flash(`重命名失败：${e}`);
    }
  }

  function askDeleteGroup(uuid: string): void {
    const inBin = selectedGroupInBin(uuid);
    confirmState = {
      message: inBin
        ? "永久删除该分组及其全部内容？此操作无法撤销。"
        : "删除该分组？其下条目将移动到回收站。",
      onconfirm: async () => {
        try {
          await vault.deleteGroup(uuid);
          if (selectedGroup === uuid) selectedGroup = null;
          flash(inBin ? "已永久删除分组" : "已移入回收站");
        } catch (e) {
          flash(`删除失败：${e}`);
        }
      },
    };
  }

  function askEmptyRecycleBin(): void {
    confirmState = {
      message: "清空回收站？其中的条目和分组将被永久删除，此操作无法撤销。",
      onconfirm: async () => {
        try {
          await vault.emptyRecycleBin();
          flash("已清空回收站");
        } catch (e) {
          flash(`清空失败：${e}`);
        }
      },
    };
  }

  async function restoreGroup(uuid: string): Promise<void> {
    try {
      await vault.restoreGroup(uuid);
      flash("已恢复分组");
    } catch (e) {
      flash(`恢复失败：${e}`);
    }
  }

  async function restoreEntry(entry: VaultEntry): Promise<void> {
    try {
      await vault.restoreEntry(entry.uuid);
      if (selectedEntry?.uuid === entry.uuid) selectedEntry = null;
      flash("已恢复条目");
    } catch (e) {
      flash(`恢复失败：${e}`);
    }
  }

  function askDeleteEntry(entry: VaultEntry): void {
    const inBin = entryInBin(entry.uuid);
    confirmState = {
      message: inBin
        ? `永久删除条目「${entry.title || "未命名"}」？此操作无法撤销。`
        : `删除条目「${entry.title || "未命名"}」？可从回收站恢复。`,
      onconfirm: async () => {
        try {
          await vault.deleteEntry(entry.uuid);
          if (selectedEntry?.uuid === entry.uuid) selectedEntry = null;
          if (selectedUuids.has(entry.uuid)) {
            const next = new Set(selectedUuids);
            next.delete(entry.uuid);
            selectedUuids = next;
          }
          flash(inBin ? "已永久删除条目" : "已移入回收站");
        } catch (e) {
          flash(`删除失败：${e}`);
        }
      },
    };
  }

  function askDeleteEntries(): void {
    const uuids = Array.from(selectedUuids);
    if (uuids.length === 0) return;
    const allInBin = uuids.every((uuid) => entryInBin(uuid));
    confirmState = {
      message: allInBin
        ? `永久删除所选 ${uuids.length} 个条目？此操作无法撤销。`
        : `删除所选 ${uuids.length} 个条目？可从回收站恢复。`,
      onconfirm: async () => {
        try {
          await vault.deleteEntries(uuids);
          selectedUuids = new Set();
          selectedEntry = null;
          flash(allInBin ? "已永久删除所选条目" : "所选条目已移入回收站");
        } catch (e) {
          flash(`删除失败：${e}`);
        }
      },
    };
  }

  async function moveEntriesTo(groupUuid: string, uuids: string[]): Promise<void> {
    if (!currentVault || uuids.length === 0) return;
    try {
      for (const uuid of uuids) {
        await vault.moveEntry(uuid, groupUuid);
      }
      flash(`已移动 ${uuids.length} 个条目`);
    } catch (e) {
      flash(`移动失败：${e}`);
    }
  }

  async function copyEntryValue(value: string, label: string, sensitive = false): Promise<void> {
    try {
      await copyValue(value, sensitive);
      flash(`已复制${label}`);
    } catch {
      flash("复制失败");
    }
  }

  let entryMenu = $state<{ x: number; y: number; entry: VaultEntry } | null>(null);
  let blankMenu = $state<{ x: number; y: number } | null>(null);

  function openEntryMenu(event: MouseEvent, entry: VaultEntry): void {
    event.preventDefault();
    event.stopPropagation();
    blankMenu = null;
    if (!selectedUuids.has(entry.uuid)) setSingleSelection(entry);
    selectedEntry = entry;
    entryMenu = { x: event.clientX, y: event.clientY, entry };
  }

  function openBlankMenu(event: MouseEvent): void {
    event.preventDefault();
    entryMenu = null;
    blankMenu = { x: event.clientX, y: event.clientY };
  }

  function selectAllEntries(): void {
    selectedUuids = new Set(sortedEntries.map((r) => r.entry.uuid));
    selectionAnchor = null;
    selectedEntry = sortedEntries[0]?.entry ?? null;
  }

  function clearSelection(): void {
    selectedUuids = new Set();
    selectionAnchor = null;
    selectedEntry = null;
  }

  function entryMenuItems(entry: VaultEntry): ContextMenuItem[] {
    const multi = selectedUuids.size > 1;
    const items: ContextMenuItem[] = [
      ...(multi
        ? [
            {
              id: "edit-selected",
              label: `编辑所选条目 (${selectedUuids.size})`,
              icon: "edit" as const,
            },
            {
              id: "delete-selected",
              label: `删除所选条目 (${selectedUuids.size})`,
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
        disabled: !isTauriRuntime() && !entry.password,
      },
      { id: "copy-url", label: "复制网址", icon: "link", disabled: !entry.url },
      { id: "autotype", label: "自动填充", icon: "keyboard" },
      { id: "autotype-password", label: "自动填充密码", icon: "key" },
      {
        id: "download-favicon",
        label: multi ? `下载所选条目图标 (${selectedUuids.size})` : "下载网址图标",
        icon: "globe",
        disabled: !isTauriRuntime() || (!multi && !entry.url),
      },
      {
        id: "tcato",
        label: "TCATO 覆盖层填充",
        icon: "shield",
        disabled: !isTauriRuntime(),
      },
      { id: "favorite", label: entry.favorite ? "取消收藏" : "收藏条目", icon: "star" },
      { id: "delete", label: "删除条目", icon: "trash", destructive: true },
    ];
    return items;
  }

  const blankMenuItems = $derived<ContextMenuItem[]>([
    { id: "new-entry", label: "新建条目", icon: "plus" },
    { id: "new-group", label: "新建分组", icon: "folder-plus" },
    { id: "import-csv", label: "导入 CSV", icon: "upload" },
    { id: "import-xml", label: "导入 XML", icon: "upload" },
    { id: "select-all", label: "全选条目", icon: "check", disabled: sortedEntries.length === 0 },
    { id: "save", label: "保存数据库", icon: "save", disabled: !currentVault?.dirty },
    { id: "save-as", label: "另存为…", icon: "copy" },
    { id: "lock", label: "锁定数据库", icon: "lock" },
    { id: "refresh", label: "刷新", icon: "refresh" },
  ]);

  function handleEntryMenuAction(id: string, entry: VaultEntry): void {
    if (id === "edit" || id === "edit-selected") openEditEntry(entry);
    else if (id === "copy-username" && entry.username)
      void copyEntryValue(entry.username, "用户名");
    else if (id === "copy-password") void copyEntryPassword(entry);
    else if (id === "copy-url" && entry.url) void copyEntryValue(entry.url, "网址");
    else if (id === "autotype") void runAutoType(entry);
    else if (id === "autotype-password") void runAutoType(entry, AUTOTYPE_PASSWORD_SEQUENCE);
    else if (id === "tcato") void openTcatoOverlay(entry);
    else if (id === "download-favicon") void downloadSelectedFavicons(entry);
    else if (id === "favorite") void toggleFavorite(entry);
    else if (id === "delete") askDeleteEntry(entry);
    else if (id === "delete-selected") askDeleteEntries();
  }

  /** KeePass-standard default auto-type sequence; no trailing enter, so the
   * user confirms the form (captcha, 2FA, etc.) before submitting. */
  const AUTOTYPE_SEQUENCE = "{USERNAME}{TAB}{PASSWORD}";
  /** Password-only variant: fills just the password, no submit. */
  const AUTOTYPE_PASSWORD_SEQUENCE = "{PASSWORD}";

  async function runAutoType(entry: VaultEntry, sequence = AUTOTYPE_SEQUENCE): Promise<void> {
    try {
      await vault.autoType(entry.uuid, sequence);
      flash("已最小化，请在 1.5 秒内切换到目标窗口");
    } catch (e) {
      flash(`自动填充失败：${e}`);
    }
  }

  /** Open the always-on-top two-channel overlay for manual channel injection. */
  async function openTcatoOverlay(entry: VaultEntry): Promise<void> {
    try {
      await invoke("open_tcato_overlay", { uuid: entry.uuid });
    } catch (e) {
      flash(`TCATO 覆盖层打开失败：${e}`);
    }
  }

  function handleBlankMenuAction(id: string): void {
    if (id === "new-entry") openCreateEntry();
    else if (id === "new-group") openGroupModal(selectedGroup);
    else if (id === "import-csv") void handleImportCsv();
    else if (id === "import-xml") void handleImportXml();
    else if (id === "select-all") selectAllEntries();
    else if (id === "save") void handleSave();
    else if (id === "save-as") void handleSaveAs();
    else if (id === "lock") void handleLock();
    else if (id === "refresh") void vault.refresh();
  }
</script>

<svelte:head>
  <title>KeyVault</title>
</svelte:head>

<svelte:window onkeydowncapture={handleShortcutKeydown} />

{#if isTcatoOverlay}
  <TcatoOverlay />
{:else}
  <main
    class="app-shell"
    class:compact={compactMode}
    class:standalone={!currentVault}
    style:--group-gap={compactMode ? `${groupDensity.groupGap}px` : undefined}
    style:--group-pad-y={compactMode ? `${groupDensity.groupPaddingY}px` : undefined}
    style:--group-indent={compactMode ? `${groupDensity.groupIndent}px` : undefined}
    style:--group-radius={compactMode ? `${groupDensity.groupRadius}px` : undefined}
  >
    {#if currentVault}
      <div class="toolbar" role="presentation" data-tauri-drag-region>
        <div class="toolbar-left">
          <button
            class="tool-button primary"
            class:icon-only={iconOnlyButtons}
            onclick={openCreateEntry}
            title="新建条目 (Ctrl+N)"
          >
            <AppIcon name="plus" size={14} />
            {#if !iconOnlyButtons}<span class="btn-label">条目</span>{/if}
          </button>
          <button
            class="tool-button"
            class:icon-only={iconOnlyButtons}
            onclick={() => openGroupModal(selectedGroup)}
            title="新建分组"
          >
            <AppIcon name="folder-plus" size={14} />
            {#if !iconOnlyButtons}<span class="btn-label">分组</span>{/if}
          </button>
          <button
            class="tool-button"
            class:icon-only={iconOnlyButtons}
            onclick={handleSave}
            disabled={busy || !currentVault.dirty}
            title="保存数据库 (Ctrl+S)"
          >
            <AppIcon name="save" size={14} />
            {#if !iconOnlyButtons}<span class="btn-label">保存</span>{/if}
          </button>
          <button
            class="tool-button"
            class:icon-only={iconOnlyButtons}
            onclick={() => void handleSaveAs()}
            title="另存为数据库副本到新路径"
          >
            <AppIcon name="copy" size={14} />
            {#if !iconOnlyButtons}<span class="btn-label">另存为</span>{/if}
          </button>
          <span class="toolbar-divider" aria-hidden="true"></span>
          <button
            class="tool-button"
            class:icon-only={iconOnlyButtons}
            onclick={handleLock}
            title="锁定数据库"
          >
            <AppIcon name="lock" size={14} />
            {#if !iconOnlyButtons}<span class="btn-label">锁定</span>{/if}
          </button>
        </div>

        <div class="toolbar-center">
          <div class="search-box">
            <span class="search-icon"><AppIcon name="search" size={13} /></span>
            <input
              class="search-input"
              type="search"
              placeholder="搜索…"
              bind:value={search}
              bind:this={searchInputEl}
              aria-label="搜索条目"
            />
            {#if search}
              <button class="clear-button" onclick={() => (search = "")} aria-label="清除搜索"
                >×</button
              >
            {/if}
          </div>
        </div>

        <div class="toolbar-right">
          {#if currentVault.dirty}
            <span class="dirty-badge">未保存</span>
          {/if}
          <button
            class="icon-action"
            onclick={() => (detailVisible = !detailVisible)}
            title={detailVisible ? "隐藏详情面板" : "显示详情面板"}
            aria-pressed={detailVisible}
          >
            <AppIcon name={detailVisible ? "chevron-right" : "chevron-left"} size={15} />
          </button>
          <button class="icon-action" onclick={() => void handleOpenReport()} title="安全报告">
            <AppIcon name="shield" size={15} />
          </button>
          <button class="icon-action" onclick={() => void handleExportCsv()} title="导出 CSV">
            <AppIcon name="download" size={15} />
          </button>
          <button class="icon-action" onclick={openSettings} title="设置">
            <AppIcon name="settings" size={16} />
          </button>
          <span class="toolbar-divider" aria-hidden="true"></span>
          <WindowControls variant="toolbar" />
        </div>
      </div>

      <div
        class="main-content"
        style={`--group-width: ${groupWidth}px; --detail-width: ${detailVisible ? detailWidth : 0}px`}
      >
        <section class="group-panel">
          <GroupTree
            root={currentVault.root}
            selected={selectedGroup}
            customIcons={currentVault.customIcons}
            showIcon={compactMode ? groupDensity.showGroupIcon : true}
            showChevron={compactMode ? groupDensity.showGroupChevron : true}
            onselect={(uuid: string | null) => {
              selectedGroup = uuid;
              selectedEntry = null;
              selectedUuids = new Set();
              selectionAnchor = null;
            }}
            onaddsubgroup={openGroupModal}
            onrename={(uuid: string, name: string) => void renameGroup(uuid, name)}
            ondelete={askDeleteGroup}
            onrestore={(uuid: string) => void restoreGroup(uuid)}
            onemptybin={askEmptyRecycleBin}
            ondropentry={(groupUuid: string, uuids: string[]) =>
              void moveEntriesTo(groupUuid, uuids)}
          />
        </section>

        <span
          class="group-resize-handle"
          role="separator"
          aria-orientation="vertical"
          title="调整分组宽度"
          onpointerdown={startGroupResize}
        ></span>

        <section class="entry-panel">
          <div class="entry-table" style={`--entry-cols: ${entryGridCols}`}>
            <div class="entry-table-head" role="row" tabindex="-1" oncontextmenu={openColumnMenu}>
              <span class="head-icon-col" aria-hidden="true"></span>
              {#each visibleCols as col (col.id)}
                <div class="head-cell" class:head-sortable={col.sortable}>
                  <button
                    class="head-button"
                    type="button"
                    onclick={() => {
                      if (col.sortable) cycleSort(col.id);
                    }}
                    title={col.sortable ? "点击排序" : undefined}
                  >
                    <span class="head-label">{col.label}</span>
                    {#if sortCol === col.id}
                      <span class="sort-arrow" aria-hidden="true"
                        >{sortDir === "asc" ? "▲" : "▼"}</span
                      >
                    {/if}
                  </button>
                  <span
                    class="resize-handle"
                    role="separator"
                    aria-orientation="vertical"
                    title="调整列宽"
                    onpointerdown={(e) => startColResize(e, col.id)}
                  ></span>
                </div>
              {/each}
              <div class="head-actions"></div>
            </div>

            <div
              class="entry-list"
              role="listbox"
              aria-label="条目列表"
              aria-multiselectable="true"
              tabindex="-1"
              oncontextmenu={openBlankMenu}
              onkeydown={(e) => {
                if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") {
                  e.preventDefault();
                  selectAllEntries();
                }
              }}
            >
              {#if filteredEntries.length === 0}
                <div class="empty-state">
                  <span class="empty-icon"><AppIcon name="key" size={20} /></span>
                  <strong>{search ? "没有匹配的条目" : "这个分组还没有条目"}</strong>
                  <p>{search ? "尝试调整搜索关键词" : "点击右上角「条目」新建一条"}</p>
                </div>
              {:else}
                {#each sortedEntries as row (row.entry.uuid)}
                  <div
                    class="entry-row"
                    class:selected={selectedUuids.has(row.entry.uuid)}
                    class:expired-row={row.entry.expired}
                    style:--row-color={row.entry.color ?? "transparent"}
                    role="option"
                    aria-selected={selectedUuids.has(row.entry.uuid)}
                    tabindex="0"
                    draggable="true"
                    ondragstart={(e) => {
                      const targets = selectedUuids.has(row.entry.uuid)
                        ? Array.from(selectedUuids)
                        : [row.entry.uuid];
                      e.dataTransfer?.setData(
                        "application/x-keyvault-entries",
                        JSON.stringify(targets),
                      );
                      if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
                    }}
                    onclick={(e) => handleRowClick(e, row.entry)}
                    oncontextmenu={(e) => openEntryMenu(e, row.entry)}
                    onkeydown={(e) => {
                      if (e.key === "Enter") setSingleSelection(row.entry);
                    }}
                  >
                    {#if row.entry.color}
                      <span class="entry-row-color-bar" aria-hidden="true"></span>
                    {/if}
                    <span class="entry-row-icon-cell">
                      <span class="entry-row-icon"
                        >{#if customIconUrl(row.entry)}
                          <img
                            class="entry-row-img"
                            src={customIconUrl(row.entry)}
                            alt=""
                            draggable="false"
                          />
                        {:else}
                          <AppIcon name={entryIconName(row.entry)} size={16} />
                        {/if}</span
                      >
                    </span>
                    {#each visibleCols as col (col.id)}
                      {#if col.id === "title"}
                        <span class="entry-row-col col-title">
                          <div class="entry-row-main">
                            <span
                              class="entry-row-title"
                              title={row.entry.expired ? "已过期" : undefined}
                              >{row.entry.title || "未命名条目"}{#if row.entry.expired}
                                <span class="expired-flag">已过期</span>
                              {/if}</span
                            >
                            {#if showDescriptions}
                              <span class="entry-row-sub">{row.entry.username}</span>
                            {/if}
                          </div>
                        </span>
                      {:else if col.id === "totp"}
                        <span class="entry-row-col col-totp">
                          {#if row.entry.hasTotp}
                            <EntryTotpBadge entryUuid={row.entry.uuid} />
                          {/if}
                        </span>
                      {:else}
                        <span
                          class="entry-row-col"
                          class:col-masked={col.id === "password"}
                          title={colText(row.entry, col.id) || undefined}
                        >
                          <span class="entry-row-col-text">{colText(row.entry, col.id)}</span>
                        </span>
                      {/if}
                    {/each}
                    <div class="entry-row-actions">
                      <button
                        class="row-btn"
                        class:star-active={row.entry.favorite}
                        title={row.entry.favorite ? "取消收藏" : "收藏条目"}
                        onclick={(e) => {
                          e.stopPropagation();
                          void toggleFavorite(row.entry);
                        }}
                      >
                        <AppIcon name="star" size={12} />
                      </button>
                      <button
                        class="row-btn"
                        title="复制用户名"
                        onclick={(e) => {
                          e.stopPropagation();
                          if (row.entry.username) void copyEntryValue(row.entry.username, "用户名");
                        }}
                      >
                        <AppIcon name="user" size={12} />
                      </button>
                      <button
                        class="row-btn"
                        title="复制密码"
                        onclick={(e) => {
                          e.stopPropagation();
                          void copyEntryPassword(row.entry);
                        }}
                      >
                        <AppIcon name="copy" size={12} />
                      </button>
                    </div>
                  </div>
                {/each}
              {/if}
            </div>
          </div>
        </section>

        {#if detailVisible}
          <span
            class="detail-resize-handle"
            role="separator"
            aria-orientation="vertical"
            title="调整详情宽度"
            onpointerdown={startDetailResize}
          ></span>

          <section class="detail-panel">
            {#if selectedEntry}
              <EntryDetail
                entry={selectedEntry}
                groupPath={pathOf(selectedEntry.groupUuid)}
                inRecycleBin={groupInBin(selectedEntry.groupUuid)}
                onfavorite={toggleFavorite}
                onedit={openEditEntry}
                ondelete={askDeleteEntry}
                onrestore={(entry: VaultEntry) => void restoreEntry(entry)}
              />
            {:else}
              <div class="detail-empty">
                <AppIcon name="eye" size={22} />
                <p>选择条目查看详情</p>
              </div>
            {/if}
          </section>
        {/if}
      </div>

      <footer class="status-bar" role="status" aria-live="polite" data-tauri-drag-region>
        <span class="status-left">
          <span class="result-count">{filteredEntries.length} 个条目</span>
          {#if selectedUuids.size > 1}
            <span class="status-group-filter">已选 {selectedUuids.size} 个</span>
          {/if}
          {#if selectedGroup !== null}
            <span class="status-group-filter" title={pathOf(selectedGroup)}>
              筛选于 {pathOf(selectedGroup)}
            </span>
          {/if}
          {#if currentVault.dirty}
            <span class="status-dirty"><i></i>未保存的修改</span>
          {/if}
        </span>
        <span class="status-msg">{statusMsg}</span>
        <span class="status-right">
          {#if currentVault.path}
            <span class="status-path" title={currentVault.path}>{currentVault.fileName}</span>
          {/if}
        </span>
      </footer>
    {:else if showLockScreen}
      <div class="standalone-bar" data-tauri-drag-region>
        <WindowControls variant="chrome" />
      </div>
      <LockScreen
        remembered={rememberedPath}
        onopened={() => void vault.refresh()}
        onswitch={() => vault.clearRemembered()}
      />
    {:else}
      <div class="standalone-bar" data-tauri-drag-region>
        <WindowControls variant="chrome" />
      </div>
      <VaultWelcome onopened={() => void vault.refresh()} />
    {/if}
  </main>
{/if}

{#if editorOpen}
  <EntryEditorDialog
    mode={editorMode}
    groups={currentVault ? [currentVault.root] : []}
    groupUuid={selectedGroup ?? currentVault?.root.uuid ?? "root"}
    entry={editEntry}
    entries={editEntries}
    onclose={() => (editorOpen = false)}
    onsaved={(input, patch) => void handleEditorSave(input, patch)}
  />
{/if}

{#if reportOpen && securityReport}
  <SecurityReportDialog
    report={securityReport}
    entries={reportEntries}
    onclose={() => (reportOpen = false)}
  />
{/if}

{#if columnMenu}
  <ColumnConfigMenu
    x={columnMenu.x}
    y={columnMenu.y}
    sections={columnMenuSections}
    onclose={() => (columnMenu = null)}
    ontoggle={toggleColumn}
  />
{/if}

{#if groupModalOpen}
  <div class="modal-backdrop" role="presentation">
    <div class="group-modal" role="dialog" aria-modal="true" aria-label="新建分组">
      <div class="modal-head">
        <span class="modal-icon"><AppIcon name="folder-plus" size={18} /></span>
        <div>
          <strong>新建分组</strong>
          <p>在{groupModalParent ? pathOf(groupModalParent) : "根"}下创建子分组</p>
        </div>
      </div>
      <input
        class="text-input"
        type="text"
        bind:value={newGroupName}
        placeholder="分组名称"
        onkeydown={(e) => {
          if (e.key === "Enter") void confirmCreateGroup();
          if (e.key === "Escape") groupModalOpen = false;
        }}
      />
      <span class="group-icon-label">图标</span>
      <div class="group-icon-grid">
        {#each KEEPASS_ICON_CHOICES as index}
          <button
            type="button"
            class="icon-option"
            class:selected={groupIconIndex === index}
            onclick={() => (groupIconIndex = groupIconIndex === index ? null : index)}
            title={`内置图标 ${index}`}
            aria-pressed={groupIconIndex === index}
          >
            <AppIcon name={groupIconName(index)} size={16} />
          </button>
        {/each}
      </div>
      <div class="modal-actions">
        <button class="modal-button" onclick={() => (groupModalOpen = false)}>取消</button>
        <button
          class="modal-button primary"
          onclick={() => void confirmCreateGroup()}
          disabled={!newGroupName.trim()}
        >
          创建
        </button>
      </div>
    </div>
  </div>
{/if}

{#if confirmState}
  <div class="modal-backdrop" role="presentation">
    <div class="group-modal confirm-modal" role="dialog" aria-modal="true" aria-label="确认操作">
      <div class="modal-head">
        <span class="modal-icon danger"><AppIcon name="trash" size={18} /></span>
        <div>
          <strong>确认删除</strong>
          <p>{confirmState.message}</p>
        </div>
      </div>
      <div class="modal-actions">
        <button class="modal-button" onclick={() => (confirmState = null)}>取消</button>
        <button
          class="modal-button danger"
          onclick={() => {
            const state = confirmState;
            if (!state) return;
            const action = state.onconfirm;
            confirmState = null;
            action();
          }}
        >
          删除
        </button>
      </div>
    </div>
  </div>
{/if}

{#if entryMenu}
  <ContextMenu
    x={entryMenu.x}
    y={entryMenu.y}
    items={entryMenuItems(entryMenu.entry)}
    onclose={() => (entryMenu = null)}
    onaction={(id) => {
      const menuEntry = entryMenu!.entry;
      entryMenu = null;
      handleEntryMenuAction(id, menuEntry);
    }}
  />
{/if}

{#if blankMenu}
  <ContextMenu
    x={blankMenu.x}
    y={blankMenu.y}
    items={blankMenuItems}
    onclose={() => (blankMenu = null)}
    onaction={(id) => {
      blankMenu = null;
      handleBlankMenuAction(id);
    }}
  />
{/if}

{#if faviconDialog}
  <div class="modal-backdrop" role="presentation">
    <div class="group-modal">
      <div class="modal-head">
        <span class="modal-icon" class:danger={faviconDialog.error}>
          <AppIcon name={faviconDialog.error ? "x" : "globe"} size={16} />
        </span>
        <div>
          <strong>{faviconDialog.error ? "下载图标失败" : "下载网址图标"}</strong>
          <p>{faviconDialog.result}</p>
        </div>
      </div>
      {#if faviconDialog.phase === "working"}
        <div class="progress-track">
          <div
            class="progress-fill"
            class:indeterminate={faviconDialog.progress.total === 0}
            style:--progress-pct={progressPct}
          ></div>
        </div>
      {:else}
        <div class="modal-actions">
          <button class="modal-button primary" onclick={() => (faviconDialog = null)}>关闭</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .app-shell {
    position: relative;
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    width: 100%;
    height: 100vh;
    min-width: 760px;
    border: 1px solid var(--border-color);
    background: color-mix(in srgb, var(--bg-settings) 98.5%, transparent);
  }

  .app-shell.compact {
    min-width: 680px;
  }

  /* Welcome / lock views render in the smaller standalone window. */
  .app-shell.standalone {
    min-width: 0;
  }

  .standalone-bar {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 30px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 0 10px;
    z-index: 20;
  }

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 14px;
    border-top: 1px solid var(--border-subtle);
    border-bottom: 1px solid var(--border-subtle);
  }

  .toolbar-divider {
    width: 1px;
    height: 18px;
    flex: 0 0 auto;
    background: var(--border-subtle);
  }

  .toolbar-left,
  .toolbar-center,
  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .toolbar-center {
    flex: 1;
    justify-content: center;
  }

  .search-box {
    display: flex;
    align-items: center;
    gap: 6px;
    width: min(340px, 100%);
    height: 28px;
    padding: 0 9px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
  }

  .search-box:focus-within {
    border-color: var(--selection-color);
  }

  .search-icon {
    display: inline-flex;
    color: var(--text-faint);
  }

  .search-input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    outline: none;
    color: var(--text-primary);
    background: transparent;
    font-size: var(--font-size-secondary, 11px);
  }

  .search-input::placeholder {
    color: var(--placeholder-color);
  }

  .search-input::-webkit-search-cancel-button {
    display: none;
  }

  .clear-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    color: var(--text-muted);
    background: transparent;
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
  }

  .clear-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .icon-action {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--card-bg);
    cursor: pointer;
  }

  .icon-action:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .tool-button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--card-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .tool-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .tool-button.primary {
    border-color: color-mix(in srgb, var(--selection-color) 45%, transparent);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 16%, var(--card-bg));
  }

  .tool-button.primary:hover {
    background: color-mix(in srgb, var(--selection-color) 24%, var(--card-bg));
  }

  .tool-button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .tool-button.icon-only {
    justify-content: center;
    width: 28px;
    padding: 0;
  }

  .dirty-badge {
    padding: 2px 7px;
    border: 1px solid color-mix(in srgb, var(--warning-color) 45%, transparent);
    border-radius: 10px;
    color: var(--warning-color);
    font-size: var(--font-size-tiny, 10px);
  }

  .main-content {
    display: grid;
    grid-template-columns: var(--group-width, 200px) minmax(0, 1fr);
    min-height: 0;
    position: relative;
  }

  .group-panel {
    min-height: 0;
    overflow: hidden;
    border-right: 1px solid var(--border-subtle);
    background: var(--surface-bg);
  }

  .entry-panel {
    display: flex;
    flex-direction: column;
    min-height: 0;
    min-width: 0;
  }

  .entry-table {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
    overflow-x: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .entry-table-head {
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: var(--entry-cols);
    align-items: center;
    gap: 0;
    flex: 0 0 auto;
    height: 28px;
    padding: 0;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--surface-bg);
  }

  .head-icon-col {
    height: 100%;
    border-right: 1px solid var(--border-subtle);
  }

  .head-cell {
    position: relative;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    height: 100%;
    padding: 0 8px;
    border-right: 1px solid var(--border-subtle);
    color: var(--text-secondary);
    font-size: var(--font-size-tiny, 10px);
    font-weight: 600;
    cursor: pointer;
    user-select: none;
  }

  .head-cell:not(.head-sortable) {
    cursor: default;
  }

  .head-cell:hover {
    color: var(--text-primary);
  }

  .head-button {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    height: 100%;
    padding: 0;
    border: 0;
    color: inherit;
    background: transparent;
    font: inherit;
    cursor: pointer;
  }

  .head-button:hover {
    color: var(--text-primary);
  }

  .head-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sort-arrow {
    flex: 0 0 auto;
    color: var(--selection-color);
    font-size: 9px;
  }

  .resize-handle {
    position: absolute;
    top: 0;
    bottom: 0;
    right: -5px;
    z-index: 2;
    width: 10px;
    cursor: col-resize;
    touch-action: none;
  }

  .head-actions {
    min-width: 0;
    height: 100%;
    padding: 0 10px;
    border-left: 1px solid var(--border-subtle);
  }

  .entry-list {
    flex: 1;
    min-height: 0;
    width: max-content;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0 0 16px;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .entry-row {
    position: relative;
    display: grid;
    grid-template-columns: var(--entry-cols);
    align-items: center;
    gap: 0;
    height: 40px;
    padding: 0;
    cursor: pointer;
  }

  .entry-row-color-bar {
    position: absolute;
    left: 0;
    top: 4px;
    bottom: 4px;
    width: 3px;
    border-radius: 0 2px 2px 0;
    background: var(--row-color);
  }

  .app-shell.compact .entry-row {
    height: 34px;
  }

  .entry-row:hover {
    background: var(--hover-bg);
  }

  .entry-row.selected {
    background: color-mix(in srgb, var(--selection-color) 15%, var(--hover-bg));
  }

  .entry-row-icon-cell {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    border-right: 1px solid var(--border-subtle);
  }

  .entry-row-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-icon-radius, 7px);
    color: var(--warning-color);
    background: var(--input-bg);
  }

  .entry-row-img {
    width: 16px;
    height: 16px;
    display: block;
    border-radius: 2px;
    object-fit: contain;
  }

  .entry-row-main {
    display: flex;
    flex-direction: column;
    justify-content: center;
    min-width: 0;
    height: 100%;
    flex: 1;
  }

  .entry-row-title {
    overflow: hidden;
    color: var(--text-primary);
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-row.expired-row .entry-row-title {
    color: color-mix(in srgb, var(--danger-color) 80%, var(--text-primary));
  }

  .expired-flag {
    display: inline-block;
    margin-left: 6px;
    padding: 1px 5px;
    border: 1px solid color-mix(in srgb, var(--danger-color) 40%, transparent);
    border-radius: 4px;
    color: var(--danger-color);
    background: color-mix(in srgb, var(--danger-color) 10%, transparent);
    font-size: 9px;
    line-height: 1.4;
    vertical-align: 1px;
  }

  .entry-row-sub {
    overflow: hidden;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-row-col {
    display: flex;
    align-items: center;
    overflow: hidden;
    min-width: 0;
    height: 100%;
    padding: 0 8px;
    border-right: 1px solid var(--border-subtle);
    font-size: 12px;
  }

  .entry-row-col-text {
    overflow: hidden;
    min-width: 0;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .col-title {
    color: var(--text-primary);
  }

  .col-masked {
    color: var(--text-faint);
  }

  .col-totp {
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .entry-row-actions {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 2px;
    min-width: 0;
    height: 100%;
    padding: 0 10px;
    border-left: 1px solid var(--border-subtle);
    opacity: 0;
  }

  .entry-row:hover .entry-row-actions,
  .entry-row.selected .entry-row-actions,
  .entry-row:focus-within .entry-row-actions {
    opacity: 1;
  }

  .row-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
    cursor: pointer;
  }

  .row-btn:hover {
    color: var(--text-primary);
    background: color-mix(in srgb, var(--text-primary) 10%, transparent);
  }

  .row-btn.star-active {
    color: var(--warning-color);
  }

  .row-btn.star-active:hover {
    color: var(--warning-color);
  }

  .detail-panel {
    position: absolute;
    top: 0;
    bottom: 0;
    right: 0;
    width: var(--detail-width, 300px);
    min-width: 0;
    z-index: 2;
    border-left: 1px solid var(--border-color);
    background: var(--card-bg);
  }

  .detail-resize-handle {
    position: absolute;
    top: 0;
    bottom: 0;
    right: calc(var(--detail-width, 300px) - 5px);
    z-index: 3;
    width: 10px;
    cursor: col-resize;
    touch-action: none;
  }

  .group-resize-handle {
    position: absolute;
    top: 0;
    bottom: 0;
    left: calc(var(--group-width, 200px) - 5px);
    z-index: 3;
    width: 10px;
    cursor: col-resize;
    touch-action: none;
  }

  .detail-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    height: 100%;
    color: var(--text-faint);
  }

  .detail-empty p {
    margin: 0;
    font-size: var(--font-size-secondary, 11px);
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 40px 20px;
    color: var(--text-faint);
    text-align: center;
  }

  .empty-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 40px;
    margin-bottom: 4px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-card-radius, 9px);
    background: var(--card-bg);
  }

  .empty-state strong {
    color: var(--text-secondary);
    font-size: 12px;
    font-weight: 560;
  }

  .empty-state p {
    margin: 0;
    font-size: var(--font-size-tiny, 10px);
  }

  .status-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    min-height: 26px;
    padding: 4px 14px;
    border-top: 1px solid var(--border-subtle);
    color: var(--text-faint);
    background: var(--statusbar-bg);
    font-size: var(--font-size-tiny, 10px);
  }

  .status-left,
  .status-right {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .result-count {
    font-variant-numeric: tabular-nums;
  }

  .status-group-filter {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status-dirty {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--warning-color);
  }

  .status-dirty i {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warning-color);
  }

  .status-msg {
    overflow: hidden;
    color: var(--text-secondary);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .status-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 220px;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, #000 45%, transparent);
  }

  .group-modal {
    width: min(340px, calc(100% - 40px));
    padding: 18px;
    border: 1px solid var(--border-color);
    border-radius: 13px;
    background: var(--surface-bg);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
  }

  .group-icon-label {
    display: block;
    margin-top: 12px;
    margin-bottom: 6px;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
  }

  .group-icon-grid {
    display: grid;
    grid-template-columns: repeat(9, 1fr);
    gap: 4px;
  }

  .group-modal .icon-option {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 30px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    cursor: pointer;
  }

  .group-modal .icon-option:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .group-modal .icon-option.selected {
    color: var(--accent-color, var(--primary-color));
    border-color: color-mix(in srgb, var(--primary-color) 55%, transparent);
    background: color-mix(in srgb, var(--primary-color) 12%, transparent);
  }

  .confirm-modal {
    width: min(380px, calc(100% - 40px));
  }

  .modal-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }

  .modal-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-icon-radius, 7px);
    color: var(--selection-color);
    background: var(--hover-bg);
  }

  .modal-icon.danger {
    color: var(--danger-color);
  }

  .modal-head strong {
    display: block;
    font-size: 13px;
    font-weight: 560;
  }

  .modal-head p {
    margin: 2px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .text-input {
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: 12px;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }

  .modal-button {
    height: 30px;
    padding: 0 14px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--card-bg);
    font-size: 12px;
    cursor: pointer;
  }

  .modal-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .modal-button.primary {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
  }

  .modal-button.danger {
    border-color: color-mix(in srgb, var(--danger-color) 50%, transparent);
    color: color-mix(in srgb, var(--danger-color) 80%, white);
    background: color-mix(in srgb, var(--danger-color) 14%, var(--card-bg));
  }

  .modal-button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .progress-track {
    height: 6px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
    overflow: hidden;
  }

  .progress-fill {
    width: var(--progress-pct, 0%);
    height: 100%;
    border-radius: inherit;
    background: var(--selection-color);
    transition: width 0.2s ease;
  }

  .progress-fill.indeterminate {
    width: 40%;
    animation: progress-slide 1.1s ease-in-out infinite alternate;
  }

  @keyframes progress-slide {
    from {
      transform: translateX(-110%);
    }
    to {
      transform: translateX(260%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .progress-fill {
      transition: none;
    }
    .progress-fill.indeterminate {
      animation: none;
    }
  }

  :global(body.resizing-column) {
    cursor: col-resize !important;
    user-select: none;
  }
</style>
