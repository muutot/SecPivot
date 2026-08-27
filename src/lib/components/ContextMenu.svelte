<script lang="ts">
  import type { IconName } from "$lib/components/AppIcon.svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ViewportMenuShell from "$lib/components/ViewportMenuShell.svelte";
  import MenuItem from "$lib/components/templates/menu/MenuItem.svelte";

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

  function close(): void {
    openSubmenu = null;
    onclose();
  }

  function dispatch(id: string): void {
    openSubmenu = null;
    onaction(id);
  }
</script>

<ViewportMenuShell {x} {y} onclose={close} ariaLabel="上下文菜单">
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
          <div class="submenu">
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
    position: absolute;
    top: -4px;
    left: calc(100% + 4px);
    z-index: 1;
    min-width: 190px;
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
