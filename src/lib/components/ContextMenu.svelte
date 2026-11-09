<script lang="ts">
  import type { IconName } from "$lib/components/AppIcon.svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ViewportMenuShell from "$lib/components/ViewportMenuShell.svelte";
  import MenuItem from "$lib/components/templates/menu/MenuItem.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  export interface ContextMenuItem {
    id: string;
    label: string;
    icon: IconName;
    destructive?: boolean;
    disabled?: boolean;
    /** Cascade child list; a parent item toggles its submenu instead of
     *  dispatching an action. */
    children?: ContextMenuItem[];
  }

  interface Props {
    x: number;
    y: number;
    items: ContextMenuItem[];
    onclose: () => void;
    onaction: (id: string) => void;
  }

  let { x, y, items, onclose, onaction }: Props = $props();

  /** id of the currently expanded cascade submenu. */
  let openSubmenu = $state<string | null>(null);

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  function close(): void {
    openSubmenu = null;
    onclose();
  }

  function dispatch(id: string): void {
    openSubmenu = null;
    onaction(id);
  }

  /** Fixed positioning keeps cascades outside the scrollable parent surface.
   *  Reposition on parent scroll and viewport/content resize. */
  function submenuFlip(node: HTMLDivElement): { destroy(): void } {
    const flip = (): void => {
      const anchor = node.parentElement?.getBoundingClientRect();
      if (!anchor) return;
      const rect = node.getBoundingClientRect();
      const preferredX =
        anchor.right + 4 + rect.width <= window.innerWidth - 8
          ? anchor.right + 4
          : anchor.left - rect.width - 4;
      node.style.left = `${Math.min(Math.max(8, preferredX), Math.max(8, window.innerWidth - rect.width - 8))}px`;
      node.style.top = `${Math.min(Math.max(8, anchor.top - 4), Math.max(8, window.innerHeight - rect.height - 8))}px`;
    };
    flip();
    const ro = new ResizeObserver(flip);
    ro.observe(node);
    window.addEventListener("resize", flip);
    window.addEventListener("scroll", flip, true);
    return {
      destroy() {
        ro.disconnect();
        window.removeEventListener("resize", flip);
        window.removeEventListener("scroll", flip, true);
      },
    };
  }
</script>

<ViewportMenuShell {x} {y} onclose={close} ariaLabel={t(lang, "menu.contextMenu")}>
  {#snippet children()}
    {#each items as item}
      {@const hasChildren = Boolean(item.children?.length)}
      <div
        class="submenu-anchor"
        role="none"
        onpointerenter={() => (openSubmenu = hasChildren ? item.id : null)}
      >
        <MenuItem
          label={item.label}
          destructive={item.destructive}
          disabled={item.disabled}
          onclick={() =>
            hasChildren
              ? (openSubmenu = openSubmenu === item.id ? null : item.id)
              : dispatch(item.id)}
        >
          {#snippet leading()}<AppIcon name={item.icon} size={15} />{/snippet}
          {#if hasChildren}
            {#snippet trailing()}
              <span class="submenu-chevron" class:open={openSubmenu === item.id}>
                <AppIcon name="chevron-right" size={12} />
              </span>
            {/snippet}
          {/if}
        </MenuItem>
        {#if hasChildren && openSubmenu === item.id}
          <div class="submenu" use:submenuFlip>
            {#each item.children as child (child.id)}
              <MenuItem
                label={child.label}
                destructive={child.destructive}
                disabled={child.disabled}
                onclick={() => dispatch(child.id)}
              >
                {#snippet leading()}<AppIcon name={child.icon} size={15} />{/snippet}
              </MenuItem>
            {/each}
          </div>
        {/if}
      </div>
    {/each}
  {/snippet}
</ViewportMenuShell>

<style>
  .submenu-anchor {
    position: relative;
  }

  .submenu {
    position: fixed;
    z-index: 9999;
    min-width: min(190px, calc(100vw - 16px));
    max-width: calc(100vw - 16px);
    max-height: calc(100dvh - 16px);
    overflow: auto;
    overscroll-behavior: contain;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
    padding: 4px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--surface-bg);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
  }

  .submenu-chevron {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    flex-shrink: 0;
    color: var(--text-faint);
    transition: transform 100ms ease;
  }

  .submenu-chevron.open {
    transform: rotate(90deg);
  }
</style>
