<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    x: number;
    y: number;
    onclose: () => void;
    ariaLabel: string;
    role?: string;
    modifier?: string;
    children?: Snippet;
  }

  let { x, y, onclose, ariaLabel, role = "menu", modifier = "", children }: Props = $props();

  let menuEl = $state<HTMLDivElement>();
  /** `null` until the first clamp pass; a clamped position of exactly 0 is
   *  valid and must not fall back to the raw cursor coordinate. */
  let posX = $state<number | null>(null);
  let posY = $state<number | null>(null);

  $effect(() => {
    if (menuEl) {
      const rect = menuEl.getBoundingClientRect();
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      posX = Math.min(x, Math.max(0, vw - rect.width - 8));
      posY = Math.min(y, Math.max(0, vh - rect.height - 8));
    }
  });

  function handleKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      onclose();
    }
  }

  function handleClickOutside(event: MouseEvent): void {
    if (menuEl && !menuEl.contains(event.target as Node)) {
      onclose();
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} onclick={handleClickOutside} />

<div
  class="viewport-menu"
  class:viewport-menu--column-config={modifier === "column-config"}
  bind:this={menuEl}
  style:left="{posX ?? x}px"
  style:top="{posY ?? y}px"
  {role}
  aria-label={ariaLabel}
>
  {#if children}{@render children()}{/if}
</div>

<style>
  .viewport-menu {
    position: fixed;
    z-index: 9999;
    min-width: 160px;
    padding: 4px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--surface-bg);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  }

  .viewport-menu--column-config {
    min-width: 190px;
    max-width: 260px;
    max-height: 60vh;
    overflow: auto;
  }
</style>
