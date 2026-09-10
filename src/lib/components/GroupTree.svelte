<script lang="ts">
  import type { VaultGroup } from "$lib/types/vault";
  import { untrack } from "svelte";
  import { buildEntryCounts, findBinGroup, totalExcludingBin } from "$lib/utils/tree";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";
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
    onmeta?: (uuid: string) => void;
    ondelete: (uuid: string) => void;
    onrestore?: (uuid: string) => void;
    onemptybin?: () => void;
    ondropentry?: (groupUuid: string, uuids: string[]) => void;
    /** Persist a group's expand state (calls `set_group_expanded`). */
    ontoggle?: (uuid: string, expanded: boolean) => void;
    /** Persist a bulk expand/collapse in one `set_groups_expanded` call. */
    onsetexpanded: (uuids: string[], expanded: boolean) => void;
    /** Close affordance for the mobile drawer; hidden on desktop. */
    onclose?: () => void;
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
    onmeta,
    ondelete,
    onrestore,
    onemptybin,
    ondropentry,
    ontoggle,
    onsetexpanded,
    onclose,
  }: Props = $props();

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

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  let knownUuids = new Set(initialExpanded);

  function toggleGroup(uuid: string): void {
    const next = new Set(expanded);
    const nowExpanded = !next.has(uuid);
    if (nowExpanded) next.add(uuid);
    else next.delete(uuid);
    expanded = next;
    ontoggle?.(uuid, nowExpanded);
  }

  /** One depth-first walk per `root`, supplying the full uuid set and the
   *  child→parent map that the new-group diff, the reveal effect, and
   *  `allExpanded` share. Recomputing any of them by re-walking the tree on
   *  every snapshot is the O(N)/O(N²) churn this memoization removes. */
  const treeWalk = $derived.by(() => {
    const uuids = new Set<string>();
    const parentByGroupUuid = new Map<string, string>();
    const visit = (group: VaultGroup, parentUuid: string | null) => {
      uuids.add(group.uuid);
      if (parentUuid) parentByGroupUuid.set(group.uuid, parentUuid);
      for (const child of group.children) visit(child, group.uuid);
    };
    visit(root, null);
    return { uuids, parentByGroupUuid };
  });

  $effect(() => {
    const { uuids, parentByGroupUuid } = treeWalk;
    untrack(() => {
      // A brand-new database (no overlapping uuids) also starts collapsed.
      let overlaps = false;
      for (const uuid of uuids) {
        if (knownUuids.has(uuid)) {
          overlaps = true;
          break;
        }
      }
      if (uuids.size > 0 && !overlaps) {
        expanded = new Set([root.uuid]);
        return;
      }
      let next: Set<string> | null = null;
      for (const uuid of uuids) {
        if (!knownUuids.has(uuid)) {
          next ??= new Set(expanded);
          next.add(uuid);
          const parent = parentByGroupUuid.get(uuid);
          if (parent) next.add(parent);
        }
      }
      if (next) expanded = next;
    });
    knownUuids = uuids;
  });

  /** Subtree entry counts per group, computed once per `root` change (a
   *  bottom-up walk) instead of re-walking the tree for every rendered node. */
  const counts = $derived(buildEntryCounts(root));
  const total = $derived.by(() => {
    const bin = findBinGroup(root);
    return totalExcludingBin(counts, root.uuid, bin?.uuid ?? null);
  });

  /** Ancestor chain (excluding the target itself, root-adjacent last) of a uuid. */
  function ancestorsOf(uuid: string): string[] {
    const chain: string[] = [];
    let cursor = treeWalk.parentByGroupUuid.get(uuid);
    while (cursor) {
      chain.push(cursor);
      cursor = treeWalk.parentByGroupUuid.get(cursor);
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
    const next = new Set(treeWalk.uuids);
    expanded = next;
    const uuids = [...next].filter((uuid) => uuid !== root.uuid);
    if (uuids.length > 0) onsetexpanded(uuids, true);
  }

  /** Collapse every group (keep only the root). */
  function collapseAll(): void {
    expanded = new Set([root.uuid]);
    const uuids = [...treeWalk.uuids].filter((uuid) => uuid !== root.uuid);
    if (uuids.length > 0) onsetexpanded(uuids, false);
  }

  /** Every renderable group (the root itself is never shown) is expanded. */
  const allExpanded = $derived.by(() => {
    if (treeWalk.uuids.size <= 1) return false;
    for (const uuid of treeWalk.uuids) {
      if (uuid !== root.uuid && !expanded.has(uuid)) return false;
    }
    return true;
  });

  /** Single expand/collapse-all toggle: collapse when everything is open. */
  function toggleExpandAll(): void {
    if (allExpanded) collapseAll();
    else expandAll();
  }
</script>

<div class="group-tree">
  <div class="tree-head">
    <span class="tree-label">{t(lang, "toolbar.groups")}</span>
    <div class="tree-tools">
      {#if onclose}
        <button
          class="tool-btn drawer-close"
          title={t(lang, "group.closePanel")}
          aria-label={t(lang, "group.closePanel")}
          onclick={onclose}
        >
          <AppIcon name="x" size={13} />
        </button>
      {/if}
      <button
        class="tool-btn"
        title={t(lang, "menu.newGroup")}
        aria-label={t(lang, "group.newUnder")}
        onclick={() => onaddsubgroup(selected)}
      >
        <AppIcon name="folder-plus" size={13} />
      </button>
      <button
        class="tool-btn"
        title={allExpanded ? t(lang, "group.collapseAll") : t(lang, "group.expandAll")}
        aria-label={allExpanded ? t(lang, "group.collapseAll") : t(lang, "group.expandAll")}
        onclick={toggleExpandAll}
      >
        <AppIcon name={allExpanded ? "chevrons-right" : "chevrons-down"} size={13} />
      </button>
    </div>
  </div>

  <div class="tree-list">
    <button class="all-row" class:selected={selected === null} onclick={() => onselect(null)}>
      <AppIcon name="grid" size={13} />
      <span class="all-name">{t(lang, "group.allEntries")}</span>
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
        {onmeta}
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

  /* Drawer close affordance exists only in the mobile overlay. */
  .drawer-close {
    display: none;
  }

  @media (max-width: 720px) {
    .drawer-close {
      display: inline-flex;
    }
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
