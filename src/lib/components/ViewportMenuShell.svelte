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
  let posX = $state(0);
  let posY = $state(0);

  $effect(() => {
    if (menuEl) {
      const rect = menuEl.getBoundingClientRect();
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      posX = Math.min(x, vw - rect.width - 8);
      posY = Math.min(y, vh - rect.height - 8);
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
  style:left="{posX || x}px"
  style:top="{posY || y}px"
  {role}
  aria-label={ariaLabel}
>
  {#if children}{@render children()}{/if}
</div>
