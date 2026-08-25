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
      <MenuItem
        label={item.label}
        destructive={item.destructive}
        disabled={item.disabled}
        onclick={() => {
          onaction(item.id);
          onclose();
        }}
      >
        {#snippet leading()}<AppIcon name={item.icon} size={15} />{/snippet}
      </MenuItem>
    {/each}
  {/snippet}
</ViewportMenuShell>
