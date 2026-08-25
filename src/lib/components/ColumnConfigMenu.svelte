<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ViewportMenuShell from "$lib/components/ViewportMenuShell.svelte";
  import MenuItem from "$lib/components/templates/menu/MenuItem.svelte";

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
</script>

<ViewportMenuShell {x} {y} {onclose} ariaLabel="列配置" modifier="column-config">
  {#snippet children()}
    {#each sections as section}
      <div class="ccm-section-title">{section.label}</div>
      {#each section.items as item}
        <MenuItem
          role="menuitemcheckbox"
          checked={item.visible}
          label={item.label}
          title={item.label}
          onclick={() => ontoggle(item.id)}
        >
          {#snippet leading()}<AppIcon name="check" size={13} />{/snippet}
        </MenuItem>
      {/each}
    {/each}
    <div class="ccm-footer">右键列标题可快速显隐列</div>
  {/snippet}
</ViewportMenuShell>

<style>
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

  .ccm-footer {
    margin-top: 4px;
    padding: 5px 10px 3px;
    border-top: 1px solid var(--border-subtle);
    color: var(--text-faint);
    font-size: 9px;
  }
</style>
