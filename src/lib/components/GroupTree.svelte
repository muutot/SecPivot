<script lang="ts">
  import type { VaultGroup } from "$lib/types/vault";
  import { countEntries } from "$lib/services/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import GroupNode from "$lib/components/GroupNode.svelte";

  interface Props {
    root: VaultGroup;
    selected: string | null;
    showIcon?: boolean;
    showChevron?: boolean;
    onselect: (uuid: string | null) => void;
    onaddsubgroup: (parentUuid: string | null) => void;
    onrename: (uuid: string, name: string) => void;
    ondelete: (uuid: string) => void;
  }

  let {
    root,
    selected,
    showIcon = true,
    showChevron = true,
    onselect,
    onaddsubgroup,
    onrename,
    ondelete,
  }: Props = $props();

  function collectUuids(group: VaultGroup, into: Set<string>): void {
    into.add(group.uuid);
    for (const child of group.children) collectUuids(child, into);
  }

  function findParent(group: VaultGroup, uuid: string): VaultGroup | null {
    for (const child of group.children) {
      if (child.uuid === uuid) return group;
      const found = findParent(child, uuid);
      if (found) return found;
    }
    return null;
  }

  const initialExpanded = new Set<string>();
  collectUuids(root, initialExpanded);

  let expanded = $state<Set<string>>(initialExpanded);

  let knownUuids = new Set(initialExpanded);

  function toggleGroup(uuid: string): void {
    const next = new Set(expanded);
    if (next.has(uuid)) next.delete(uuid);
    else next.add(uuid);
    expanded = next;
  }

  $effect(() => {
    const uuids = new Set<string>();
    collectUuids(root, uuids);
    let next: Set<string> | null = null;
    for (const uuid of uuids) {
      if (!knownUuids.has(uuid)) {
        next ??= new Set(expanded);
        next.add(uuid);
        const parent = findParent(root, uuid);
        if (parent) next.add(parent.uuid);
      }
    }
    if (next) expanded = next;
    knownUuids = new Set(uuids);
  });

  const total = $derived(countEntries(root));
</script>

<div class="group-tree">
  <div class="tree-head">
    <span class="tree-label">分组</span>
  </div>

  <div class="tree-list">
    <button class="all-row" class:selected={selected === null} onclick={() => onselect(null)}>
      <AppIcon name="grid" size={13} />
      <span class="all-name">全部条目</span>
      <span class="all-count">{total}</span>
    </button>
    {#each root.children as child (child.uuid)}
      <GroupNode
        group={child}
        depth={0}
        {selected}
        {expanded}
        {showIcon}
        {showChevron}
        onselect={(uuid: string) => onselect(uuid)}
        ontoggle={toggleGroup}
        onaddsubgroup={(parentUuid: string) => onaddsubgroup(parentUuid)}
        {onrename}
        {ondelete}
      />
    {/each}
  </div>
</div>

<style>
  .group-tree {
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .tree-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px 6px;
  }

  .tree-label {
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .tree-list {
    min-height: 0;
    overflow: auto;
    padding: 0 4px 12px;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .all-row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 28px;
    padding: 0 10px;
    margin-bottom: 4px;
    border: 1px solid transparent;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: transparent;
    font-size: var(--font-size-secondary, 11px);
    text-align: left;
    cursor: pointer;
  }

  .all-row:hover {
    background: var(--hover-bg);
  }

  .all-row.selected {
    border-color: color-mix(in srgb, var(--selection-color) 40%, transparent);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--hover-bg));
  }

  .all-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .all-count {
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }
</style>
