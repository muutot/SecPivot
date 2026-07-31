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
    onselect: (uuid: string) => void;
    onaddsubgroup: (parentUuid: string) => void;
    onrename: (uuid: string, name: string) => void;
    ondelete: (uuid: string) => void;
  }

  let { group, depth, selected, expanded, onselect, onaddsubgroup, onrename, ondelete }: Props =
    $props();

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

  function toggle(): void {
    if (!hasChildren) return;
    const next = new Set(expanded);
    if (next.has(group.uuid)) next.delete(group.uuid);
    else next.add(group.uuid);
    expanded.clear();
    for (const item of next) expanded.add(item);
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
  style:padding-left={`calc(8px + (var(--group-indent, 14px) * ${depth}))`}
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
      <span
        class="chevron-btn"
        class:leaf={!hasChildren}
        class:open={isExpanded}
        role="button"
        tabindex={hasChildren ? 0 : -1}
        onclick={toggle}
        onkeydown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            toggle();
          }
        }}
        aria-label={isExpanded ? "折叠" : "展开"}
      >
        <AppIcon name="chevron-down" size={13} />
      </span>
      <button
        class="group-select"
        onclick={() => onselect(group.uuid)}
        oncontextmenu={openMenu}
        title={group.name}
      >
        <AppIcon name="folder" size={13} />
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
      {onselect}
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
    gap: 2px;
    min-width: 0;
    padding-right: 4px;
  }

  .group-select {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    flex: 1;
    padding: var(--group-pad-y, 6px) 6px;
    border: 1px solid transparent;
    border-radius: var(--settings-control-radius, 6px);
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

  .group-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group-count {
    margin-left: auto;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }

  .chevron-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 20px;
    flex: 0 0 auto;
    color: var(--text-faint);
    cursor: pointer;
    border-radius: 4px;
  }

  .chevron-btn.leaf {
    visibility: hidden;
  }

  .chevron-btn .app-icon {
    transition: transform 100ms ease;
  }

  .chevron-btn.open .app-icon {
    transform: rotate(90deg);
  }

  .chevron-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
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
