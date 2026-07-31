<script lang="ts">
  import type { VaultGroup } from "$lib/types/vault";
  import { countEntries } from "$lib/services/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ContextMenu, { type ContextMenuItem } from "$lib/components/ContextMenu.svelte";

  interface Props {
    group: VaultGroup;
    depth: number;
    selected: string | null;
    expanded: Set<string>;
    showIcon?: boolean;
    showChevron?: boolean;
    onselect: (uuid: string) => void;
    ontoggle: (uuid: string) => void;
    onaddsubgroup: (parentUuid: string) => void;
    onrename: (uuid: string, name: string) => void;
    ondelete: (uuid: string) => void;
  }

  let {
    group,
    depth,
    selected,
    expanded,
    showIcon = true,
    showChevron = true,
    onselect,
    ontoggle,
    onaddsubgroup,
    onrename,
    ondelete,
  }: Props = $props();

  let renaming = $state(false);
  let nameInput = $state(group.name);
  let inputEl: HTMLInputElement | undefined = $state();
  let menu = $state<{ x: number; y: number } | null>(null);

  const count = $derived(countEntries(group));
  const isExpanded = $derived(expanded.has(group.uuid));
  const hasChildren = $derived(group.children.length > 0);

  const menuItems: ContextMenuItem[] = [
    { id: "add-subgroup", label: "新建子分组", icon: "folder-plus" },
    { id: "rename", label: "重命名", icon: "edit" },
    { id: "delete", label: "删除分组", icon: "trash", destructive: true },
  ];

  $effect(() => {
    if (renaming) inputEl?.focus();
  });

  function handleRowClick(): void {
    onselect(group.uuid);
    if (hasChildren) ontoggle(group.uuid);
  }

  function openMenu(event: MouseEvent): void {
    event.preventDefault();
    onselect(group.uuid);
    menu = { x: event.clientX, y: event.clientY };
  }

  function closeMenu(): void {
    menu = null;
  }

  function handleMenuAction(id: string): void {
    if (id === "add-subgroup") onaddsubgroup(group.uuid);
    else if (id === "rename") renaming = true;
    else if (id === "delete") ondelete(group.uuid);
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
  style:padding-left={`calc(10px + (var(--group-indent, 14px) * ${depth}))`}
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
      {#if showChevron}
        <span
          class="chevron-btn"
          class:leaf={!hasChildren}
          class:open={isExpanded}
          role="button"
          tabindex={hasChildren ? 0 : -1}
          aria-label={isExpanded ? "折叠分组" : "展开分组"}
          title={hasChildren ? (isExpanded ? "折叠" : "展开") : ""}
          onclick={() => ontoggle(group.uuid)}
          onkeydown={(e) => {
            if (e.key === "Enter" || e.key === " ") {
              e.preventDefault();
              ontoggle(group.uuid);
            }
          }}
        >
          <AppIcon name="chevron-down" size={13} />
        </span>
      {/if}
      <button
        class="group-select"
        class:no-icon={!showIcon}
        onclick={handleRowClick}
        oncontextmenu={openMenu}
        title={group.name}
      >
        {#if showIcon}
          <AppIcon name="folder" size={13} />
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
    <svelte:self
      group={child}
      depth={depth + 1}
      {selected}
      {expanded}
      {showIcon}
      {showChevron}
      {onselect}
      {ontoggle}
      {onaddsubgroup}
      {onrename}
      {ondelete}
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

  .chevron-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    width: 16px;
    height: 18px;
    border-radius: var(--group-radius, var(--settings-control-radius, 6px));
    color: var(--text-faint);
    cursor: pointer;
  }

  .chevron-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .chevron-btn.leaf {
    visibility: hidden;
    pointer-events: none;
  }

  .chevron-btn svg {
    transition: transform 0.15s ease;
  }

  .chevron-btn.open svg {
    transform: rotate(90deg);
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
    line-height: 1.3;
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
