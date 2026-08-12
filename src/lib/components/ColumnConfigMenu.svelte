<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ViewportMenuShell from "$lib/components/ViewportMenuShell.svelte";

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
