<script lang="ts">
  import type { IconName } from "$lib/components/AppIcon.svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ViewportMenuShell from "$lib/components/ViewportMenuShell.svelte";

  export interface ContextMenuItem {
    id: string;
    label: string;
    icon: IconName;
    destructive?: boolean;
    disabled?: boolean;
  }

  interface Props {
    x: number;
    y: number;
    items: ContextMenuItem[];
    onclose: () => void;
    onaction: (id: string) => void;
  }

  let { x, y, items, onclose, onaction }: Props = $props();
</script>

<ViewportMenuShell {x} {y} {onclose} ariaLabel="上下文菜单">
  {#snippet children()}
    {#each items as item}
      <button
        type="button"
        class="menu-item"
        class:destructive={item.destructive}
        role="menuitem"
        disabled={item.disabled}
        onclick={() => {
          onaction(item.id);
          onclose();
        }}
      >
        <span class="menu-icon"><AppIcon name={item.icon} size={15} /></span>
        <span class="menu-label">{item.label}</span>
      </button>
    {/each}
  {/snippet}
</ViewportMenuShell>

<style>
  .menu-item.destructive {
    color: var(--danger-color);
  }

  .menu-item.destructive:hover {
    background: color-mix(in srgb, var(--danger-color) 12%, transparent);
    color: var(--danger-color);
  }

  .menu-icon {
    display: inline-flex;
    align-items: center;
    width: 18px;
    flex-shrink: 0;
  }
</style>
