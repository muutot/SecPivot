<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { get } from "svelte/store";
  import { vault } from "$lib/services/vault";
  import { appSettings } from "$lib/services/settings";
  import { syncCompactShellClass } from "$lib/services/settings-bootstrap";
  import { armIdleLock, installAutoLock, lockVault, copySensitive } from "$lib/services/security";
  import { copyText } from "$lib/utils/clipboard";
  import type { EntryInput, VaultEntry, VaultGroup, VaultState } from "$lib/types/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ContextMenu, { type ContextMenuItem } from "$lib/components/ContextMenu.svelte";
  import VaultWelcome from "$lib/components/VaultWelcome.svelte";
  import LockScreen from "$lib/components/LockScreen.svelte";
  import GroupTree from "$lib/components/GroupTree.svelte";
  import EntryDetail from "$lib/components/EntryDetail.svelte";
  import EntryEditorDialog from "$lib/components/EntryEditorDialog.svelte";

  let currentVault = $state<VaultState | null>(null);
  let rememberedPath = $state<{ path: string; fileName: string } | null>(null);
  let search = $state("");
  let selectedGroup = $state<string | null>(null);
  let selectedEntry = $state<VaultEntry | null>(null);
  let selectedUuids = $state<Set<string>>(new Set());
  let selectionAnchor = $state<string | null>(null);
  let editorOpen = $state(false);
  let editorMode: "create" | "edit" = $state("create");
  let editEntry: VaultEntry | null = $state(null);
  let groupModalOpen = $state(false);
  let groupModalParent = $state<string | null>(null);
  let newGroupName = $state("");
  let confirmState = $state<{ message: string; onconfirm: () => void } | null>(null);
  let statusMsg = $state("");
  let busy = $state(false);

  let statusTimer: ReturnType<typeof setTimeout> | undefined = $state();

  onMount(() => {
    const unsubscribe = vault.subscribe((value) => {
      currentVault = value;
      if (!value) {
        selectedEntry = null;
        selectedUuids = new Set();
        selectionAnchor = null;
        editorOpen = false;
      }
      armIdleLock();
    });
    const unsubRemembered = vault.remembered((value) => {
      rememberedPath = value;
    });
    void vault.refresh();
    const stopAutoLock = installAutoLock();
    return () => {
      unsubscribe();
      unsubRemembered();
      stopAutoLock();
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

  const compactMode = $derived(get(appSettings).general.compactMode);
  const groupDensity = $derived(get(appSettings).general.density);
  const showDescriptions = $derived(get(appSettings).general.showDescriptions);
  const showLockScreen = $derived(
    !currentVault && rememberedPath !== null && get(appSettings).general.rememberLastDatabase,
  );
  $effect(() => {
    syncCompactShellClass(compactMode);
  });

  const allGroups = $derived.by((): VaultGroup[] => {
    if (!currentVault) return [];
    const list: VaultGroup[] = [];
    function walk(group: VaultGroup): void {
      list.push(group);
      for (const child of group.children) walk(child);
    }
    walk(currentVault.root);
    return list;
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

  const selectedSubtree = $derived.by((): VaultGroup[] => {
    if (!currentVault) return [];
    if (selectedGroup === null) return allGroups;
    const group = allGroups.find((g) => g.uuid === selectedGroup);
    if (!group) return allGroups;
    const list: VaultGroup[] = [];
    function collect(g: VaultGroup): void {
      list.push(g);
      for (const child of g.children) collect(child);
    }
    collect(group);
    return list;
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

  type SortCol = "title" | "url";
  let sortCol = $state<SortCol>("title");
  let sortDir = $state<"asc" | "desc">("asc");
  let colWidths = $state<{ url: number }>({ url: 200 });
  let groupWidth = $state(200);
  let detailWidth = $state(300);
  let detailVisible = $state(false);

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
      const av = a.entry[col] ?? "";
      const bv = b.entry[col] ?? "";
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

  function startResize(e: PointerEvent): void {
    e.preventDefault();
    e.stopPropagation();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const startX = e.clientX;
    const startW = colWidths.url;
    document.body.classList.add("resizing-column");
    const onMove = (ev: PointerEvent): void => {
      colWidths.url = Math.min(400, Math.max(100, startW - (ev.clientX - startX)));
    };
    const onUp = (ev: PointerEvent): void => {
      if (target.hasPointerCapture(ev.pointerId)) target.releasePointerCapture(ev.pointerId);
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      document.body.classList.remove("resizing-column");
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
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
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function findEntryByUuid(state: VaultState | null, uuid: string | null): VaultEntry | null {
    if (!state || !uuid) return null;
    for (const group of allGroupsOf(state.root)) {
      const found = group.entries.find((e) => e.uuid === uuid);
      if (found) return found;
    }
    return null;
  }

  function allGroupsOf(root: VaultGroup): VaultGroup[] {
    const list: VaultGroup[] = [];
    function walk(group: VaultGroup): void {
      list.push(group);
      for (const child of group.children) walk(child);
    }
    walk(root);
    return list;
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
    editorOpen = true;
  }

  function openEditEntry(entry: VaultEntry): void {
    editorMode = "edit";
    editEntry = entry;
    editorOpen = true;
  }

  async function handleEditorSave(input: EntryInput): Promise<void> {
    try {
      if (editorMode === "create") {
        const state = await vault.addEntry(input);
        setSingleSelection(findEntryByUuid(state, null));
        editorOpen = false;
        flash("已创建条目");
      } else if (editEntry) {
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
    groupModalOpen = true;
  }

  async function confirmCreateGroup(): Promise<void> {
    const name = newGroupName.trim();
    if (!name) return;
    try {
      await vault.addGroup({ parentUuid: groupModalParent, name });
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
    confirmState = {
      message: "删除该分组？其下条目将移动到根分组。",
      onconfirm: async () => {
        try {
          await vault.deleteGroup(uuid);
          if (selectedGroup === uuid) selectedGroup = null;
          flash("已删除分组");
        } catch (e) {
          flash(`删除失败：${e}`);
        }
      },
    };
  }

  function askDeleteEntry(entry: VaultEntry): void {
    confirmState = {
      message: `删除条目「${entry.title || "未命名"}」？`,
      onconfirm: async () => {
        try {
          await vault.deleteEntry(entry.uuid);
          if (selectedEntry?.uuid === entry.uuid) selectedEntry = null;
          if (selectedUuids.has(entry.uuid)) {
            const next = new Set(selectedUuids);
            next.delete(entry.uuid);
            selectedUuids = next;
          }
          flash("已删除条目");
        } catch (e) {
          flash(`删除失败：${e}`);
        }
      },
    };
  }

  async function copyEntryValue(value: string, label: string, sensitive = false): Promise<void> {
    try {
      if (sensitive) {
        await copySensitive(value);
      } else {
        await copyText(value);
      }
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
    return [
      { id: "edit", label: "编辑条目", icon: "edit" },
      { id: "copy-username", label: "复制用户名", icon: "user", disabled: !entry.username },
      { id: "copy-password", label: "复制密码", icon: "copy", disabled: !entry.password },
      { id: "copy-url", label: "复制网址", icon: "link", disabled: !entry.url },
      { id: "favorite", label: entry.favorite ? "取消收藏" : "收藏条目", icon: "star" },
      { id: "delete", label: "删除条目", icon: "trash", destructive: true },
    ];
  }

  const blankMenuItems = $derived<ContextMenuItem[]>([
    { id: "new-entry", label: "新建条目", icon: "plus" },
    { id: "new-group", label: "新建分组", icon: "folder-plus" },
    { id: "select-all", label: "全选条目", icon: "check", disabled: sortedEntries.length === 0 },
    { id: "save", label: "保存数据库", icon: "save", disabled: !currentVault?.dirty },
    { id: "lock", label: "锁定数据库", icon: "lock" },
    { id: "refresh", label: "刷新", icon: "refresh" },
  ]);

  function handleEntryMenuAction(id: string, entry: VaultEntry): void {
    if (id === "edit") openEditEntry(entry);
    else if (id === "copy-username" && entry.username)
      void copyEntryValue(entry.username, "用户名");
    else if (id === "copy-password" && entry.password)
      void copyEntryValue(entry.password, "密码", true);
    else if (id === "copy-url" && entry.url) void copyEntryValue(entry.url, "网址");
    else if (id === "favorite") void toggleFavorite(entry);
    else if (id === "delete") askDeleteEntry(entry);
  }

  function handleBlankMenuAction(id: string): void {
    if (id === "new-entry") openCreateEntry();
    else if (id === "new-group") openGroupModal(selectedGroup);
    else if (id === "select-all") selectAllEntries();
    else if (id === "save") void handleSave();
    else if (id === "lock") void handleLock();
    else if (id === "refresh") void vault.refresh();
  }
</script>

<svelte:head>
  <title>KeyVault</title>
</svelte:head>

<main
  class="app-shell"
  class:compact={compactMode}
  style:--group-gap={compactMode ? `${groupDensity.groupGap}px` : undefined}
  style:--group-pad-y={compactMode ? `${groupDensity.groupPaddingY}px` : undefined}
  style:--group-indent={compactMode ? `${groupDensity.groupIndent}px` : undefined}
  style:--group-radius={compactMode ? `${groupDensity.groupRadius}px` : undefined}
>
  {#if currentVault}
    <div class="toolbar" role="presentation" data-tauri-drag-region>
      <div class="toolbar-left">
        <button class="tool-button primary" onclick={openCreateEntry} title="新建条目 (Ctrl+N)">
          <AppIcon name="plus" size={14} />条目
        </button>
        <button class="tool-button" onclick={() => openGroupModal(selectedGroup)} title="新建分组">
          <AppIcon name="folder-plus" size={14} />分组
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
        <button class="icon-action" onclick={openSettings} title="设置">
          <AppIcon name="settings" size={16} />
        </button>
        <button
          class="tool-button"
          onclick={handleSave}
          disabled={busy || !currentVault.dirty}
          title="保存数据库 (Ctrl+S)"
        >
          <AppIcon name="save" size={14} />保存
        </button>
        <button class="tool-button" onclick={handleLock} title="锁定数据库">
          <AppIcon name="lock" size={14} />锁定
        </button>
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
        <div class="entry-head">
          <span class="entry-count">{filteredEntries.length} 个条目</span>
          {#if selectedGroup !== null}
            <span class="entry-group-filter">筛选于 {pathOf(selectedGroup)}</span>
          {/if}
        </div>

        <div class="entry-table" style={`--col-url: ${colWidths.url}px`}>
          <div class="entry-table-head" role="row">
            <div class="head-cell head-title">
              <button
                class="head-button"
                type="button"
                onclick={() => cycleSort("title")}
                title="点击排序"
              >
                <span class="head-label">标题</span>
                {#if sortCol === "title"}
                  <span class="sort-arrow" aria-hidden="true">{sortDir === "asc" ? "▲" : "▼"}</span>
                {/if}
              </button>
              <span
                class="resize-handle"
                role="separator"
                aria-orientation="vertical"
                title="调整列宽"
                onpointerdown={(e) => startResize(e)}
              ></span>
            </div>
            <div class="head-cell head-url">
              <button
                class="head-button"
                type="button"
                onclick={() => cycleSort("url")}
                title="点击排序"
              >
                <span class="head-label">网址</span>
                {#if sortCol === "url"}
                  <span class="sort-arrow" aria-hidden="true">{sortDir === "asc" ? "▲" : "▼"}</span>
                {/if}
              </button>
            </div>
            <div class="head-actions"></div>
          </div>

          <div
            class="entry-list"
            role="listbox"
            aria-label="条目列表"
            aria-multiselectable="true"
            tabindex="-1"
            oncontextmenu={openBlankMenu}
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
                  role="option"
                  aria-selected={selectedUuids.has(row.entry.uuid)}
                  tabindex="0"
                  onclick={(e) => handleRowClick(e, row.entry)}
                  oncontextmenu={(e) => openEntryMenu(e, row.entry)}
                  onkeydown={(e) => {
                    if (e.key === "Enter") setSingleSelection(row.entry);
                  }}
                >
                  <span class="entry-row-icon"><AppIcon name="key" size={13} /></span>
                  <div class="entry-row-main">
                    <span class="entry-row-title">{row.entry.title || "未命名条目"}</span>
                    {#if showDescriptions}
                      <span class="entry-row-sub">{row.entry.username}</span>
                    {/if}
                  </div>
                  <span class="entry-row-col col-url" title={row.entry.url || undefined}>
                    {row.entry.url}
                  </span>
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
                        if (row.entry.password)
                          void copyEntryValue(row.entry.password, "密码", true);
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
              onfavorite={toggleFavorite}
              onedit={openEditEntry}
              ondelete={askDeleteEntry}
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

    <footer class="status-bar" role="status" aria-live="polite">
      <span class="status-left">
        <span class="result-count">{filteredEntries.length} 条</span>
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
    <LockScreen
      remembered={rememberedPath}
      onopened={() => void vault.refresh()}
      onswitch={() => vault.clearRemembered()}
    />
  {:else}
    <VaultWelcome onopened={() => void vault.refresh()} />
  {/if}
</main>

{#if editorOpen}
  <EntryEditorDialog
    mode={editorMode}
    groups={currentVault?.root.children ?? []}
    groupUuid={selectedGroup ?? currentVault?.root.uuid ?? "root"}
    entry={editEntry}
    onclose={() => (editorOpen = false)}
    onsaved={(input) => void handleEditorSave(input)}
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

<style>
  .app-shell {
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

  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 6px 14px;
    border-top: 1px solid var(--border-subtle);
    border-bottom: 1px solid var(--border-subtle);
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

  .dirty-badge {
    padding: 2px 7px;
    border: 1px solid color-mix(in srgb, var(--warning-color) 45%, transparent);
    border-radius: 10px;
    color: var(--warning-color);
    font-size: var(--font-size-tiny, 10px);
  }

  .main-content {
    display: grid;
    grid-template-columns: var(--group-width, 200px) minmax(0, 1fr) var(--detail-width, 300px);
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

  .entry-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 8px 12px 6px;
    flex: 0 0 auto;
  }

  .entry-count {
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }

  .entry-group-filter {
    overflow: hidden;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-table {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }

  .entry-table-head {
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) var(--col-url, 200px) 70px;
    align-items: center;
    gap: 9px;
    flex: 0 0 auto;
    height: 28px;
    padding: 0 10px;
    border-bottom: 1px solid var(--border-subtle);
    background: var(--surface-bg);
  }

  .head-cell {
    position: relative;
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    height: 100%;
    color: var(--text-secondary);
    font-size: var(--font-size-tiny, 10px);
    font-weight: 600;
    cursor: pointer;
    user-select: none;
  }

  .head-title {
    grid-column: 1 / 3;
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
  }

  .entry-list {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 0 0 16px;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .entry-row {
    display: grid;
    grid-template-columns: 24px minmax(0, 1fr) var(--col-url, 200px) 70px;
    align-items: center;
    gap: 9px;
    height: 40px;
    padding: 0 10px;
    cursor: pointer;
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

  .entry-row-main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .entry-row-title {
    overflow: hidden;
    color: var(--text-primary);
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-row-sub {
    overflow: hidden;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-row-col {
    overflow: hidden;
    min-width: 0;
    font-size: 12px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .col-url {
    color: var(--text-faint);
  }

  .entry-row-actions {
    display: flex;
    justify-content: flex-end;
    gap: 2px;
    min-width: 0;
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
    position: relative;
    min-height: 0;
    min-width: 0;
    border-left: 1px solid var(--border-subtle);
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

  :global(body.resizing-column) {
    cursor: col-resize !important;
    user-select: none;
  }
</style>
