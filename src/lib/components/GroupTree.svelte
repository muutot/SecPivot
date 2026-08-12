<script lang="ts">
  import type { VaultGroup } from "$lib/types/vault";
  import { untrack } from "svelte";
  import { buildEntryCounts } from "$lib/utils/tree";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import GroupNode from "$lib/components/GroupNode.svelte";

  interface Props {
    root: VaultGroup;
    selected: string | null;
    /** When set, reveal (expand ancestors + scroll into view) this group. */
    reveal?: string | null;
    showIcon?: boolean;
    showChevron?: boolean;
    /** Database custom icons (favicon `data:` URLs) keyed by icon UUID. */
    customIcons?: Record<string, string>;
    onselect: (uuid: string | null) => void;
    onaddsubgroup: (parentUuid: string | null) => void;
    onrename: (uuid: string, name: string) => void;
    onchangeicon: (uuid: string) => void;
    onautotype?: (uuid: string) => void;
    ondelete: (uuid: string) => void;
    onrestore?: (uuid: string) => void;
    onemptybin?: () => void;
    ondropentry?: (groupUuid: string, uuids: string[]) => void;
    /** Persist a group's expand state (calls `set_group_expanded`). */
    ontoggle?: (uuid: string, expanded: boolean) => void;
    /** Persist a bulk expand/collapse in one `set_groups_expanded` call. */
    onsetexpanded: (uuids: string[], expanded: boolean) => void;
  }

  let {
    root,
    selected,
    reveal = null,
    showIcon = true,
    showChevron = true,
    customIcons = {},
    onselect,
    onaddsubgroup,
    onrename,
    onchangeicon,
    onautotype,
    ondelete,
    onrestore,
    onemptybin,
    ondropentry,
    ontoggle,
    onsetexpanded,
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

  function collectExpanded(group: VaultGroup, into: Set<string>): void {
    if (group.isExpanded) into.add(group.uuid);
    for (const child of group.children) collectExpanded(child, into);
  }

  const initialRoot = (() => root)();
  // Restore the groups the user had open last session from the persisted
  // `Group.is_expanded` flag; the root itself is never rendered but is kept
  // so a fresh database still has a stable anchor.
  const initialExpanded = (() => {
    const set = new Set<string>();
    collectExpanded(initialRoot, set);
    set.add(initialRoot.uuid);
    return set;
  })();

  let expanded = $state<Set<string>>(initialExpanded);

  let knownUuids = new Set(initialExpanded);

  function toggleGroup(uuid: string): void {
    const next = new Set(expanded);
    const nowExpanded = !next.has(uuid);
    if (nowExpanded) next.add(uuid);
    else next.delete(uuid);
    expanded = next;
    ontoggle?.(uuid, nowExpanded);
  }

  $effect(() => {
    const uuids = new Set<string>();
    collectUuids(root, uuids);
    // A brand-new database (no overlapping uuids) also starts collapsed.
    if (uuids.size > 0 && ![...uuids].some((uuid) => knownUuids.has(uuid))) {
      expanded = new Set([root.uuid]);
      knownUuids = new Set(uuids);
      return;
    }
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

  /** Subtree entry counts per group, computed once per `root` change (a
   *  bottom-up walk) instead of re-walking the tree for every rendered node. */
  const counts = $derived(buildEntryCounts(root));
  const total = $derived(
    root.children.reduce(
      (sum, child) => sum + (child.isRecycleBin ? 0 : (counts.get(child.uuid) ?? 0)),
      0,
    ),
  );

  /** Ancestor chain (excluding the target itself, root-adjacent last) of a uuid. */
  function ancestorsOf(uuid: string): string[] {
    const chain: string[] = [];
    let cursor = findParent(root, uuid);
    while (cursor) {
      chain.push(cursor.uuid);
      cursor = findParent(root, cursor.uuid);
    }
    return chain;
  }

  /** Reveal the requested group by expanding every ancestor so its row is
   *  rendered, then let the matched `GroupNode` scroll itself into view.
   *  `expanded` is read via `untrack` so writing it does not self-trigger. */
  $effect(() => {
    const target = reveal;
    if (!target) return;
    untrack(() => {
      const next = new Set(expanded);
      for (const anc of ancestorsOf(target)) next.add(anc);
      expanded = next;
    });
  });

  /** Expand every group (keep the root itself; it is not rendered). */
  function expandAll(): void {
    const next = new Set<string>();
    collectUuids(root, next);
    expanded = next;
    const uuids = [...next].filter((uuid) => uuid !== root.uuid);
    if (uuids.length > 0) onsetexpanded(uuids, true);
  }

  /** Collapse every group (keep only the root). */
  function collapseAll(): void {
    const all = new Set<string>();
    collectUuids(root, all);
    expanded = new Set([root.uuid]);
    const uuids = [...all].filter((uuid) => uuid !== root.uuid);
    if (uuids.length > 0) onsetexpanded(uuids, false);
  }
</script>

<div class="group-tree">
  <div class="tree-head">
    <span class="tree-label">分组</span>
    <div class="tree-tools">
      <button
        class="tool-btn"
        title="新建分组"
        aria-label="在当前分组下新建分组"
        onclick={() => onaddsubgroup(selected)}
      >
        <AppIcon name="folder-plus" size={13} />
      </button>
      <button class="tool-btn" title="全部展开" aria-label="全部展开" onclick={expandAll}>
        <AppIcon name="chevrons-down" size={13} />
      </button>
      <button class="tool-btn" title="全部折叠" aria-label="全部折叠" onclick={collapseAll}>
        <AppIcon name="chevrons-right" size={13} />
      </button>
    </div>
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
        {reveal}
        {expanded}
        {showIcon}
        {showChevron}
        {customIcons}
        {counts}
        onselect={(uuid: string) => onselect(uuid)}
        ontoggle={toggleGroup}
        onaddsubgroup={(parentUuid: string) => onaddsubgroup(parentUuid)}
        {onrename}
        {onchangeicon}
        {onautotype}
        {ondelete}
        {onrestore}
        {onemptybin}
        {ondropentry}
      />
    {/each}
  </div>
</div>

<style>
  .group-tree {
    display: flex;
    flex-direction: column;
    flex: 1;
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

  .tree-tools {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .tool-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    border: none;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
    cursor: pointer;
  }

  .tool-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .tree-list {
    flex: 1;
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
