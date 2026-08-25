<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import type { IconName } from "$lib/components/AppIcon.svelte";
  import Toggle from "$lib/components/templates/form/Toggle.svelte";

  interface Props {
    icon: IconName;
    label: string;
    description: string;
    checked: boolean;
    ariaLabel?: string;
    disabled?: boolean;
    onchange: (checked: boolean) => void;
  }

  let {
    icon,
    label,
    description,
    checked,
    ariaLabel,
    disabled = false,
    onchange,
  }: Props = $props();
</script>

<section class="setting-card toggle-card">
  <div class="setting-heading">
    <span class="setting-icon"><AppIcon name={icon} size={17} /></span>
    <div>
      <strong>{label}</strong>
      <p>{description}</p>
    </div>
  </div>
  <Toggle bind:checked {disabled} ariaLabel={ariaLabel ?? label} onchange={(value) => onchange(value)} />
</section>
