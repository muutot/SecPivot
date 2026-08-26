<script lang="ts">
  import type { Snippet } from "svelte";

  /** Button template. Variants cover the retired `.modal-button` and
   *  `.settings-action-button` primitives:
   *  - plain    dialog footer button
   *  - primary  emphasized dialog action
   *  - danger   destructive dialog action
   *  - action   compact secondary action (settings rows)
   *  - field    action aligned with a 30px input */
  export type ButtonVariant = "plain" | "primary" | "danger" | "action" | "field";

  interface Props {
    variant?: ButtonVariant;
    type?: "button" | "submit";
    disabled?: boolean;
    title?: string;
    ariaLabel?: string;
    onclick?: (event: MouseEvent) => void;
    children: Snippet;
  }

  let {
    variant = "plain",
    type = "button",
    disabled = false,
    title = undefined,
    ariaLabel = undefined,
    onclick,
    children,
  }: Props = $props();
</script>

<button
  class={["btn", `btn--${variant}`]}
  {type}
  {disabled}
  {title}
  aria-label={ariaLabel}
  {onclick}
>
  {@render children()}
</button>

<style>
  .btn {
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    font-size: 12px;
    cursor: pointer;
  }

  /* Dialog footer buttons (.modal-button) */
  .btn--plain,
  .btn--primary,
  .btn--danger {
    height: 30px;
    padding: 0 14px;
    color: var(--text-secondary);
    background: var(--card-bg);
  }

  .btn--plain:hover:not(:disabled),
  .btn--primary:hover:not(:disabled),
  .btn--danger:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .btn--primary {
    border-color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
  }

  .btn--primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--selection-color) 24%, var(--card-bg));
  }

  .btn--danger {
    border-color: color-mix(in srgb, var(--danger-color) 50%, transparent);
    color: color-mix(in srgb, var(--danger-color) 80%, white);
    background: color-mix(in srgb, var(--danger-color) 14%, var(--card-bg));
  }

  .btn--plain:disabled,
  .btn--primary:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  /* Compact secondary actions (.settings-action-button) */
  .btn--action,
  .btn--field {
    height: 26px;
    padding: 0 10px;
    flex: 0 0 auto;
    color: var(--text-secondary);
    background: var(--card-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
  }

  .btn--field {
    height: 30px;
    padding-inline: 12px;
  }

  .btn--action:disabled,
  .btn--field:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }
</style>
