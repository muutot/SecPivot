<script lang="ts">
  import type { VaultGroup } from "$lib/types/vault";
  import { countEntries } from "$lib/utils/tree";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import { keepassGroupIconName } from "$lib/utils/keepass-icons";
  import ContextMenu, { type ContextMenuItem } from "$lib/components/ContextMenu.svelte";
  import GroupNode from "$lib/components/GroupNode.svelte";
  import {
    activeContextMenu,
    closeContextMenu,
    openContextMenu,
  } from "$lib/stores/activeContextMenu.svelte";

  interface Props {
    group: VaultGroup;
    depth: number;
    selected: string | null;
    /** When set and matching this node, scroll the row into view. */
    reveal?: string | null;
    expanded: Set<string>;
    showIcon?: boolean;
    showChevron?: boolean;
    /** Database custom icons (favicon `data:` URLs) keyed by icon UUID. */
    customIcons?: Record<string, string>;
    /** Precomputed subtree entry counts keyed by group uuid (avoid the O(N²)
     *  per-node `countEntries` walk); falls back to a local walk when absent. */
    counts?: Map<string, number> | null;
    inRecycleBin?: boolean;
    onselect: (uuid: string) => void;
    ontoggle: (uuid: string) => void;
    onaddsubgroup: (parentUuid: string) => void;
    onrename: (uuid: string, name: string) => void;
    onchangeicon: (uuid: string) => void;
    onautotype?: (uuid: string) => void;
    onmeta?: (uuid: string) => void;
    ondelete: (uuid: string) => void;
    onrestore?: (uuid: string) => void;
    onemptybin?: () => void;
    ondropentry?: (groupUuid: string, uuids: string[]) => void;
  }

  let {
    group,
    depth,
    selected,
    reveal = null,
    expanded,
    showIcon = true,
    showChevron = true,
    customIcons = {},
    counts = null,
    inRecycleBin = false,
    onselect,
    ontoggle,
    onaddsubgroup,
    onrename,
    onchangeicon,
    onautotype,
    onmeta,
    ondelete,
    onrestore,
    onemptybin,
    ondropentry,
  }: Props = $props();

  let renaming = $state(false);
  let nameInput = $state<string>("");
  let inputEl: HTMLInputElement | undefined = $state();
  let nodeEl: HTMLDivElement | undefined = $state();
  let menu = $state<{ x: number; y: number } | null>(null);
  let dragActive = $state(false);

  $effect(() => {
    if (reveal !== group.uuid) return;
    // Reveal this row by scrolling only the tree's own scroll container
    // (`.tree-list`); `scrollIntoView` would also scroll outer/viewport
    // ancestors and can lock the panel. The target may mount or shift as its
    // ancestors expand a few paints after this effect runs, so retry on
    // successive frames until the row is actually visible, or abort.
    const vpad = 8;
    let frames = 0;
    let rafId = 0;
    function attempt(): void {
      const el = nodeEl;
      const scroller = el?.closest(".tree-list");
      if (!el || !scroller) {
        // The row may not be mounted inside the tree yet; keep retrying within
        // the same paint budget instead of silently aborting the reveal.
        if (frames < 10) {
          frames += 1;
          rafId = requestAnimationFrame(attempt);
        }
        return;
      }
      const elRect = el.getBoundingClientRect();
      const scrRect = scroller.getBoundingClientRect();
      if (
        scrRect.height > vpad &&
        elRect.top >= scrRect.top + vpad &&
        elRect.bottom <= scrRect.bottom - vpad
      ) {
        return; // already fully within the visible area
      }
      let target = scroller.scrollTop;
      if (elRect.top < scrRect.top) {
        target += elRect.top - scrRect.top - vpad;
      } else if (elRect.bottom > scrRect.bottom) {
        target += elRect.bottom - scrRect.bottom + vpad;
      }
      scroller.scrollTop = Math.max(0, target);
      if (frames < 10) {
        frames += 1;
        rafId = requestAnimationFrame(attempt);
      }
    }
    rafId = requestAnimationFrame(attempt);
    return () => cancelAnimationFrame(rafId);
  });

  const groupName = $derived(group.name);

  $effect(() => {
    nameInput = groupName;
  });

  const count = $derived(counts ? (counts.get(group.uuid) ?? 0) : countEntries(group));
  const isExpanded = $derived(expanded.has(group.uuid));
  const hasChildren = $derived(group.children.length > 0);
  const isBin = $derived(group.isRecycleBin);
  const iconName = $derived(keepassGroupIconName(group.icon));

  const DRAG_MIME = "application/x-secpivot-entries";

  const menuItems: ContextMenuItem[] = $derived(
    isBin
      ? count > 0
        ? [{ id: "empty-bin", label: "清空回收站", icon: "trash", destructive: true }]
        : []
      : inRecycleBin
        ? [
            { id: "restore", label: "恢复分组", icon: "undo" },
            { id: "delete", label: "永久删除", icon: "trash", destructive: true },
          ]
        : [
            { id: "add-subgroup", label: "新建子分组", icon: "folder-plus" },
            { id: "rename", label: "重命名", icon: "edit" },
            { id: "change-icon", label: "设置图标", icon: "palette" },
            { id: "autotype", label: "自动填充设置", icon: "keyboard" },
            { id: "meta", label: "属性", icon: "sliders" },
            { id: "delete", label: "删除分组", icon: "trash", destructive: true },
          ],
  );

  $effect(() => {
    if (renaming) inputEl?.focus();
  });

  function handleRowClick(): void {
    onselect(group.uuid);
  }

  function handleMiddleClick(event: MouseEvent): void {
    if (event.button !== 1) return;
    event.preventDefault();
    if (hasChildren) ontoggle(group.uuid);
  }

  function openMenu(event: MouseEvent): void {
    event.preventDefault();
    onselect(group.uuid);
    menu = { x: event.clientX, y: event.clientY };
    openContextMenu("group");
  }

  function closeMenu(): void {
    menu = null;
    closeContextMenu("group");
  }

  // Only one context menu may be visible at a time. When another owner (a
  // page-level menu) becomes active, close this node's menu.
  $effect(() => {
    if ($activeContextMenu !== "group" && menu) menu = null;
  });

  function handleMenuAction(id: string): void {
    if (id === "add-subgroup") onaddsubgroup(group.uuid);
    else if (id === "rename") renaming = true;
    else if (id === "change-icon") onchangeicon(group.uuid);
    else if (id === "autotype") onautotype?.(group.uuid);
    else if (id === "meta") onmeta?.(group.uuid);
    else if (id === "delete") ondelete(group.uuid);
    else if (id === "restore") onrestore?.(group.uuid);
    else if (id === "empty-bin") onemptybin?.();
  }

  function commitRename(): void {
    const value = nameInput.trim();
    renaming = false;
    if (value && value !== group.name) onrename(group.uuid, value);
  }
</script>

<div
  class="group-node"
  class:selected={selected === group.uuid}
  style:padding-left={`calc(var(--group-indent, 14px) * ${depth})`}
  bind:this={nodeEl}
>
  {#if renaming}
    <div class="rename-row">
      <input
        class="rename-input"
        type="text"
        bind:value={nameInput}
        onkeydown={(e) => {
          if (e.key === "Enter") commitRename();
          if (e.key === "Escape") renaming = false;
        }}
        onblur={commitRename}
        bind:this={inputEl}
      />
      <button class="mini-btn" onclick={() => (renaming = false)} aria-label="取消">
        <AppIcon name="x" size={13} />
      </button>
    </div>
  {:else}
    <div class="group-row">
      <button
        class="group-select"
        class:no-icon={!showIcon}
        class:drop-target={dragActive}
        onclick={handleRowClick}
        onauxclick={handleMiddleClick}
        oncontextmenu={openMenu}
        title={group.name}
        ondragover={(e) => {
          const transfer = e.dataTransfer;
          if (!isBin && transfer?.types.includes(DRAG_MIME)) {
            e.preventDefault();
            transfer.dropEffect = "move";
            dragActive = true;
          }
        }}
        ondragleave={() => (dragActive = false)}
        ondrop={(e) => {
          dragActive = false;
          if (isBin || !e.dataTransfer) return;
          e.preventDefault();
          const raw = e.dataTransfer.getData(DRAG_MIME);
          if (!raw) return;
          ondropentry?.(group.uuid, JSON.parse(raw) as string[]);
        }}
      >
        {#if showChevron}
          {#if hasChildren}
            <span class="chevron" class:open={isExpanded} aria-hidden="true">
              <AppIcon name="chevron-right" size={13} />
            </span>
          {:else}
            <span class="leaf-dot" aria-hidden="true"></span>
          {/if}
        {/if}
        {#if showIcon}
          {#if group.customIcon && customIcons[group.customIcon]}
            <img
              class="group-icon-img"
              src={customIcons[group.customIcon]}
              alt=""
              draggable="false"
            />
          {:else}
            <AppIcon name={isBin ? "trash" : iconName} size={16} />
          {/if}
        {/if}
        <span class="group-name">{group.name}</span>
        {#if count > 0}
          <span class="group-count">{count}</span>
        {/if}
      </button>
    </div>
  {/if}
</div>

{#if isExpanded}
  {#each group.children as child (child.uuid)}
    <GroupNode
      group={child}
      depth={depth + 1}
      {selected}
      {reveal}
      {expanded}
      {showIcon}
      {showChevron}
      {customIcons}
      {counts}
      inRecycleBin={isBin || inRecycleBin}
      {onselect}
      {ontoggle}
      {onaddsubgroup}
      {onrename}
      {onchangeicon}
      {onautotype}
      {onmeta}
      {ondelete}
      {onrestore}
      {onemptybin}
      {ondropentry}
    />
  {/each}
{/if}

{#if menu}
  <ContextMenu
    x={menu.x}
    y={menu.y}
    items={menuItems}
    onclose={closeMenu}
    onaction={handleMenuAction}
  />
{/if}

<style>
  .group-node {
    display: flex;
    flex-direction: column;
    margin-bottom: var(--group-gap, 0px);
  }

  .group-row {
    display: flex;
    align-items: center;
    gap: 0;
    min-width: 0;
    padding-right: 4px;
  }

  .chevron {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    margin-right: -4px;
    color: var(--text-faint);
    transition: transform 0.15s ease;
  }

  .group-select:hover .chevron,
  .group-select:focus-visible .chevron {
    color: var(--text-primary);
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .leaf-dot {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 16px;
    height: 16px;
    margin-right: -4px;
  }

  .leaf-dot::before {
    content: "";
    width: 4px;
    height: 4px;
    border-radius: 999px;
    background: var(--text-faint);
  }

  .group-select {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
    padding: var(--group-pad-y, 6px) 6px;
    border: 1px solid transparent;
    border-radius: var(--group-radius, var(--settings-control-radius, 6px));
    color: var(--text-secondary);
    background: transparent;
    font-size: var(--font-size-secondary, 11px);
    line-height: 1;
    text-align: left;
    cursor: pointer;
  }

  .group-node.selected .group-select {
    border-color: color-mix(in srgb, var(--selection-color) 40%, transparent);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--hover-bg));
  }

  .group-select:hover {
    background: var(--hover-bg);
  }

  .group-select.drop-target {
    border-color: color-mix(in srgb, var(--selection-color) 60%, transparent);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--hover-bg));
  }

  .group-select.no-icon {
    padding-left: 0;
  }

  .group-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group-icon-img {
    width: 16px;
    height: 16px;
    display: block;
    border-radius: 2px;
    object-fit: contain;
  }

  .group-count {
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }

  .rename-row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding-right: 4px;
  }

  .rename-input {
    width: 100%;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--selection-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
  }
</style>
