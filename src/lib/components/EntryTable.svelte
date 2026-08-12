<script lang="ts">
  import type { VaultEntry } from "$lib/types/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import type { IconName } from "$lib/components/AppIcon.svelte";
  import EntryTotpBadge from "$lib/components/EntryTotpBadge.svelte";

  export interface EntryTableColumn {
    id: string;
    label: string;
    width: number;
    sortable: boolean;
  }

  export interface EntryTableRow {
    entry: VaultEntry;
  }

  interface Props {
    rows: EntryTableRow[];
    visibleCols: EntryTableColumn[];
    entryGridCols: string;
    sortCol: string;
    sortDir: "asc" | "desc";
    selectedUuids: Set<string>;
    showDescriptions: boolean;
    compact: boolean;
    searchActive: boolean;
    customIconUrl: (entry: VaultEntry) => string | undefined;
    entryIconName: (entry: VaultEntry) => IconName;
    colText: (entry: VaultEntry, colId: string) => string;
    oncyclesort: (colId: string) => void;
    oncolumnresize: (colId: string, width: number) => void;
    oncolumnreorder: (colId: string, toIndex: number) => void;
    onsavelayout: () => void;
    onrowclick: (event: MouseEvent, entry: VaultEntry) => void;
    onentrycontextmenu: (event: MouseEvent, entry: VaultEntry) => void;
    oncolumncontextmenu: (event: MouseEvent) => void;
    onblankcontextmenu: (event: MouseEvent) => void;
    onselectall: () => void;
    onselectentry: (entry: VaultEntry) => void;
    onfavorite: (entry: VaultEntry) => void;
    oncopyusername: (entry: VaultEntry) => void;
    oncopypassword: (entry: VaultEntry) => void;
  }

  let {
    rows,
    visibleCols,
    entryGridCols,
    sortCol,
    sortDir,
    selectedUuids,
    showDescriptions,
    compact,
    searchActive,
    customIconUrl,
    entryIconName,
    colText,
    oncyclesort,
    oncolumnresize,
    oncolumnreorder,
    onsavelayout,
    onrowclick,
    onentrycontextmenu,
    oncolumncontextmenu,
    onblankcontextmenu,
    onselectall,
    onselectentry,
    onfavorite,
    oncopyusername,
    oncopypassword,
  }: Props = $props();

  const COL_WIDTH_MIN = 30;
  const COL_WIDTH_MAX = 400;

  let entryHeadEl = $state<HTMLElement>();
  let colDrag = $state<{ id: string; fromIndex: number } | null>(null);
  let colDropIndex = $state<number | null>(null);
  let suppressColumnSort = $state(false);

  function startColResize(event: PointerEvent, colId: string): void {
    event.preventDefault();
    event.stopPropagation();
    const target = event.currentTarget as HTMLElement;
    target.setPointerCapture(event.pointerId);
    const startX = event.clientX;
    const cellEl = target.parentElement;
    const renderWidth = cellEl ? Math.round(cellEl.getBoundingClientRect().width) : 0;
    const storedWidth = visibleCols.find((column) => column.id === colId)?.width ?? 0;
    const startWidth = storedWidth > 0 ? storedWidth : Math.max(renderWidth, COL_WIDTH_MIN);
    document.body.classList.add("resizing-column");
    const onMove = (moveEvent: PointerEvent): void => {
      const width = Math.min(
        COL_WIDTH_MAX,
        Math.max(COL_WIDTH_MIN, startWidth + (moveEvent.clientX - startX)),
      );
      oncolumnresize(colId, width);
    };
    const onUp = (upEvent: PointerEvent): void => {
      if (target.hasPointerCapture(upEvent.pointerId)) {
        target.releasePointerCapture(upEvent.pointerId);
      }
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      document.body.classList.remove("resizing-column");
      onsavelayout();
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function startColDrag(event: PointerEvent, colId: string, fromIndex: number): void {
    if (event.button !== 0) return;
    const startX = event.clientX;
    const startY = event.clientY;
    let active = false;
    const onMove = (moveEvent: PointerEvent): void => {
      if (!active && Math.hypot(moveEvent.clientX - startX, moveEvent.clientY - startY) > 4) {
        active = true;
        colDrag = { id: colId, fromIndex };
        colDropIndex = fromIndex;
        document.body.classList.add("dragging-column");
      }
      if (active) colDropIndex = computeColumnDropIndex(moveEvent.clientX);
    };
    const onUp = (): void => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      document.body.classList.remove("dragging-column");
      if (active) {
        oncolumnreorder(colId, colDropIndex ?? fromIndex);
        suppressColumnSort = true;
        setTimeout(() => (suppressColumnSort = false), 50);
        onsavelayout();
        colDrag = null;
        colDropIndex = null;
      }
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  }

  function computeColumnDropIndex(clientX: number): number {
    const cells = Array.from(entryHeadEl?.querySelectorAll<HTMLElement>(".head-cell") ?? []);
    let index = cells.length;
    for (let i = 0; i < cells.length; i++) {
      const rect = cells[i].getBoundingClientRect();
      if (clientX < rect.left + rect.width / 2) {
        index = i;
        break;
      }
    }
    return index;
  }

  function startEntryDrag(event: DragEvent, entry: VaultEntry): void {
    const targets = selectedUuids.has(entry.uuid) ? Array.from(selectedUuids) : [entry.uuid];
    event.dataTransfer?.setData("application/x-secpivot-entries", JSON.stringify(targets));
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }
</script>

<div class="entry-table" class:compact style={`--entry-cols: ${entryGridCols}`}>
  <div
    class="entry-table-head"
    role="row"
    tabindex="-1"
    oncontextmenu={oncolumncontextmenu}
    bind:this={entryHeadEl}
  >
    <span class="head-icon-col" aria-hidden="true"></span>
    {#each visibleCols as col, i (col.id)}
      <div
        class="head-cell"
        class:head-sortable={col.sortable}
        class:col-dragging={colDrag?.id === col.id}
        class:drop-before={colDrag && colDropIndex === i}
        class:drop-after={colDrag && colDropIndex === i + 1}
        role="presentation"
        onpointerdown={(event) => startColDrag(event, col.id, i)}
      >
        <button
          class="head-button"
          type="button"
          onclick={() => {
            if (suppressColumnSort) return;
            if (col.sortable) oncyclesort(col.id);
          }}
          title={col.sortable ? "点击排序,按住拖动调整顺序" : "按住拖动调整顺序"}
        >
          <span class="head-label">{col.label}</span>
          {#if sortCol === col.id}
            <span class="sort-arrow" aria-hidden="true">{sortDir === "asc" ? "▲" : "▼"}</span>
          {/if}
        </button>
        <span
          class="resize-handle"
          role="separator"
          aria-orientation="vertical"
          title="调整列宽"
          onpointerdown={(event) => startColResize(event, col.id)}
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
    oncontextmenu={onblankcontextmenu}
    onkeydown={(event) => {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
        event.preventDefault();
        onselectall();
      }
    }}
  >
    {#if rows.length === 0}
      <div class="empty-state">
        <span class="empty-icon"><AppIcon name="key" size={20} /></span>
        <strong>{searchActive ? "没有匹配的条目" : "这个分组还没有条目"}</strong>
        <p>{searchActive ? "尝试调整搜索关键词" : "点击右上角「条目」新建一条"}</p>
      </div>
    {:else}
      {#each rows as row (row.entry.uuid)}
        <div
          class="entry-row"
          class:selected={selectedUuids.has(row.entry.uuid)}
          class:expired-row={row.entry.expired}
          style:--row-color={row.entry.color ?? "transparent"}
          role="option"
          aria-selected={selectedUuids.has(row.entry.uuid)}
          tabindex="0"
          draggable="true"
          ondragstart={(event) => startEntryDrag(event, row.entry)}
          onclick={(event) => onrowclick(event, row.entry)}
          oncontextmenu={(event) => onentrycontextmenu(event, row.entry)}
          onkeydown={(event) => {
            if (event.key === "Enter") onselectentry(row.entry);
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
          <span class="mobile-entry-summary">
            <span class="entry-row-main">
              <span class="entry-row-title" title={row.entry.expired ? "已过期" : undefined}
                >{row.entry.title || "未命名条目"}{#if row.entry.expired}
                  <span class="expired-flag">已过期</span>
                {/if}</span
              >
              {#if showDescriptions}
                <span class="entry-row-sub">{row.entry.username}</span>
              {/if}
            </span>
          </span>
          {#each visibleCols as col (col.id)}
            {#if col.id === "title"}
              <span class="entry-row-col col-title">
                <div class="entry-row-main">
                  <span class="entry-row-title" title={row.entry.expired ? "已过期" : undefined}
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
              {@const text = colText(row.entry, col.id)}
              <span
                class="entry-row-col"
                class:col-masked={col.id === "password"}
                title={text || undefined}
              >
                <span class="entry-row-col-text">{text}</span>
              </span>
            {/if}
          {/each}
          <div class="entry-row-actions">
            <button
              class="row-btn"
              class:star-active={row.entry.favorite}
              title={row.entry.favorite ? "取消收藏" : "收藏条目"}
              onclick={(event) => {
                event.stopPropagation();
                onfavorite(row.entry);
              }}
            >
              <AppIcon name="star" size={12} />
            </button>
            <button
              class="row-btn"
              title="复制用户名"
              onclick={(event) => {
                event.stopPropagation();
                oncopyusername(row.entry);
              }}
            >
              <AppIcon name="user" size={12} />
            </button>
            <button
              class="row-btn"
              title="复制密码"
              onclick={(event) => {
                event.stopPropagation();
                oncopypassword(row.entry);
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

<style>
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

  .head-cell.col-dragging {
    opacity: 0.45;
    cursor: grabbing;
  }

  .head-cell.drop-before::after,
  .head-cell.drop-after::after {
    content: "";
    position: absolute;
    top: 3px;
    bottom: 3px;
    width: 2px;
    border-radius: 1px;
    background: var(--selection-color);
  }

  .head-cell.drop-before::after {
    left: -2px;
  }

  .head-cell.drop-after::after {
    right: -2px;
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

  .entry-table.compact .entry-row {
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

  .mobile-entry-summary {
    display: none;
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

  .row-btn.star-active,
  .row-btn.star-active:hover {
    color: var(--warning-color);
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

  @media (max-width: 720px) {
    .entry-table {
      overflow-x: hidden;
    }

    .entry-table-head {
      display: none;
    }

    .entry-list {
      width: 100%;
      overflow-x: hidden;
    }

    .entry-row,
    .entry-table.compact .entry-row {
      grid-template-columns: 44px minmax(0, 1fr) 100px;
      width: 100%;
      min-width: 0;
      height: 48px;
      border-bottom: 1px solid var(--border-subtle);
    }

    .entry-row-col {
      display: none;
    }

    .entry-row-icon-cell,
    .mobile-entry-summary,
    .entry-row-actions {
      border: 0;
    }

    .mobile-entry-summary {
      display: flex;
      min-width: 0;
      height: 100%;
      padding: 0 6px;
    }

    .entry-row-actions {
      gap: 4px;
      padding: 0 8px 0 2px;
      opacity: 1;
    }

    .row-btn {
      width: 28px;
      height: 32px;
    }
  }

  :global(body.dragging-column) {
    cursor: grabbing !important;
    user-select: none;
  }
</style>
