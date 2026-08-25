<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { get } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { vault } from "$lib/services/vault";
  import { appSettings, isMobile, isTauriRuntime } from "$lib/services/settings";
  import type { EntryColumnState } from "$lib/types/settings";
  import ColumnConfigMenu, {
    type ColumnMenuSection,
  } from "$lib/components/ColumnConfigMenu.svelte";
  import { dispatchShortcut, effectiveShortcuts } from "$lib/services/keyboard";
  import { syncCompactShellClass } from "$lib/services/settings-bootstrap";
  import { armIdleLock, beginTcatoOverlayOpen, lockVault, copyValue } from "$lib/services/security";
  import { usePanelLayout } from "$lib/composables/usePanelLayout.svelte";
  import { useVaultSelection } from "$lib/composables/useVaultSelection.svelte";
  import { useEntryFilter } from "$lib/composables/useEntryFilter.svelte";
  import { useEntryEditor } from "$lib/composables/useEntryEditor.svelte";
  import { BUILTIN_COLUMNS, useEntryColumns } from "$lib/services/columns.svelte";
  import { runFaviconDownload, type FaviconFlowHost } from "$lib/services/favicon-flow";
  import {
    changeGroupIconFlow,
    confirmDeleteGroupFlow,
    confirmEmptyRecycleBinFlow,
    createGroupFlow,
    moveEntriesFlow,
    renameGroupFlow,
    restoreGroupFlow,
    saveGroupMetaFlow,
    setGroupExpandedFlow,
    setGroupsExpandedFlow,
    type GroupFlowHost,
  } from "$lib/services/group-flows";
  import {
    csvToImportEntries,
    downloadTextFile,
    importEntries as runImportEntries,
    importRowsToEntries,
    pickImportFile,
    toCsvExportRows,
    toXmlExportRows,
    xmlToImportEntries,
    type IoHost,
    type ImportEntry,
  } from "$lib/services/io";
  import { showTip } from "$lib/services/tips";
  import type {
    EntryInput,
    EntryPatch,
    EntryFlags,
    EntryAutoTypeConfig,
    ImportRow,
    AutotypeCandidate,
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
  import { closeContextMenu, openContextMenu } from "$lib/stores/activeContextMenu.svelte";
  import VaultWelcome from "$lib/components/VaultWelcome.svelte";
  import LockScreen from "$lib/components/LockScreen.svelte";
  import GroupTree from "$lib/components/GroupTree.svelte";
  import VaultTabs from "$lib/components/VaultTabs.svelte";
  import GroupAutoTypeDialog from "$lib/components/GroupAutoTypeDialog.svelte";
  import GroupMetaDialog from "$lib/components/GroupMetaDialog.svelte";
  import EntryDetail from "$lib/components/EntryDetail.svelte";
  import EntryEditorDialog from "$lib/components/EntryEditorDialog.svelte";
  import EntryTable, { type EntryTableColumn } from "$lib/components/EntryTable.svelte";
  import AdvancedSearchDialog from "$lib/components/AdvancedSearchDialog.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import AppToolbar from "$lib/components/AppToolbar.svelte";
  import AutotypePickDialog from "$lib/components/AutotypePickDialog.svelte";
  import FaviconProgressDialog from "$lib/components/FaviconProgressDialog.svelte";
  import SecurityReportDialog from "$lib/components/SecurityReportDialog.svelte";
  import SimilarPasswordsDialog from "$lib/components/SimilarPasswordsDialog.svelte";
  import ExpiredEntriesDialog from "$lib/components/ExpiredEntriesDialog.svelte";
  import ChangeTimelineDialog from "$lib/components/ChangeTimelineDialog.svelte";
  import HibpCheckDialog from "$lib/components/HibpCheckDialog.svelte";
  import DbMetaDialog from "$lib/components/DbMetaDialog.svelte";
  import DatabaseSettingsDialog from "$lib/components/DatabaseSettingsDialog.svelte";
  import TcatoOverlay from "$lib/components/TcatoOverlay.svelte";
  import WindowControls from "$lib/components/WindowControls.svelte";
  import { buildCsv, parseCsv, parseCsvRows } from "$lib/utils/csv";
  import { buildKeePassXml, parseKdbxXml } from "$lib/utils/kdbx-xml";
  import { formatDateOnly } from "$lib/utils/date";
  import { formatBytes, formatKeePassSize } from "$lib/utils/format";
  import { resolveImportGroupPath, type ImportGroupResolver } from "$lib/utils/import-groups";
  import {
    awaitCurrentView,
    consumeCurrentView,
    LatestOperationGuard,
    sessionResourceKey,
    SessionViewGuard,
    type SessionViewToken,
  } from "$lib/utils/session-state";
  import { matchesAdvancedSearch, type AdvancedSearchQuery } from "$lib/utils/entry-search";
  import {
    buildBlankMenuItems,
    buildEntryMenuItems,
    buildToolbarMenuItems,
  } from "$lib/utils/menu-items";
  import {
    buildGroupPathIndex,
    buildVaultTreeIndex,
    collectGroups,
    collectEntries,
    findEntryIn,
    findGroupIn,
  } from "$lib/utils/tree";

  const ENTRY_SORT_COLLATOR = new Intl.Collator("zh-CN", { numeric: true });

  /** The TCATO overlay window loads the same SPA with a `#/tcato` hash. */
  const isTcatoOverlay =
    typeof window !== "undefined" && window.location.hash.startsWith("#/tcato");
  const showWindowControls = isTauriRuntime() && !isMobile();

  let currentVault = $state<VaultState | null>(null);
  let currentVaultSessionId = $state<string | null>(null);
  let activeSessionId = $state<string | null>(null);
  const sessionView = new SessionViewGuard();
  const busyOperations = new LatestOperationGuard();
  const groupCreateOperations = new LatestOperationGuard();
  const groupIconOperations = new LatestOperationGuard();
  const tcatoOperations = new LatestOperationGuard();
  let rememberedPath = $state<{ path: string; fileName: string } | null>(null);
  let search = $state("");
  let advancedQuery = $state<AdvancedSearchQuery | null>(null);
  let advancedSearchOpen = $state(false);
  let selectedGroup = $state<string | null>(null);
  let revealGroupUuid = $state<string | null>(null);
  let groupModalOpen = $state(false);
  let groupModalParent = $state<string | null>(null);
  let newGroupName = $state("");
  let groupIconIndex = $state<number | null>(null);
  let groupCreating = $state(false);
  let groupIconDialogUuid = $state<string | null>(null);
  let groupAutoTypeUuid = $state<string | null>(null);
  let groupMetaUuid = $state<string | null>(null);
  let groupIconPick = $state<number | null>(null);
  let groupIconSaving = $state(false);
  let confirmState = $state<{ message: string; onconfirm: () => void } | null>(null);
  let autotypePick = $state<AutotypeCandidate[] | null>(null);
  let busy = $state(false);
  let reportOpen = $state(false);
  let remoteConflict = $state<string | null>(null);
  let similarOpen = $state(false);
  let expiredOpen = $state(false);
  let timelineOpen = $state(false);
  let hibpOpen = $state(false);
  let emergencyExportOpen = $state(false);
  let emergencyIncludePasswords = $state(false);
  let dbMetaOpen = $state(false);
  let dbSettingsOpen = $state(false);
  let securityReport = $state<SecurityReport | null>(null);
  let faviconDialog = $state<{
    phase: "working" | "done";
    progress: FaviconProgress;
    result: string;
    error: boolean;
  } | null>(null);

  /** Component hooks handed to the shared import/export orchestration in
   *  `$lib/services/io`; keeps every stale-view branch behavior-identical. */
  const ioHost: IoHost = {
    sessionView,
    operations: busyOperations,
    setBusy: (value) => {
      busy = value;
    },
    notify: flash,
    currentState: () => currentVault,
  };

  /** Component hooks for the extracted favicon download flow. */
  const faviconHost: FaviconFlowHost = {
    sessionView,
    operations: busyOperations,
    isBusy: () => busy,
    setBusy: (value) => {
      busy = value;
    },
    notify: flash,
    setDialog: (state) => {
      faviconDialog = state;
    },
  };

  /** Component hooks for the extracted group/recycle-bin flows; `ask` feeds
   *  the page-owned confirmation dialog. */
  const groupFlowHost: GroupFlowHost = {
    sessionView,
    createOperations: groupCreateOperations,
    iconOperations: groupIconOperations,
    notify: flash,
    ask: (message, onconfirm) => {
      confirmState = { message, onconfirm };
    },
  };

  /** Seen (sessionId, path) pairs that already flashed the expired-entries
   *  toast. Session ids are never recycled, so the keys accumulate for the
   *  whole window lifetime; cap the set and evict the oldest key so long
   *  sessions cannot grow it without bound. The dedupe only suppresses a
   *  transient toast, so a rare re-notify for an evicted key is benign. */
  const expiredNotifiedViews = new Set<string>();
  const MAX_EXPIRED_NOTIFICATIONS = 256;
  function markExpiredNotified(key: string): void {
    if (expiredNotifiedViews.size >= MAX_EXPIRED_NOTIFICATIONS) {
      const oldest = expiredNotifiedViews.values().next().value;
      if (oldest !== undefined) expiredNotifiedViews.delete(oldest);
    }
    expiredNotifiedViews.add(key);
  }

  function countExpiredEntries(group: VaultGroup): number {
    return collectEntries(group).filter((e) => e.expired).length;
  }

  async function notifyExpiredEntries(view: SessionViewToken): Promise<void> {
    if (currentVaultSessionId !== view.sessionId || !currentVault) return;
    const requestedPath = currentVault.path;
    const delayed = await awaitCurrentView(
      sessionView,
      view,
      () => new Promise<void>((resolve) => setTimeout(resolve, 300)),
    );
    if (
      !delayed.current ||
      currentVaultSessionId !== view.sessionId ||
      currentVault?.path !== requestedPath
    ) {
      return;
    }

    const notificationKey = sessionResourceKey(view.sessionId, requestedPath);
    if (expiredNotifiedViews.has(notificationKey)) return;
    markExpiredNotified(notificationKey);

    const expired = countExpiredEntries(currentVault.root);
    if (expired > 0) flash(`有 ${expired} 个条目已过期,请及时更新密码`);
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
      const opened = Boolean(value) && !currentVault;
      const closed = !value;
      const previousOwner = currentVaultSessionId;
      const previousPath = currentVault?.path ?? null;
      currentVault = value;
      currentVaultSessionId = value
        ? (vault.getActiveSessionId() ?? (!isTauriRuntime() ? "browser" : null))
        : null;
      if (
        currentVaultSessionId &&
        (currentVaultSessionId !== previousOwner || value?.path !== previousPath)
      ) {
        const view = sessionView.capture();
        if (view?.sessionId === currentVaultSessionId) void notifyExpiredEntries(view);
      }
      if (!value) {
        selection.selectedEntry = null;
        selection.selectedUuids = new Set();
        selection.selectionAnchor = null;
        search = "";
        advancedQuery = null;
        selectedGroup = null;
        editor.reset();
      } else {
        selection.selectedEntry = findEntryByUuid(value, selection.selectedEntry?.uuid ?? null);
      }
      // Re-arm the idle timer only on open/close transitions; every refresh
      // (save, favicon run, RPC write) otherwise silently resets the deadline
      // and auto-lock stops measuring real user inactivity.
      if (opened || closed) armIdleLock();
    });
    const unsubRemembered = vault.remembered((value) => {
      rememberedPath = value;
    });
    const unsubActive = vault.activeId.subscribe((value) => {
      if (value === activeSessionId) return;
      activeSessionId = value;
      sessionView.activate(value);
      const view = sessionView.capture();
      if (view) void notifyExpiredEntries(view);
      // A long operation that started in the previous tab intentionally skips
      // its `finally` write after switching. Reset shared activity flags here
      // so the newly visible tab cannot inherit a disabled toolbar/dialog.
      busyOperations.invalidate();
      groupCreateOperations.invalidate();
      groupIconOperations.invalidate();
      tcatoOperations.invalidate();
      busy = false;
      groupCreating = false;
      groupIconSaving = false;
      search = "";
      advancedQuery = null;
      selectedGroup = null;
      revealGroupUuid = null;
      selection.selectedEntry = null;
      selection.selectedUuids = new Set();
      selection.selectionAnchor = null;
      securityReport = null;
      // Session-scoped overlays/forms must never survive a tab switch: their
      // captured uuids may also exist in an identical copy of another vault.
      editor.reset();
      groupModalOpen = false;
      groupIconDialogUuid = null;
      groupAutoTypeUuid = null;
      groupMetaUuid = null;
      reportOpen = false;
      similarOpen = false;
      expiredOpen = false;
      timelineOpen = false;
      hibpOpen = false;
      emergencyExportOpen = false;
      dbMetaOpen = false;
      dbSettingsOpen = false;
      confirmState = null;
      autotypePick = null;
      faviconDialog = null;
      remoteConflict = null;
      entryMenu = null;
      blankMenu = null;
      toolbarMenu = null;
      columnMenu = null;
      closeContextMenu();
      advancedSearchOpen = false;
      layout.mobileNavOpen = false;
      emergencyIncludePasswords = false;
    });
    // Browser-extension writes land straight in the backend session; refresh
    // so the entry list and dirty tab state update without a reopen.
    let unlistenVaultChanged: UnlistenFn | undefined;
    let unlistenAutotypePick: UnlistenFn | undefined;
    if (isTauriRuntime()) {
      void listen("browser-vault-changed", () => void vault.refresh()).then(
        (stop) => (unlistenVaultChanged = stop),
      );
      void listen<AutotypeCandidate[]>("autotype-pick-request", (event) => {
        const candidates = event.payload;
        if (
          candidates.length === 0 ||
          candidates.some((candidate) => candidate.sessionId !== vault.getActiveSessionId())
        ) {
          return;
        }
        autotypePick = candidates;
      }).then((stop) => (unlistenAutotypePick = stop));
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
      unsubActive();
      void unlistenVaultChanged?.();
      void unlistenAutotypePick?.();
      window.removeEventListener("resize", rememberWindowSize);
      if (windowResizeTimer) clearTimeout(windowResizeTimer);
      sessionView.activate(null);
    };
  });

  function flash(message: string): void {
    showTip(message);
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
  const toolbarOverflowMenu = $derived(settings.general.toolbarOverflowMenu);
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
  /** Mirror of the window-size settings; `get(appSettings)` is untracked in
   *  an effect, so a plain read would freeze at the initial value. */
  let winSize = $state({
    width: get(appSettings).general.windowWidth,
    height: get(appSettings).general.windowHeight,
  });
  $effect(() => {
    const unsubscribe = appSettings.subscribe((s) => {
      winSize = { width: s.general.windowWidth, height: s.general.windowHeight };
    });
    return unsubscribe;
  });

  $effect(() => {
    const view = currentVault === null ? (showLockScreen ? "lock" : "welcome") : "main";
    if (!isTauriRuntime() || isTcatoOverlay || view === "lock") return;
    const width = view === "welcome" ? WELCOME_WINDOW_SIZE.width : winSize.width;
    const height = view === "welcome" ? WELCOME_WINDOW_SIZE.height : winSize.height;
    const key = `${width}x${height}`;
    if (lastAppliedSize === key) return;
    lastAppliedSize = key;
    void getCurrentWindow().setSize(new LogicalSize(width, height));
  });

  const treeIndex = $derived(currentVault ? buildVaultTreeIndex(currentVault.root) : null);
  const allGroups = $derived(treeIndex?.groups ?? []);
  const allEntries = $derived(treeIndex?.entries ?? []);

  const reportEntries = $derived.by(() => {
    if (!treeIndex) return [];
    return allEntries.map((entry) => ({
      entry,
      path: treeIndex.pathByGroupUuid.get(entry.groupUuid) ?? "",
    }));
  });

  function pathOf(groupUuid: string): string {
    return treeIndex?.pathByGroupUuid.get(groupUuid) ?? "";
  }

  /** Whether the given group uuid is the recycle bin or nested inside it. */
  function groupInBin(groupUuid: string): boolean {
    return treeIndex?.recycleBinUuids.has(groupUuid) ?? false;
  }

  function selectedGroupInBin(uuid: string): boolean {
    return groupInBin(uuid);
  }

  function entryInBin(entryUuid: string): boolean {
    const entry = treeIndex?.entryByUuid.get(entryUuid);
    if (!entry) return false;
    return groupInBin(entry.groupUuid);
  }

  const selectedSubtree = $derived.by((): VaultGroup[] => {
    if (!currentVault) return [];
    if (selectedGroup === null) return allGroups.filter((g) => !groupInBin(g.uuid));
    const group = treeIndex?.groupByUuid.get(selectedGroup);
    if (!group) return allGroups;
    return collectGroups(group);
  });

  $effect(() => {
    // 调试：F12 控制台可见，无需 window.__TAURI__
    console.log(
      `[SecPivot debug] vault=${currentVault?.path ?? "null"} groups=${allGroups.length} entries=${allEntries.length} filtered=${filteredEntries.length} subtree=${selectedSubtree.length} selectedGroup=${selectedGroup ?? "null"} search="${search}" adv=${advancedQuery ? "on" : "off"}`,
    );
  });

  /** Search/filter pipeline lives in the extracted composable; see
   *  `useEntryFilter.svelte.ts`. */
  const entryFilter = useEntryFilter({
    currentVault: () => currentVault,
    selectedSubtree: () => selectedSubtree,
    search: () => search,
    advancedQuery: () => advancedQuery,
  });
  const filteredEntries = $derived(entryFilter.filteredEntries);

  type SortCol = string;
  let sortCol = $state<SortCol>("title");
  let sortDir = $state<"asc" | "desc">("asc");
  /** Column visibility/width/order, grid template, sort keys and cell text
   *  live in the extracted service; see `columns.svelte.ts`. */
  const columns = useEntryColumns(() => allEntries);
  /** Panel widths, detail visibility, mobile drawer and drag-resize logic live
   *  in the extracted composable; see `usePanelLayout.svelte.ts`. */
  const layout = usePanelLayout({
    entryColumns: () => columns.entryColumns,
    urlColWidth: () => columns.colState("url").width || 200,
    selectedEntry: () => selection.selectedEntry,
  });
  const sortedEntries = $derived.by(() => {
    const dir = sortDir === "asc" ? 1 : -1;
    const col = sortCol;
    const keyedEntries = filteredEntries.map((row) => ({
      row,
      favorite: Number(row.entry.favorite),
      key: columns.sortKeyFor(row.entry, col),
    }));
    keyedEntries.sort((a, b) => {
      const fav = b.favorite - a.favorite;
      if (fav !== 0) return fav;
      return ENTRY_SORT_COLLATOR.compare(a.key, b.key) * dir;
    });
    return keyedEntries.map(({ row }) => row);
  });

  /** Selection model (single/shift-range/ctrl-toggle) lives in the extracted
   *  composable; see `useVaultSelection.svelte.ts`. */
  const selection = useVaultSelection({
    visibleUuids: () => sortedEntries.map((r) => r.entry.uuid),
  });

  function cycleSort(col: SortCol): void {
    if (sortCol === col) {
      sortDir = sortDir === "asc" ? "desc" : "asc";
    } else {
      sortCol = col;
      sortDir = "asc";
    }
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
          visible: columns.colState(def.id).visible,
        })),
      },
    ];
    if (columns.customColumnNames.length > 0) {
      sections.push({
        label: "自定义条目",
        items: columns.customColumnNames.map((name) => {
          const id = `custom:${name}`;
          return { id, label: name, visible: columns.colState(id).visible };
        }),
      });
    }
    return sections;
  });
  function openColumnMenu(e: MouseEvent): void {
    e.preventDefault();
    entryMenu = null;
    blankMenu = null;
    toolbarMenu = null;
    columnMenu = { x: e.clientX, y: e.clientY };
    openContextMenu("page");
  }

  function findEntryByUuid(state: VaultState | null, uuid: string | null): VaultEntry | null {
    if (!state || !uuid) return null;
    return findEntryIn(state.root, uuid);
  }

  /** The just-created entry is the one with the newest `created` stamp in its
   *  target group (the backend generates its uuid, so we locate it this way). */
  function findNewestEntryInGroup(state: VaultState, groupUuid: string): VaultEntry | null {
    const group = findGroupIn(state.root, groupUuid);
    if (!group) return null;
    let newest: VaultEntry | null = null;
    for (const entry of group.entries) {
      if (!newest || (entry.created ?? "") >= (newest.created ?? "")) newest = entry;
    }
    return newest;
  }

  function setSingleSelection(entry: VaultEntry | null): void {
    selection.setSingleSelection(entry);
  }

  async function toggleFavorite(entry: VaultEntry): Promise<void> {
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    try {
      const saved = await vault.callInSession(sessionId, () => vault.toggleFavorite(entry.uuid));
      if (!sessionView.isCurrent(view)) return;
      if (selection.selectedEntry?.uuid === entry.uuid) {
        selection.selectedEntry = findEntryByUuid(saved, entry.uuid);
      }
    } catch (e) {
      if (sessionView.isCurrent(view)) flash(`收藏失败：${e}`);
    }
  }

  async function handleSave(): Promise<void> {
    if (!currentVault || !currentVault.dirty) {
      flash("没有需要保存的修改");
      return;
    }
    if (currentVault.readOnly) {
      flash("数据库已进入只读模式：连续保存失败，请使用「另存为」到可写位置后继续");
      return;
    }
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    const operation = busyOperations.begin();
    busy = true;
    try {
      const saved = await vault.callInSession(sessionId, () => vault.save());
      if (!sessionView.isCurrent(view)) return;
      selection.selectedEntry = findEntryByUuid(saved, selection.selectedEntry?.uuid ?? null);
      flash("已保存到数据库");
    } catch (e) {
      if (!sessionView.isCurrent(view)) return;
      const message = String(e);
      if (message.startsWith("REMOTE_CHANGED")) {
        remoteConflict = message.replace("REMOTE_CHANGED\n", "");
      } else {
        flash(`保存失败：${message}`);
      }
    } finally {
      if (sessionView.isCurrent(view) && busyOperations.isCurrent(operation)) busy = false;
    }
  }

  async function resolveRemoteConflict(action: "merge" | "overwrite" | "download"): Promise<void> {
    const message = remoteConflict;
    remoteConflict = null;
    if (!currentVault) return;
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    if (
      action === "download" &&
      !window.confirm("下载远程版本将丢弃当前未保存的本地修改，继续？")
    ) {
      if (sessionView.isCurrent(view)) remoteConflict = message;
      return;
    }
    const operation = busyOperations.begin();
    busy = true;
    try {
      if (action === "merge") {
        const merged = await vault.callInSession(sessionId, () => vault.mergeRemote());
        if (!sessionView.isCurrent(view)) return;
        selection.selectedEntry = findEntryByUuid(merged, selection.selectedEntry?.uuid ?? null);
        flash("已合并本地与远程版本");
      } else if (action === "overwrite") {
        const saved = await vault.callInSession(sessionId, () => vault.save(true));
        if (!sessionView.isCurrent(view)) return;
        selection.selectedEntry = findEntryByUuid(saved, selection.selectedEntry?.uuid ?? null);
        flash("已覆盖远程版本");
      } else {
        const refreshed = await vault.callInSession(sessionId, () => vault.refreshRemote());
        if (!sessionView.isCurrent(view)) return;
        selection.selectedEntry = findEntryByUuid(refreshed, selection.selectedEntry?.uuid ?? null);
        flash("已下载远程版本");
      }
    } catch (e) {
      if (!sessionView.isCurrent(view)) return;
      flash(`操作失败：${e}`);
    } finally {
      if (sessionView.isCurrent(view) && busyOperations.isCurrent(operation)) busy = false;
    }
  }

  /** Save As: pick a new local path, persist there and switch to it. */
  async function handleSaveAs(): Promise<void> {
    if (!currentVault) return;
    if (!isTauriRuntime()) {
      flash("浏览器预览不支持另存为");
      return;
    }
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    const fileName = currentVault.fileName;
    try {
      const ext = get(appSettings).database.fileExtension;
      const baseName = (fileName.replace(/\.\w+$/i, "") || "secpivot") + "." + ext;
      const picked = await awaitCurrentView(sessionView, view, () =>
        save({
          defaultPath: baseName,
          filters: [{ name: "KeePass 数据库", extensions: [ext] }],
        }),
      );
      if (!picked.current || !picked.value) return;
      await vault.callInSession(sessionId, () => vault.saveAs(String(picked.value)));
      if (!sessionView.isCurrent(view)) return;
      flash("已另存为数据库");
    } catch (e) {
      if (sessionView.isCurrent(view)) flash(`另存为失败：${e}`);
    }
  }

  async function handleExportCsv(): Promise<void> {
    if (!currentVault) return;
    if (!window.confirm("导出的 CSV 包含明文密码，请妥善保管并在使用后删除。继续导出？")) return;
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    const fileName = currentVault.fileName;
    try {
      if (isTauriRuntime()) {
        const picked = await awaitCurrentView(sessionView, view, () =>
          save({
            defaultPath: (fileName.replace(/\.kdbx$/i, "") || "secpivot") + ".csv",
            filters: [{ name: "CSV 文件", extensions: ["csv"] }],
          }),
        );
        if (!picked.current || !picked.value) return;
        await invoke("export_csv", {
          sessionId,
          path: String(picked.value),
        });
      } else {
        if (!sessionView.isCurrent(view)) return;
        downloadTextFile(
          buildCsv(toCsvExportRows(reportEntries)),
          "secpivot-export.csv",
          "text/csv;charset=utf-8",
        );
      }
      if (sessionView.isCurrent(view)) flash("已导出 CSV");
    } catch (e) {
      if (sessionView.isCurrent(view)) flash(`导出失败：${e}`);
    }
  }

  async function handleExportXml(): Promise<void> {
    if (!currentVault) return;
    if (!window.confirm("导出的 XML 包含明文密码，请妥善保管并在使用后删除。继续导出？")) return;
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    const fileName = currentVault.fileName;
    try {
      if (isTauriRuntime()) {
        const picked = await awaitCurrentView(sessionView, view, () =>
          save({
            defaultPath: (fileName.replace(/\.kdbx$/i, "") || "secpivot") + ".xml",
            filters: [{ name: "KeePass XML 文件", extensions: ["xml"] }],
          }),
        );
        if (!picked.current || !picked.value) return;
        await invoke("export_xml", {
          sessionId,
          path: String(picked.value),
        });
      } else {
        if (!sessionView.isCurrent(view)) return;
        downloadTextFile(
          buildKeePassXml(toXmlExportRows(reportEntries)),
          "secpivot-export.xml",
          "text/xml;charset=utf-8",
        );
      }
      if (sessionView.isCurrent(view)) flash("已导出 XML");
    } catch (e) {
      if (sessionView.isCurrent(view)) flash(`导出失败：${e}`);
    }
  }

  async function confirmExportEmergency(): Promise<void> {
    if (!currentVault) return;
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    const includePasswords = emergencyIncludePasswords;
    try {
      const picked = await awaitCurrentView(sessionView, view, () =>
        save({
          defaultPath: `SecPivot-应急表-${new Date().toISOString().slice(0, 10)}.html`,
          filters: [{ name: "HTML 文件", extensions: ["html"] }],
        }),
      );
      if (!picked.current || !picked.value) return;
      await invoke("export_emergency_sheet", {
        sessionId,
        path: String(picked.value),
        includePasswords,
      });
      if (!sessionView.isCurrent(view)) return;
      emergencyExportOpen = false;
      flash("应急表已导出");
    } catch (e) {
      if (sessionView.isCurrent(view)) flash(`导出应急表失败：${e}`);
    }
  }

  async function handleClearHistory(): Promise<void> {
    if (!currentVault) return;
    if (!window.confirm("将删除所有条目的历史版本快照（当前条目内容不受影响）。继续？")) return;
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    try {
      const cleared = await vault.callInSession(sessionId, () => vault.clearAllHistory());
      if (!sessionView.isCurrent(view)) return;
      flash(`已清理 ${cleared} 个条目的历史`);
    } catch (e) {
      if (sessionView.isCurrent(view)) flash(`清理历史失败：${e}`);
    }
  }

  async function handleOpenReport(): Promise<void> {
    if (reportOpen || busy || !currentVault) return;
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    const operation = busyOperations.begin();
    busy = true;
    try {
      const report = await vault.callInSession(sessionId, () => vault.securityReport());
      if (!sessionView.isCurrent(view)) return;
      securityReport = report;
      reportOpen = true;
    } catch (e) {
      if (!sessionView.isCurrent(view)) return;
      flash(`安全分析失败：${e}`);
    } finally {
      if (sessionView.isCurrent(view) && busyOperations.isCurrent(operation)) busy = false;
    }
  }

  async function handleDownloadFavicons(): Promise<void> {
    await runFaviconDownload(faviconHost, undefined, "没有可下载的网址图标");
  }

  /** Download icons for the selected entries only (context menu, multi-select aware). */
  async function downloadSelectedFavicons(entry: VaultEntry): Promise<void> {
    const uuids =
      selection.selectedUuids.size > 1 ? Array.from(selection.selectedUuids) : [entry.uuid];
    await runFaviconDownload(faviconHost, uuids, "所选条目没有可下载的网址图标");
  }

  async function copyEntryPassword(entry: VaultEntry): Promise<void> {
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    try {
      const copied = await consumeCurrentView(
        sessionView,
        view,
        () => vault.callInSession(sessionId, () => vault.getEntryPassword(entry.uuid)),
        (password) => copyValue(password, true),
      );
      if (copied && sessionView.isCurrent(view)) flash("已复制密码");
    } catch {
      if (sessionView.isCurrent(view)) flash("复制失败");
    }
  }

  /** In-place reveal for the entry-list password column: resolve the secret
   * only for the still-current session view; any staleness or failure keeps
   * the cell masked (EntryTable re-masks on timeout/mouse leave as well). */
  async function revealEntryPassword(entry: VaultEntry): Promise<string | null> {
    const view = sessionView.capture();
    if (!view) return null;
    const { sessionId } = view;
    try {
      return await vault.callInSession(sessionId, () => vault.getEntryPassword(entry.uuid));
    } catch {
      return null;
    }
  }

  async function handleImportCsv(): Promise<void> {
    if (!currentVault) return;
    const view = sessionView.capture();
    if (!view) return;
    const baseGroupUuid = selectedGroup;
    const text = await pickImportFile(ioHost, view, [{ name: "CSV 文件", extensions: ["csv"] }]);
    if (text === null) return;
    const entries: ImportEntry[] = csvToImportEntries(text);
    if (entries.length === 0) {
      flash("CSV 中没有可导入的条目");
      return;
    }
    await runImportEntries(ioHost, entries, view, baseGroupUuid);
  }

  async function handleImportXml(): Promise<void> {
    if (!currentVault) return;
    const view = sessionView.capture();
    if (!view) return;
    const baseGroupUuid = selectedGroup;
    const text = await pickImportFile(ioHost, view, [
      { name: "KeePass XML 文件", extensions: ["xml"] },
      { name: "CSV 文件", extensions: ["csv"] },
    ]);
    if (text === null) return;
    const entries: ImportEntry[] = xmlToImportEntries(text);
    if (entries.length === 0) {
      flash("XML 中没有可导入的条目");
      return;
    }
    await runImportEntries(ioHost, entries, view, baseGroupUuid);
  }

  async function handleImportBitwarden(): Promise<void> {
    if (!currentVault) return;
    const view = sessionView.capture();
    if (!view) return;
    const baseGroupUuid = selectedGroup;
    const text = await pickImportFile(ioHost, view, [
      { name: "Bitwarden JSON 文件", extensions: ["json"] },
    ]);
    if (text === null) return;
    let rows: ImportRow[];
    try {
      if (!isTauriRuntime()) throw new Error("浏览器预览不支持 Bitwarden 导入");
      const parsed = await awaitCurrentView(sessionView, view, () =>
        invoke<ImportRow[]>("parse_bitwarden_json", { text }),
      );
      if (!parsed.current) return;
      rows = parsed.value;
    } catch (e) {
      if (sessionView.isCurrent(view)) flash(`导入 Bitwarden 失败：${e}`);
      return;
    }
    const entries: ImportEntry[] = importRowsToEntries(rows);
    if (entries.length === 0) {
      flash("Bitwarden 文件中没有可导入的条目");
      return;
    }
    await runImportEntries(ioHost, entries, view, baseGroupUuid);
  }

  async function handleImportOnePassword(): Promise<void> {
    if (!currentVault) return;
    const view = sessionView.capture();
    if (!view) return;
    const baseGroupUuid = selectedGroup;
    const text = await pickImportFile(ioHost, view, [
      { name: "1Password 1PIF 文件", extensions: ["1pif"] },
    ]);
    if (text === null) return;
    let rows: ImportRow[];
    try {
      if (!isTauriRuntime()) throw new Error("浏览器预览不支持 1Password 导入");
      const parsed = await awaitCurrentView(sessionView, view, () =>
        invoke<ImportRow[]>("parse_1pif", { text }),
      );
      if (!parsed.current) return;
      rows = parsed.value;
    } catch (e) {
      if (sessionView.isCurrent(view)) flash(`导入 1Password 失败：${e}`);
      return;
    }
    const entries: ImportEntry[] = importRowsToEntries(rows);
    if (entries.length === 0) {
      flash("1PIF 文件中没有可导入的条目");
      return;
    }
    await runImportEntries(ioHost, entries, view, baseGroupUuid);
  }

  function openSettings(): void {
    void goto("/settings");
  }

  async function handleLock(): Promise<void> {
    if (currentVault?.dirty && !window.confirm("有未保存的修改，仍要锁定吗？")) return;
    await lockVault();
    flash("数据库已锁定");
  }

  /** Editor dialog flow (open modes + guarded save pipeline); see
   *  `useEntryEditor.svelte.ts`. */
  const editor = useEntryEditor({
    sessionView,
    notify: flash,
    findEntry: findEntryByUuid,
    findNewestInGroup: findNewestEntryInGroup,
    setSingleSelection,
    setSelectedEntry: (entry) => (selection.selectedEntry = entry),
    getSelectedEntry: () => selection.selectedEntry,
  });

  let toolbarEl: {
    focusSearch: () => void;
  } | null = $state(null);

  /** Dispatch recorded app shortcuts; skipped while typing or modals are open. */
  function handleShortcutKeydown(event: KeyboardEvent): void {
    if (isTcatoOverlay || !currentVault) return;
    // Close the group icon picker with Escape (mirrors the group modal input).
    if (event.key === "Escape") {
      if (groupIconDialogUuid && !groupIconSaving) {
        groupIconDialogUuid = null;
      }
      return;
    }
    // Holding a key fires repeats: never dispatch the same shortcut twice, and
    // never start a second action while another (import/favicon/save) is running.
    if (event.repeat || busy) return;
    if (
      editor.editorOpen ||
      groupModalOpen ||
      groupIconDialogUuid ||
      reportOpen ||
      confirmState ||
      entryMenu ||
      blankMenu
    )
      return;
    const target = event.target as HTMLElement | null;
    if (
      target &&
      (target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName))
    ) {
      return;
    }
    const shortcuts = effectiveShortcuts(get(appSettings).keyboard.shortcuts);
    dispatchShortcut(event, shortcuts, {
      save: () => void handleSave(),
      lock: () => void handleLock(),
      edit: () => {
        if (selection.selectedEntry) openEditEntry(selection.selectedEntry);
      },
      "copy-password": () => {
        if (selection.selectedEntry) void copyEntryPassword(selection.selectedEntry);
      },
      "new-entry": () => editor.openCreate(),
      "focus-search": () => toolbarEl?.focusSearch(),
      "locate-in-tree": () => {
        if (selection.selectedEntry && currentVault) {
          const targetGroup = selection.selectedEntry.groupUuid;
          selectedGroup = targetGroup;
          // Reset so a repeat locate on the same group still re-expands the
          // tree even after a full collapse (setting the identical uuid
          // again would not re-run the reveal effect).
          revealGroupUuid = null;
          requestAnimationFrame(() => {
            revealGroupUuid = targetGroup;
          });
          flash("已定位到所在分组");
        }
      },
    });
  }

  /** Collect the fully-populated entries behind the current selection. */
  function collectSelectedEntries(): VaultEntry[] {
    return allEntries.filter((entry) => selection.selectedUuids.has(entry.uuid));
  }

  function openEditEntry(entry: VaultEntry): void {
    if (selection.selectedUuids.size > 1 && selection.selectedUuids.has(entry.uuid)) {
      const multiEntries = collectSelectedEntries();
      if (multiEntries.length < 2) {
        setSingleSelection(entry);
        editor.openEdit(entry);
        return;
      }
      editor.openEdit(entry, multiEntries);
      return;
    }
    editor.openEdit(entry);
  }

  function openGroupModal(parentUuid: string | null): void {
    groupModalParent = parentUuid;
    newGroupName = "";
    groupIconIndex = null;
    groupModalOpen = true;
  }

  async function confirmCreateGroup(): Promise<void> {
    const name = newGroupName.trim();
    if (!name || groupCreating) return;
    const view = sessionView.capture();
    if (!view) return;
    await createGroupFlow(groupFlowHost, {
      view,
      sessionId: view.sessionId,
      parentUuid: groupModalParent,
      name,
      iconIndex: groupIconIndex,
      closeModal: () => (groupModalOpen = false),
      resetBusy: () => (groupCreating = false),
    });
  }

  async function renameGroup(uuid: string, name: string): Promise<void> {
    const view = sessionView.capture();
    if (!view) return;
    await renameGroupFlow(groupFlowHost, { view, sessionId: view.sessionId, uuid, name });
  }

  async function saveGroupMeta(meta: {
    notes?: string;
    tags?: string;
    enableSearching?: boolean;
  }): Promise<boolean> {
    if (!groupMetaUuid) return false;
    const view = sessionView.capture();
    if (!view) return false;
    const targetUuid = groupMetaUuid;
    return saveGroupMetaFlow(groupFlowHost, {
      view,
      sessionId: view.sessionId,
      uuid: targetUuid,
      meta,
      stillTarget: () => groupMetaUuid === targetUuid,
    });
  }

  /** Open the group icon picker dialog, seeding the selection from the group's
   *  current built-in icon index (or none when it has no explicit icon). */
  function openGroupIconDialog(uuid: string): void {
    const group = treeIndex?.groupByUuid.get(uuid);
    groupIconDialogUuid = uuid;
    groupIconPick = group?.icon ?? null;
    groupIconSaving = false;
  }

  async function confirmChangeGroupIcon(): Promise<void> {
    const uuid = groupIconDialogUuid;
    if (!uuid || groupIconSaving) return;
    const view = sessionView.capture();
    if (!view) return;
    await changeGroupIconFlow(groupFlowHost, {
      view,
      sessionId: view.sessionId,
      uuid,
      pick: groupIconPick,
      closeModal: () => (groupIconDialogUuid = null),
      resetBusy: () => (groupIconSaving = false),
    });
  }

  function askDeleteGroup(uuid: string): void {
    const inBin = selectedGroupInBin(uuid);
    const view = sessionView.capture();
    if (!view) return;
    confirmDeleteGroupFlow(groupFlowHost, {
      view,
      sessionId: view.sessionId,
      uuid,
      inBin,
      resetSelectedGroup: () => {
        if (selectedGroup === uuid) selectedGroup = null;
      },
    });
  }

  function askEmptyRecycleBin(): void {
    const view = sessionView.capture();
    if (!view) return;
    confirmEmptyRecycleBinFlow(groupFlowHost, { view, sessionId: view.sessionId });
  }

  async function restoreGroup(uuid: string): Promise<void> {
    const view = sessionView.capture();
    if (!view) return;
    await restoreGroupFlow(groupFlowHost, { view, sessionId: view.sessionId, uuid });
  }

  async function restoreEntry(entry: VaultEntry): Promise<void> {
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    try {
      await vault.callInSession(sessionId, () => vault.restoreEntry(entry.uuid));
      if (!sessionView.isCurrent(view)) return;
      if (selection.selectedEntry?.uuid === entry.uuid) selection.selectedEntry = null;
      flash("已恢复条目");
    } catch (e) {
      if (sessionView.isCurrent(view)) flash(`恢复失败：${e}`);
    }
  }

  function askDeleteEntry(entry: VaultEntry): void {
    const inBin = entryInBin(entry.uuid);
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    confirmState = {
      message: inBin
        ? `永久删除条目「${entry.title || "未命名"}」？此操作无法撤销。`
        : `删除条目「${entry.title || "未命名"}」？可从回收站恢复。`,
      onconfirm: async () => {
        if (!sessionView.isCurrent(view)) return;
        try {
          await vault.callInSession(sessionId, () => vault.deleteEntry(entry.uuid));
          if (!sessionView.isCurrent(view)) return;
          if (selection.selectedEntry?.uuid === entry.uuid) selection.selectedEntry = null;
          if (selection.selectedUuids.has(entry.uuid)) {
            const next = new Set(selection.selectedUuids);
            next.delete(entry.uuid);
            selection.selectedUuids = next;
          }
          flash(inBin ? "已永久删除条目" : "已移入回收站");
        } catch (e) {
          if (sessionView.isCurrent(view)) flash(`删除失败：${e}`);
        }
      },
    };
  }

  function askDeleteEntries(): void {
    const uuids = Array.from(selection.selectedUuids);
    if (uuids.length === 0) return;
    const allInBin = uuids.every((uuid) => entryInBin(uuid));
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    confirmState = {
      message: allInBin
        ? `永久删除所选 ${uuids.length} 个条目？此操作无法撤销。`
        : `删除所选 ${uuids.length} 个条目？可从回收站恢复。`,
      onconfirm: async () => {
        if (!sessionView.isCurrent(view)) return;
        try {
          await vault.callInSession(sessionId, () => vault.deleteEntries(uuids));
          if (!sessionView.isCurrent(view)) return;
          selection.selectedUuids = new Set();
          selection.selectedEntry = null;
          flash(allInBin ? "已永久删除所选条目" : "所选条目已移入回收站");
        } catch (e) {
          if (sessionView.isCurrent(view)) flash(`删除失败：${e}`);
        }
      },
    };
  }

  async function moveEntriesTo(groupUuid: string, uuids: string[]): Promise<void> {
    if (!currentVault || uuids.length === 0) return;
    const view = sessionView.capture();
    if (!view) return;
    await moveEntriesFlow(groupFlowHost, {
      view,
      sessionId: view.sessionId,
      groupUuid,
      uuids,
    });
  }

  async function toggleGroupExpanded(uuid: string, expanded: boolean): Promise<void> {
    const view = sessionView.capture();
    if (!view) return;
    await setGroupExpandedFlow(groupFlowHost, {
      view,
      sessionId: view.sessionId,
      uuid,
      expanded,
    });
  }

  async function toggleGroupsExpanded(uuids: string[], expanded: boolean): Promise<void> {
    const view = sessionView.capture();
    if (!view) return;
    await setGroupsExpandedFlow(groupFlowHost, {
      view,
      sessionId: view.sessionId,
      uuids,
      expanded,
    });
  }

  async function copyEntryValue(value: string, label: string, sensitive = false): Promise<void> {
    const view = sessionView.capture();
    if (!view) return;
    try {
      await copyValue(value, sensitive);
      if (sessionView.isCurrent(view)) flash(`已复制${label}`);
    } catch {
      if (sessionView.isCurrent(view)) flash("复制失败");
    }
  }

  let entryMenu = $state<{ x: number; y: number; entry: VaultEntry } | null>(null);
  let blankMenu = $state<{ x: number; y: number } | null>(null);
  let toolbarMenu = $state<{ x: number; y: number } | null>(null);

  function openEntryMenu(event: MouseEvent, entry: VaultEntry): void {
    event.preventDefault();
    event.stopPropagation();
    blankMenu = null;
    toolbarMenu = null;
    // Right-click updates the selection (so menu actions target this entry)
    // but must not force the detail panel open.
    if (selection.selectedEntry !== entry) layout.suppressNextAutoOpen();
    if (!selection.selectedUuids.has(entry.uuid)) setSingleSelection(entry);
    selection.selectedEntry = entry;
    entryMenu = { x: event.clientX, y: event.clientY, entry };
    openContextMenu("page");
  }

  function openBlankMenu(event: MouseEvent): void {
    event.preventDefault();
    entryMenu = null;
    toolbarMenu = null;
    blankMenu = { x: event.clientX, y: event.clientY };
    openContextMenu("page");
  }

  function toggleToolbarMenu(event: MouseEvent): void {
    event.stopPropagation();
    if (toolbarMenu) {
      toolbarMenu = null;
      closeContextMenu("page");
      return;
    }
    entryMenu = null;
    blankMenu = null;
    const rect = (event.currentTarget as HTMLButtonElement).getBoundingClientRect();
    toolbarMenu = { x: rect.left, y: rect.bottom + 4 };
    openContextMenu("page");
  }

  function selectAllEntries(): void {
    selection.selectedUuids = new Set(sortedEntries.map((r) => r.entry.uuid));
    selection.selectionAnchor = null;
    selection.selectedEntry = sortedEntries[0]?.entry ?? null;
  }

  function entryMenuItems(entry: VaultEntry): ContextMenuItem[] {
    return buildEntryMenuItems({
      entry,
      selectedCount: selection.selectedUuids.size,
      isDesktop: isTauriRuntime(),
    });
  }

  const blankMenuItems = $derived<ContextMenuItem[]>(
    buildBlankMenuItems({
      hasVisibleEntries: sortedEntries.length > 0,
      canSave: Boolean(currentVault?.dirty) && !currentVault?.readOnly,
    }),
  );

  const toolbarMenuItems = $derived<ContextMenuItem[]>(
    buildToolbarMenuItems({ detailVisible: layout.detailVisible, busy }),
  );

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
    const view = sessionView.capture();
    if (!view) return;
    const { sessionId } = view;
    try {
      await vault.callInSession(sessionId, () => vault.autoType(entry.uuid, sequence));
      if (!sessionView.isCurrent(view)) return;
      flash("已最小化，请在 1.5 秒内切换到目标窗口");
    } catch (e) {
      if (sessionView.isCurrent(view)) flash(`自动填充失败：${e}`);
    }
  }

  /** Open the always-on-top two-channel overlay for manual channel injection. */
  async function openTcatoOverlay(entry: VaultEntry): Promise<void> {
    // Mark the overlay active *synchronously*: focusing it blurs the main
    // window, which would otherwise trip the focus-loss lock before the
    // backend's open event is delivered.
    const focusLockLease = beginTcatoOverlayOpen();
    const view = sessionView.capture();
    const operation = tcatoOperations.begin();
    try {
      if (!view) throw new Error("数据库未打开");
      await invoke("open_tcato_overlay", { sessionId: view.sessionId, uuid: entry.uuid });
      focusLockLease.confirm();
    } catch (e) {
      if (view && sessionView.isCurrent(view) && tcatoOperations.isCurrent(operation)) {
        flash(`TCATO 覆盖层打开失败：${e}`);
      }
    } finally {
      focusLockLease.release();
    }
  }

  function handleBlankMenuAction(id: string): void {
    if (id === "new-entry") editor.openCreate();
    else if (id === "new-group") openGroupModal(selectedGroup);
    else if (id === "import-csv") void handleImportCsv();
    else if (id === "import-xml") void handleImportXml();
    else if (id === "import-bitwarden") void handleImportBitwarden();
    else if (id === "import-1password") void handleImportOnePassword();
    else if (id === "select-all") selectAllEntries();
    else if (id === "save") void handleSave();
    else if (id === "save-as") void handleSaveAs();
    else if (id === "similar-passwords") similarOpen = true;
    else if (id === "expired-entries") expiredOpen = true;
    else if (id === "change-timeline") timelineOpen = true;
    else if (id === "hibp-check") hibpOpen = true;
    else if (id === "clear-history") void handleClearHistory();
    else if (id === "lock") void handleLock();
    else if (id === "refresh") void vault.refresh();
    else if (id === "export-csv") void handleExportCsv();
    else if (id === "export-xml") void handleExportXml();
    else if (id === "export-emergency") emergencyExportOpen = true;
    else if (id === "db-settings") dbSettingsOpen = true;
  }

  function handleToolbarMenuAction(id: string): void {
    if (id === "save-as") void handleSaveAs();
    else if (id === "toggle-detail") layout.detailVisible = !layout.detailVisible;
    else if (id === "security-report") void handleOpenReport();
    else if (id === "similar-passwords") similarOpen = true;
    else if (id === "hibp-check") hibpOpen = true;
    else if (id === "export-csv") void handleExportCsv();
    else if (id === "export-xml") void handleExportXml();
    else if (id === "export-emergency") emergencyExportOpen = true;
    else if (id === "db-settings") dbSettingsOpen = true;
    else if (id === "settings") openSettings();
  }
</script>

<svelte:head>
  <title>SecPivot</title>
</svelte:head>

<svelte:window onkeydowncapture={handleShortcutKeydown} />

{#if isTcatoOverlay}
  <TcatoOverlay />
{:else}
  <main
    class="app-shell"
    class:compact={compactMode}
    class:standalone={!currentVault}
    class:mobile-nav-open={layout.mobileNavOpen}
    style:--group-gap={compactMode ? `${groupDensity.groupGap}px` : undefined}
    style:--group-pad-y={compactMode ? `${groupDensity.groupPaddingY}px` : undefined}
    style:--group-indent={compactMode ? `${groupDensity.groupIndent}px` : undefined}
    style:--group-radius={compactMode ? `${groupDensity.groupRadius}px` : undefined}
    oncontextmenu={(e) => e.preventDefault()}
  >
    {#if currentVault}
      <AppToolbar
        bind:search
        {iconOnlyButtons}
        {toolbarOverflowMenu}
        {showWindowControls}
        {busy}
        dirty={currentVault.dirty}
        readOnly={currentVault.readOnly}
        mobileNavOpen={layout.mobileNavOpen}
        detailVisible={layout.detailVisible}
        toolbarMenuOpen={toolbarMenu !== null}
        advancedFilterActive={advancedQuery !== null}
        ontogglenav={() => (layout.mobileNavOpen = !layout.mobileNavOpen)}
        onsave={handleSave}
        onsaveas={() => void handleSaveAs()}
        onlock={handleLock}
        onnewentry={editor.openCreate}
        onclearsearch={() => (search = "")}
        onadvancedsearch={() => (advancedSearchOpen = true)}
        ontoggledetail={() => (layout.detailVisible = !layout.detailVisible)}
        onreport={() => void handleOpenReport()}
        onexportcsv={() => void handleExportCsv()}
        onsettings={openSettings}
        ontogglemenu={toggleToolbarMenu}
        bind:this={toolbarEl}
      />

      <VaultTabs />

      <div
        class="main-content"
        style={`--group-width: ${layout.groupWidth}px; --detail-width: ${layout.detailVisible ? layout.detailWidth : 0}px`}
      >
        {#if layout.mobileNavOpen}
          <button
            class="mobile-drawer-backdrop"
            aria-label="关闭分组面板"
            onclick={() => (layout.mobileNavOpen = false)}
          ></button>
        {/if}
        <section class="group-panel">
          {#key currentVaultSessionId}
            <GroupTree
              root={currentVault.root}
              selected={selectedGroup}
              reveal={revealGroupUuid}
              customIcons={currentVault.customIcons}
              showIcon={groupDensity.showGroupIcon}
              showChevron={groupDensity.showGroupChevron}
              onselect={(uuid: string | null) => {
                selectedGroup = uuid;
                selection.selectedEntry = null;
                selection.selectedUuids = new Set();
                selection.selectionAnchor = null;
                layout.mobileNavOpen = false;
              }}
              onaddsubgroup={openGroupModal}
              onrename={(uuid: string, name: string) => void renameGroup(uuid, name)}
              onchangeicon={openGroupIconDialog}
              onautotype={(uuid: string) => (groupAutoTypeUuid = uuid)}
              onmeta={(uuid: string) => (groupMetaUuid = uuid)}
              ondelete={askDeleteGroup}
              onrestore={(uuid: string) => void restoreGroup(uuid)}
              onemptybin={askEmptyRecycleBin}
              ontoggle={(uuid: string, expanded: boolean) =>
                void toggleGroupExpanded(uuid, expanded)}
              onsetexpanded={(uuids: string[], expanded: boolean) =>
                void toggleGroupsExpanded(uuids, expanded)}
              ondropentry={(groupUuid: string, uuids: string[]) =>
                void moveEntriesTo(groupUuid, uuids)}
            />
          {/key}
        </section>

        <span
          class="group-resize-handle"
          role="separator"
          aria-orientation="vertical"
          title="调整分组宽度"
          onpointerdown={layout.startGroupResize}
        ></span>

        <section class="entry-panel">
          <EntryTable
            rows={sortedEntries}
            visibleCols={columns.visibleCols}
            entryGridCols={columns.entryGridCols}
            {sortCol}
            {sortDir}
            selectedUuids={selection.selectedUuids}
            {showDescriptions}
            compact={compactMode}
            searchActive={Boolean(search)}
            {customIconUrl}
            {entryIconName}
            colText={columns.columnText}
            onrevealpassword={revealEntryPassword}
            oncyclesort={cycleSort}
            oncolumnresize={columns.resizeEntryColumn}
            oncolumnreorder={columns.applyColumnReorder}
            onsavelayout={layout.saveLayout}
            onrowclick={selection.handleRowClick}
            onentrycontextmenu={openEntryMenu}
            oncolumncontextmenu={openColumnMenu}
            onblankcontextmenu={openBlankMenu}
            onselectall={selectAllEntries}
            onselectentry={setSingleSelection}
            onfavorite={(entry) => void toggleFavorite(entry)}
            oncopyusername={(entry) => {
              if (entry.username) void copyEntryValue(entry.username, "用户名");
            }}
            oncopypassword={(entry) => void copyEntryPassword(entry)}
          />
        </section>

        {#if layout.detailVisible}
          <span
            class="detail-resize-handle"
            role="separator"
            aria-orientation="vertical"
            title="调整详情宽度"
            onpointerdown={layout.startDetailResize}
          ></span>

          <section class="detail-panel">
            {#if selection.selectedEntry}
              <EntryDetail
                entry={selection.selectedEntry}
                groupPath={pathOf(selection.selectedEntry.groupUuid)}
                inRecycleBin={groupInBin(selection.selectedEntry.groupUuid)}
                onfavorite={toggleFavorite}
                onedit={openEditEntry}
                ondelete={askDeleteEntry}
                onrestore={(entry: VaultEntry) => void restoreEntry(entry)}
                onback={() => {
                  selection.selectedEntry = null;
                  selection.selectedUuids = new Set();
                  selection.selectionAnchor = null;
                }}
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
          <span class="result-count" title="all={allEntries.length} filtered={filteredEntries.length} groups={allGroups.length} subtree={selectedSubtree.length}">{filteredEntries.length} 个条目</span>
          <span class="status-group-filter" title="debug all={allEntries.length} filtered={filteredEntries.length} groups={allGroups.length} subtree={selectedSubtree.length} search='{search}' adv={advancedQuery ? 'on' : 'off'}">[调试 {allEntries.length}/{filteredEntries.length}]</span>
          {#if selection.selectedUuids.size > 1}
            <span class="status-group-filter">已选 {selection.selectedUuids.size} 个</span>
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
        <span class="status-right">
          {#if currentVault.path}
            <button
              class="status-path"
              title={currentVault.path}
              onclick={() => (dbMetaOpen = true)}
            >
              {currentVault.fileName}
            </button>
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

{#if editor.editorOpen}
  <EntryEditorDialog
    mode={editor.editorMode}
    groups={currentVault ? [currentVault.root] : []}
    groupUuid={selectedGroup ?? currentVault?.root.uuid ?? "root"}
    entry={editor.editEntry}
    entries={editor.editEntries}
    onclose={editor.close}
    onsaved={(input, patch, autotype, flags) => editor.handleSave(input, patch, autotype, flags)}
  />
{/if}

{#if reportOpen && securityReport}
  <SecurityReportDialog
    report={securityReport}
    entries={reportEntries}
    onclose={() => (reportOpen = false)}
  />
{/if}

{#if similarOpen}
  <SimilarPasswordsDialog
    onclose={() => (similarOpen = false)}
    onselect={(uuid: string) => {
      const target = currentVault ? findEntryByUuid(currentVault, uuid) : null;
      if (target) {
        setSingleSelection(target);
        similarOpen = false;
      }
    }}
  />
{/if}

{#if expiredOpen}
  <ExpiredEntriesDialog
    onclose={() => (expiredOpen = false)}
    onselect={(uuid: string) => {
      const target = currentVault ? findEntryByUuid(currentVault, uuid) : null;
      if (target) {
        setSingleSelection(target);
        expiredOpen = false;
      }
    }}
  />
{/if}

{#if timelineOpen}
  <ChangeTimelineDialog
    onclose={() => (timelineOpen = false)}
    onselect={(uuid: string) => {
      const target = currentVault ? findEntryByUuid(currentVault, uuid) : null;
      if (target) {
        setSingleSelection(target);
        timelineOpen = false;
      }
    }}
  />
{/if}

{#if hibpOpen}
  <HibpCheckDialog
    uuids={[...selection.selectedUuids]}
    onclose={() => (hibpOpen = false)}
    onselect={(uuid: string) => {
      const target = currentVault ? findEntryByUuid(currentVault, uuid) : null;
      if (target) {
        setSingleSelection(target);
        hibpOpen = false;
      }
    }}
  />
{/if}

{#if dbMetaOpen && currentVault}
  <DbMetaDialog
    name={currentVault.databaseName ?? ""}
    description={currentVault.databaseDescription ?? ""}
    onclose={() => (dbMetaOpen = false)}
  />
{/if}

{#if dbSettingsOpen}
  <DatabaseSettingsDialog onclose={() => (dbSettingsOpen = false)} />
{/if}

{#if advancedSearchOpen}
  <AdvancedSearchDialog
    initialQuery={advancedQuery}
    onapply={(query) => (advancedQuery = query)}
    onclear={() => (advancedQuery = null)}
    onclose={() => (advancedSearchOpen = false)}
  />
{/if}

{#if columnMenu}
  <ColumnConfigMenu
    x={columnMenu.x}
    y={columnMenu.y}
    sections={columnMenuSections}
    onclose={() => {
      columnMenu = null;
      closeContextMenu("page");
    }}
    ontoggle={columns.toggleColumn}
  />
{/if}

{#if groupModalOpen}
  <ModalShell
    title="新建分组"
    description={`在${groupModalParent ? pathOf(groupModalParent) : "根"}下创建子分组`}
    size="small"
    closeOnEscape
    onclose={() => (groupModalOpen = false)}
  >
    {#snippet icon()}<AppIcon name="folder-plus" size={18} />{/snippet}
    {#snippet children()}
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
    {/snippet}
    {#snippet actions()}
      <button class="modal-button" onclick={() => (groupModalOpen = false)}>取消</button>
      <button
        class="modal-button primary"
        onclick={() => void confirmCreateGroup()}
        disabled={!newGroupName.trim() || groupCreating}
      >
        创建
      </button>
    {/snippet}
  </ModalShell>
{/if}

{#if groupIconDialogUuid}
  <ModalShell
    title="设置分组图标"
    description="选择内置图标,点击保存后生效"
    size="small"
    closeOnEscape
    onclose={() => (groupIconDialogUuid = null)}
  >
    {#snippet icon()}<AppIcon name="palette" size={18} />{/snippet}
    {#snippet children()}
      <span class="group-icon-label">图标</span>
      <div class="group-icon-grid">
        {#each KEEPASS_ICON_CHOICES as index}
          <button
            type="button"
            class="icon-option"
            class:selected={groupIconPick === index}
            onclick={() => (groupIconPick = groupIconPick === index ? null : index)}
            title={`内置图标 ${index}`}
            aria-pressed={groupIconPick === index}
          >
            <AppIcon name={groupIconName(index)} size={16} />
          </button>
        {/each}
      </div>
    {/snippet}
    {#snippet actions()}
      <button class="modal-button" onclick={() => (groupIconDialogUuid = null)}>取消</button>
      <button
        class="modal-button primary"
        onclick={() => void confirmChangeGroupIcon()}
        disabled={groupIconSaving}
      >
        保存
      </button>
    {/snippet}
  </ModalShell>
{/if}

{#if groupAutoTypeUuid && currentVault}
  {@const group = findGroupIn(currentVault.root, groupAutoTypeUuid)}
  {#if group}
    <GroupAutoTypeDialog {group} onclose={() => (groupAutoTypeUuid = null)} />
  {/if}
{/if}

{#if groupMetaUuid && currentVault}
  {@const group = findGroupIn(currentVault.root, groupMetaUuid)}
  {#if group}
    <GroupMetaDialog {group} onclose={() => (groupMetaUuid = null)} onsaved={saveGroupMeta} />
  {/if}
{/if}

{#if emergencyExportOpen}
  <ModalShell
    title="导出 HTML 应急表"
    description="生成可离线打开、可直接打印的应急文件"
    size="small"
    closeOnEscape
    onclose={() => (emergencyExportOpen = false)}
  >
    {#snippet children()}
      <div class="export-warning">
        <p>
          导出的 HTML
          是明文文件。若勾选包含密码，文件将写入所有条目的明文密码，请妥善保管并在使用后删除。
        </p>
        <label class="export-check">
          <input type="checkbox" bind:checked={emergencyIncludePasswords} />
          包含密码（强烈建议不勾选）
        </label>
      </div>
    {/snippet}
    {#snippet actions()}
      <button class="modal-button" onclick={() => (emergencyExportOpen = false)}>取消</button>
      <button class="modal-button primary" onclick={() => void confirmExportEmergency()}
        >导出</button
      >
    {/snippet}
  </ModalShell>
{/if}

{#if remoteConflict}
  <ModalShell
    title="远程库已变更"
    description="保存前检测到远程版本已被其他设备修改"
    size="small"
    closeOnEscape
    onclose={() => (remoteConflict = null)}
  >
    {#snippet children()}
      <p class="conflict-note">{remoteConflict}</p>
    {/snippet}
    {#snippet actions()}
      <button class="modal-button" onclick={() => (remoteConflict = null)}>取消（保留本地）</button>
      <button class="modal-button" onclick={() => void resolveRemoteConflict("merge")}>
        合并本地与远程
      </button>
      <button class="modal-button" onclick={() => void resolveRemoteConflict("download")}>
        下载远程
      </button>
      <button class="modal-button danger" onclick={() => void resolveRemoteConflict("overwrite")}>
        覆盖远程
      </button>
    {/snippet}
  </ModalShell>
{/if}

{#if confirmState}
  <ModalShell
    title="确认删除"
    description={confirmState.message}
    ariaLabel="确认操作"
    size="confirm"
    tone="danger"
    closeOnEscape
    onclose={() => (confirmState = null)}
  >
    {#snippet icon()}<AppIcon name="trash" size={18} />{/snippet}
    {#snippet actions()}
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
    {/snippet}
  </ModalShell>
{/if}

{#if entryMenu}
  <ContextMenu
    x={entryMenu.x}
    y={entryMenu.y}
    items={entryMenuItems(entryMenu.entry)}
    onclose={() => {
      entryMenu = null;
      closeContextMenu("page");
    }}
    onaction={(id) => {
      const menuEntry = entryMenu!.entry;
      entryMenu = null;
      closeContextMenu("page");
      handleEntryMenuAction(id, menuEntry);
    }}
  />
{/if}

{#if blankMenu}
  <ContextMenu
    x={blankMenu.x}
    y={blankMenu.y}
    items={blankMenuItems}
    onclose={() => {
      blankMenu = null;
      closeContextMenu("page");
    }}
    onaction={(id) => {
      blankMenu = null;
      closeContextMenu("page");
      handleBlankMenuAction(id);
    }}
  />
{/if}

{#if toolbarMenu}
  <ContextMenu
    x={toolbarMenu.x}
    y={toolbarMenu.y}
    items={toolbarMenuItems}
    onclose={() => {
      toolbarMenu = null;
      closeContextMenu("page");
    }}
    onaction={(id) => {
      toolbarMenu = null;
      closeContextMenu("page");
      handleToolbarMenuAction(id);
    }}
  />
{/if}

{#if autotypePick}
  <AutotypePickDialog
    candidates={autotypePick}
    onclose={() => (autotypePick = null)}
    onerror={(message) => flash(message)}
  />
{/if}

{#if faviconDialog}
  <FaviconProgressDialog dialog={faviconDialog} onclose={() => (faviconDialog = null)} />
{/if}

<style>
  .app-shell {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: auto minmax(0, 1fr) auto;
    width: 100%;
    height: 100vh;
    min-width: 760px;
    overflow: hidden;
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

  .mobile-drawer-backdrop {
    display: none;
  }

  .main-content {
    display: grid;
    grid-template-columns: var(--group-width, 200px) minmax(0, 1fr);
    min-height: 0;
    position: relative;
  }

  .group-panel {
    display: flex;
    flex-direction: column;
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

  .status-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 220px;
    padding: 1px 6px;
    border: 1px solid transparent;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
    font: inherit;
    font-size: var(--font-size-tiny, 10px);
    cursor: pointer;
  }

  .status-path:hover {
    color: var(--text-secondary);
    background: var(--hover-bg);
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

  .group-icon-grid .icon-option {
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

  .group-icon-grid .icon-option:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .group-icon-grid .icon-option.selected {
    color: var(--accent-color, var(--primary-color));
    border-color: color-mix(in srgb, var(--primary-color) 55%, transparent);
    background: color-mix(in srgb, var(--primary-color) 12%, transparent);
  }

  @media (max-width: 720px) {
    .app-shell,
    .app-shell.compact {
      min-width: 0;
      height: 100dvh;
    }

    .main-content {
      display: block;
      position: relative;
      min-width: 0;
      min-height: 0;
      height: 100%;
    }

    .mobile-drawer-backdrop {
      position: absolute;
      inset: 0;
      z-index: 5;
      display: block;
      padding: 0;
      border: 0;
      background: color-mix(in srgb, #000 35%, transparent);
    }

    .group-panel {
      position: absolute;
      top: 0;
      bottom: 0;
      left: 0;
      width: min(82vw, 320px);
      z-index: 6;
      box-shadow: 0 0 24px rgba(0, 0, 0, 0.3);
      border-right: 1px solid var(--border-color);
      transform: translateX(-100%);
      transition: transform 0.16s ease;
    }

    .app-shell.mobile-nav-open .group-panel {
      transform: translateX(0);
    }

    .entry-panel {
      min-width: 0;
      width: 100%;
      height: 100%;
    }

    .group-resize-handle,
    .detail-resize-handle {
      display: none;
    }

    .detail-panel {
      position: absolute;
      top: 0;
      bottom: 0;
      left: 0;
      width: 100%;
      border-left: 1px solid var(--border-color);
    }

    .status-bar {
      flex-wrap: wrap;
      gap: 4px 12px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .group-panel {
      transition: none;
    }
  }

  :global(body.resizing-column) {
    cursor: col-resize !important;
    user-select: none;
  }

  .export-warning p {
    margin: 0 0 10px;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    line-height: 1.5;
  }

  .export-check {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .conflict-note {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    line-height: 1.6;
    white-space: pre-wrap;
  }
</style>
