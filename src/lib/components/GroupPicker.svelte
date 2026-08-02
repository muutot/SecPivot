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
  /** The group currently being browsed (the one whose children are listed). */
  let browseUuid = $state<string | null>(null);
  let pickerEl = $state<HTMLDivElement>();

  const root = $derived(groups[0] ?? null);

  function findGroup(parent: VaultGroup, uuid: string): VaultGroup | null {
    if (parent.uuid === uuid) return parent;
    for (const child of parent.children) {
      const found = findGroup(child, uuid);
      if (found) return found;
    }
    return null;
  }

  /** Path from root to the browsed/committed group (top-down). */
  const chain = $derived.by(() => {
    const path: VaultGroup[] = [];
    if (!root) return path;
    const targetUuid = browseUuid ?? value;
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
    walk(root, targetUuid);
    if (path.length === 0 && root) path.push(root);
    return path;
  });

  const current = $derived(chain[chain.length - 1] ?? null);

  /** Commit group as the picker value. Passing explicitly keeps the prop-decoupled. */
  function commit(uuid: string): void {
    onchange(uuid);
    open = false;
    browseUuid = null;
  }

  function close(): void {
    open = false;
    browseUuid = null;
  }

  function handleKeydown(e: KeyboardEvent): void {
    if (!open) return;
    if (e.key === "Escape") {
      e.preventDefault();
      close();
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (current) commit(current.uuid);
    }
  }

  function handleClickOutside(e: MouseEvent): void {
    if (open && pickerEl && !pickerEl.contains(e.target as Node)) {
      close();
    }
  }

  function toggle(): void {
    if (open) {
      close();
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
    <span class="trigger-value" title={chain.map((g) => g.name).join(" / ")}>
      {chain.map((g) => g.name).join(" / ") || "—"}
    </span>
    <AppIcon name="chevron-down" size={13} />
  </button>

  {#if open && current}
    <div class="picker-panel" role="listbox" aria-label="选择分组" tabindex="-1">
      <!-- Breadcrumb: click to browse upward; the last crumb is the browsed group. -->
      <div class="crumb-current">
        {#each chain as crumb, i (crumb.uuid)}
          {#if i > 0}
            <span class="crumb-sep">/</span>
          {/if}
          {#if i < chain.length - 1}
            <button
              type="button"
              class="crumb-link"
              onclick={(e) => {
                e.stopPropagation();
                browseUuid = crumb.uuid;
              }}
            >
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

      <!-- Child groups: a click only browses into the children, never commits. -->
      {#if current.children.length === 0}
        <div class="picker-empty">该分组下没有子分组</div>
      {:else}
        {#each current.children as child (child.uuid)}
          <button
            type="button"
            class="picker-row"
            role="option"
            aria-selected={child.uuid === value}
            onclick={(e) => {
              if (e.detail > 1) {
                commit(child.uuid);
              } else {
                e.stopPropagation();
                browseUuid = child.uuid;
              }
            }}
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

      <div class="crumb-divider"></div>

      <!-- Explicit commit: whole row acts as the confirm target // anything else stays browse-only. -->
      <button type="button" class="picker-confirm" onclick={() => commit(current.uuid)}>
        <AppIcon name="check" size={13} />
        <span>确认</span>
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

  .trigger-value {
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

  .crumb-current {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    width: 100%;
    padding: 6px 10px;
    color: var(--text-primary);
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
    overflow: hidden;
  }

  .crumb-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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

  .picker-row,
  .picker-confirm {
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
    background: transparent;
    cursor: pointer;
  }

  .picker-row:hover {
    background: var(--hover-bg);
    color: var(--text-primary);
  }

  .row-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .picker-confirm {
    border-color: color-mix(in srgb, var(--selection-color) 40%, transparent);
    color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
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
