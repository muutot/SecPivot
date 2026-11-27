<script lang="ts">
  import { onMount, type Snippet } from "svelte";
  import { attachMenuKeyboard } from "$lib/utils/menu-keyboard";
  import { liquidGlass } from "$lib/utils/liquid-glass";

  interface Props {
    x: number;
    y: number;
    onclose: () => void;
    ariaLabel: string;
    role?: string;
    modifier?: string;
    navigateSubmenu?: (event: KeyboardEvent) => boolean;
    children?: Snippet;
  }

  let {
    x,
    y,
    onclose,
    ariaLabel,
    role = "menu",
    modifier = "",
    navigateSubmenu,
    children,
  }: Props = $props();

  let menuEl = $state<HTMLDivElement>();
  /** `null` until the first measured clamp pass. */
  let posX = $state<number | null>(null);
  let posY = $state<number | null>(null);

  $effect(() => {
    if (!menuEl) return;
    const clamp = (): void => {
      if (!menuEl) return;
      const rect = menuEl.getBoundingClientRect();
      const vw = window.innerWidth;
      const vh = window.innerHeight;
      posX = Math.min(Math.max(8, x), Math.max(8, vw - rect.width - 8));
      posY = Math.min(Math.max(8, y), Math.max(8, vh - rect.height - 8));
    };
    clamp();
    const observer = new ResizeObserver(clamp);
    observer.observe(menuEl);
    window.addEventListener("resize", clamp);
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", clamp);
    };
  });

  onMount(() => {
    if (menuEl)
      return attachMenuKeyboard(
        menuEl,
        () => onclose(),
        (event) => navigateSubmenu?.(event) ?? false,
      );
  });
</script>

<div
  class="viewport-menu"
  class:viewport-menu--column-config={modifier === "column-config"}
  bind:this={menuEl}
  use:liquidGlass
  style:left="{posX ?? x}px"
  style:top="{posY ?? y}px"
  {role}
  aria-label={ariaLabel}
  tabindex="-1"
>
  <div class="viewport-menu__content">
    {#if children}{@render children()}{/if}
  </div>
</div>

<style>
  .viewport-menu {
    position: fixed;
    z-index: 9999;
    min-width: min(160px, calc(100vw - 16px));
    max-width: calc(100vw - 16px);
    max-height: calc(100dvh - 16px);
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--glass-menu-base, var(--surface-bg));
    box-shadow: var(--glass-surface-shadow, 0 8px 32px rgba(0, 0, 0, 0.5));
  }

  /* Keep optics on the decorative layer: filtering the menu itself would
     make its fixed cascades relative to it and clip them while scrolling. */
  .viewport-menu::before {
    content: var(--glass-menu-decoration, none);
    position: absolute;
    inset: 0;
    z-index: -1;
    border-radius: inherit;
    pointer-events: none;
    background: var(--glass-surface-fill, var(--surface-bg));
    backdrop-filter: var(--liquid-refraction-filter, var(--glass-filter, none));
  }

  .viewport-menu__content {
    max-height: calc(100dvh - 18px);
    overflow: auto;
    overscroll-behavior: contain;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
    padding: 4px;
  }

  .viewport-menu--column-config {
    min-width: min(190px, calc(100vw - 16px));
    max-width: min(260px, calc(100vw - 16px));
    max-height: 60vh;
  }

  .viewport-menu--column-config .viewport-menu__content {
    max-height: calc(60vh - 2px);
  }
</style>
