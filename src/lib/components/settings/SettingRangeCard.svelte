<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import type { IconName } from "$lib/components/AppIcon.svelte";

  interface Props {
    icon: IconName;
    label: string;
    description: string;
    value: number;
    valueLabel: string;
    min: number;
    max: number;
    step?: number;
    ariaLabel?: string;
    disabled?: boolean;
    onchange: (value: number) => void;
  }

  let {
    icon,
    label,
    description,
    value,
    valueLabel,
    min,
    max,
    step = 1,
    ariaLabel,
    disabled = false,
    onchange,
  }: Props = $props();

  const percentage = $derived.by(() => {
    if (max <= min) return 0;
    return Math.round(Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100)));
  });
</script>

<section class="setting-card">
  <div class="setting-heading">
    <span class="setting-icon"><AppIcon name={icon} size={17} /></span>
    <div class="heading-inline">
      <div>
        <strong>{label}</strong>
        <p>{description}</p>
      </div>
      <span class="value-label">{valueLabel}</span>
    </div>
  </div>
  <input
    type="range"
    class="transparency-slider"
    {min}
    {max}
    {step}
    {value}
    aria-label={ariaLabel ?? label}
    {disabled}
    style:--slider-pct={`${percentage}%`}
    oninput={(event) => onchange(Number(event.currentTarget.value))}
  />
</section>
