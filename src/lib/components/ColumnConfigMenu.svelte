<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";

  export interface ColumnMenuSection {
    label: string;
    items: { id: string; label: string; visible: boolean }[];
  }

  interface Props {
    x: number;
    y: number;
    sections: ColumnMenuSection[];
    onclose: () => void;
    ontoggle: (id: string) => void;
  }

  let { x, y, sections, onclose, ontoggle }: Props = $props();

  let menuEl = $state<HTMLDivElement>();
  let posX = $state(0);
  let posY = $state(0);

  function adjustPosition(width: number, height: number): void {
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    posX = Math.min(x, vw - width - 8);
    posY = Math.min(y, vh - height - 8);
  }

  $effect(() => {
    if (menuEl) {
      const rect = menuEl.getBoundingClientRect();
      adjustPosition(rect.width, rect.height);
    }
  });

  function handleKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }

  function handleClickOutside(e: MouseEvent): void {
    if (menuEl && !menuEl.contains(e.target as Node)) {
      onclose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} onclick={handleClickOutside} />

<div
  class="column-config-menu"
  bind:this={menuEl}
  style:left="{posX || x}px"
  style:top="{posY || y}px"
  role="menu"
  aria-label="列配置"
>
  {#each sections as section}
    <div class="ccm-section-title">{section.label}</div>
    {#each section.items as item}
      <button
        type="button"
        class="menu-item"
        class:checked={item.visible}
        role="menuitemcheckbox"
        aria-checked={item.visible}
        onclick={() => ontoggle(item.id)}
      >
        <span class="menu-check"><AppIcon name="check" size={13} /></span>
        <span class="menu-label" title={item.label}>{item.label}</span>
      </button>
    {/each}
  {/each}
  <div class="ccm-footer">右键列标题可快速显隐列</div>
</div>

<style>
  .column-config-menu {
    position: fixed;
    z-index: 9999;
    min-width: 190px;
    max-width: 260px;
    max-height: 60vh;
    padding: 4px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--surface-bg);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    overflow: auto;
  }

  .ccm-section-title {
    padding: 5px 10px 3px;
    color: var(--text-faint);
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.04em;
  }

  .ccm-section-title:not(:first-child) {
    margin-top: 4px;
    border-top: 1px solid var(--border-subtle);
  }

  .menu-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    text-align: left;
    cursor: pointer;
    transition: background 80ms ease;
  }

  .menu-item:hover {
    background: var(--hover-bg);
    color: var(--text-primary);
  }

  .menu-check {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    flex-shrink: 0;
    color: var(--text-faint);
    opacity: 0;
  }

  .menu-item.checked .menu-check {
    opacity: 1;
    color: var(--selection-color);
  }

  .menu-label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ccm-footer {
    margin-top: 4px;
    padding: 5px 10px 3px;
    border-top: 1px solid var(--border-subtle);
    color: var(--text-faint);
    font-size: 9px;
  }
</style>
