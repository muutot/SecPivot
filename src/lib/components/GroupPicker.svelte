<script lang="ts">
  import type { VaultGroup } from "$lib/types/vault";
  import { countEntries } from "$lib/services/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import type { IconName } from "$lib/components/AppIcon.svelte";
  import { KEEPASS_ICONS, GROUP_DEFAULT_ICON } from "$lib/utils/keepass-icons";

  interface Props {
    groups: VaultGroup[];
    value: string;
    onchange: (uuid: string) => void;
  }

  let { groups, value, onchange }: Props = $props();

  let open = $state(false);
  let browseUuid = $state<string | null>(null);
  let pickerEl = $state<HTMLDivElement>();

  function findGroup(parent: VaultGroup, uuid: string): VaultGroup | null {
    if (parent.uuid === uuid) return parent;
    for (const child of parent.children) {
      const found = findGroup(child, uuid);
      if (found) return found;
    }
    return null;
  }

  /** Path from root to the group currently being browsed (top-down). */
  const chain = $derived.by(() => {
    const path: VaultGroup[] = [];
    const root = groups[0];
    if (!root) return path;
    const targetUuid = browseUuid ?? value;
    let root1 = root;
    const walked = new Set<string>();
    function walk(group: VaultGroup, target: string): boolean {
      if (walked.has(group.uuid)) return false;
      walked.add(group.uuid);
      if (group.uuid === target) {
        path.unshift(group);
        return true;
      }
      for (const child of group.children) {
        if (walk(child, target)) {
          path.unshift(group);
          return true;
        }
      }
      return false;
    }
    walk(root1, targetUuid);
    if (path.length === 0 && root1) path.push(root1);
    return path;
  });

  const current = $derived(chain[chain.length - 1] ?? null);

  function drillTo(uuid: string): void {
    browseUuid = uuid;
  }

  function dismiss(): void {
    open = false;
    browseUuid = null;
  }

  function confirm(): void {
    if (!current) return;
    onchange(current.uuid);
    dismiss();
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      dismiss();
    } else if (e.key === "Enter" && open) {
      e.preventDefault();
      confirm();
    }
  }

  function handlePanelClick(e: MouseEvent): void {
    if (e.target === e.currentTarget) confirm();
  }

  function handleClickOutside(e: MouseEvent): void {
    if (open && pickerEl && !pickerEl.contains(e.target as Node)) {
      dismiss();
    }
  }

  function toggle(): void {
    if (open) {
      open = false;
      browseUuid = null;
    } else {
      browseUuid = null;
      open = true;
    }
  }

  function iconOf(group: VaultGroup): IconName {
    return ((group.icon !== undefined
      ? (KEEPASS_ICONS[group.icon] as string | undefined)
      : undefined) ?? GROUP_DEFAULT_ICON) as IconName;
  }
</script>

<svelte:window onkeydown={handleKeydown} onclick={handleClickOutside} />

<div class="group-picker" bind:this={pickerEl}>
  <button
    type="button"
    class="picker-trigger"
    aria-haspopup="listbox"
    aria-expanded={open}
    onclick={toggle}
  >
    <span class="trigger-label" title={chain.map((g) => g.name).join(" / ")}>
      {chain.map((g) => g.name).join(" / ") || "—"}
    </span>
    <AppIcon name="chevron-down" size={13} />
  </button>

  {#if open && current}
    <div
      class="picker-panel"
      role="listbox"
      aria-label="选择分组"
      tabindex="-1"
      onclick={handlePanelClick}
      onkeydown={(e) => e.key === "Enter" && confirm()}
    >
      {#if chain.length > 1}
        <button
          type="button"
          class="picker-up"
          onclick={() => drillTo(chain[chain.length - 2].uuid)}
        >
          <AppIcon name="chevron-left" size={13} />
          <span class="up-label">返回上一级：{chain[chain.length - 2].name}</span>
        </button>
      {/if}

      <div class="crumb-current">
        {#each chain as crumb, i (crumb.uuid)}
          {#if i > 0}
            <span class="crumb-sep">/</span>
          {/if}
          {#if i < chain.length - 1}
            <button type="button" class="crumb-link" onclick={() => drillTo(crumb.uuid)}>
              {crumb.name}
            </button>
          {:else}
            <span class="crumb-label">
              <AppIcon name={iconOf(current)} size={13} />
              <span class="crumb-name">{crumb.name}</span>
            </span>
          {/if}
        {/each}
        <span class="crumb-count">{countEntries(current)}</span>
      </div>

      <div class="crumb-divider"></div>

      {#if current.children.length === 0}
        <div class="picker-empty">该分组下没有子分组,点击下方空白处或按回车确认</div>
      {:else}
        {#each current.children as child (child.uuid)}
          <button
            type="button"
            class="picker-row"
            role="option"
            aria-selected={child.uuid === current.uuid}
            onclick={() => drillTo(child.uuid)}
          >
            <AppIcon name={iconOf(child)} size={14} />
            <span class="row-name">{child.name}</span>
            <span class="row-count">{countEntries(child)}</span>
            {#if child.children.length > 0}
              <AppIcon name="chevron-right" size={11} />
            {/if}
          </button>
        {/each}
      {/if}

      <button type="button" class="picker-confirm" onclick={confirm}>
        <AppIcon name="check" size={13} />
        <span>选择当前分组</span>
      </button>
    </div>
  {/if}
</div>

<style>
  .group-picker {
    position: relative;
  }

  .picker-trigger {
    display: flex;
    align-items: center;
    gap: 8px;
    justify-content: space-between;
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: 12px;
    cursor: pointer;
    transition:
      border-color 100ms ease,
      background 100ms ease;
  }

  .picker-trigger:hover,
  .picker-trigger:focus-visible {
    border-color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 10%, var(--input-bg));
    outline: none;
  }

  .trigger-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .picker-panel {
    position: absolute;
    z-index: 30;
    top: calc(100% + 4px);
    left: 0;
    right: 0;
    max-height: 250px;
    padding: 4px;
    overflow-y: auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--surface-bg);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .picker-up,
  .picker-row,
  .picker-confirm,
  .crumb-current {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border: 1px solid transparent;
    border-radius: 4px;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    text-align: left;
  }

  .picker-up,
  .picker-row {
    cursor: pointer;
  }

  .picker-up:hover,
  .picker-row:hover {
    background: var(--hover-bg);
    color: var(--text-primary);
  }

  .picker-up {
    color: var(--selection-color);
  }

  .up-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .crumb-label,
  .crumb-name,
  .row-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .crumb-current {
    flex-wrap: wrap;
    color: var(--text-primary);
    cursor: default;
  }

  .crumb-current .crumb-link {
    border: 0;
    padding: 0;
    background: none;
    color: var(--text-secondary);
    font-size: inherit;
    cursor: pointer;
  }

  .crumb-current .crumb-link:hover {
    color: var(--text-primary);
  }

  .crumb-label {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .crumb-name {
    max-width: 160px;
  }

  .crumb-sep {
    color: var(--text-faint);
  }

  .crumb-count,
  .row-count {
    flex: 0 0 auto;
    color: var(--text-faint);
    font-size: 10px;
  }

  .crumb-divider {
    height: 1px;
    margin: 4px 6px;
    background: var(--border-color);
  }

  .picker-confirm {
    margin-top: 4px;
    border-color: color-mix(in srgb, var(--selection-color) 40%, transparent);
    color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
    cursor: pointer;
  }

  .picker-confirm:hover {
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 22%, var(--hover-bg));
  }

  .picker-empty {
    padding: 12px;
    color: var(--text-faint);
    font-size: 11px;
    text-align: center;
  }
</style>
