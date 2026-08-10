<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import type { IconName } from "$lib/components/AppIcon.svelte";

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
  <button
    class="toggle-switch"
    class:active={checked}
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={ariaLabel ?? label}
    {disabled}
    onclick={() => onchange(!checked)}
  >
    <span class="toggle-knob"></span>
  </button>
</section>
