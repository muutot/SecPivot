<script lang="ts">
  /** Switch template (settings-shared `.toggle-switch`). The knob is part of
   *  the template — an unlabeled track is unrepresentable. */
  interface Props {
    checked: boolean;
    ariaLabel?: string;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
  }

  let { checked = $bindable(false), ariaLabel, disabled = false, onchange }: Props = $props();

  function toggle(): void {
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  class="toggle"
  class:active={checked}
  role="switch"
  aria-checked={checked}
  aria-label={ariaLabel}
  {disabled}
  onclick={toggle}
>
  <span class="knob"></span>
</button>

<style>
  .toggle {
    position: relative;
    width: 40px;
    height: 22px;
    flex-shrink: 0;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: 12px;
    background: var(--input-bg);
    cursor: pointer;
    transition:
      border-color 100ms ease,
      background 100ms ease;
  }

  .toggle.active {
    border-color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 18%, transparent);
  }

  .toggle:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text-faint);
    transition:
      transform 120ms ease,
      background 100ms ease;
  }

  .toggle.active .knob {
    transform: translateX(18px);
    background: var(--selection-color);
  }
</style>
