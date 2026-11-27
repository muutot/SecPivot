<script lang="ts">
  import type { Snippet } from "svelte";

  /** Button template. Variants cover the retired `.modal-button` and
   *  `.settings-action-button` primitives:
   *  - plain    dialog footer button
   *  - primary  emphasized dialog action
   *  - danger   destructive dialog action
   *  - warning  cautionary dialog action
   *  - action   compact secondary action (settings rows)
   *  - field    action aligned with a 30px input */
  export type ButtonVariant = "plain" | "primary" | "danger" | "warning" | "action" | "field";

  interface Props {
    variant?: ButtonVariant;
    type?: "button" | "submit";
    disabled?: boolean;
    /** Operation in flight: wait cursor wins over not-allowed. */
    busy?: boolean;
    title?: string;
    ariaLabel?: string;
    onclick?: (event: MouseEvent) => void;
    children: Snippet;
  }

  let {
    variant = "plain",
    type = "button",
    disabled = false,
    busy = false,
    title = undefined,
    ariaLabel = undefined,
    onclick,
    children,
  }: Props = $props();
</script>

<button
  class={["btn", `btn--${variant}`]}
  class:busy
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
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    font-size: 12px;
    line-height: 1;
    white-space: nowrap;
    cursor: pointer;
  }

  /* Dialog footer buttons (.modal-button) */
  .btn--plain,
  .btn--primary,
  .btn--danger,
  .btn--warning {
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

  .btn--warning {
    border-color: color-mix(in srgb, var(--warning-color) 50%, transparent);
    background: color-mix(in srgb, var(--warning-color) 14%, var(--card-bg));
  }

  .btn--warning:hover:not(:disabled) {
    color: var(--text-primary);
    background: color-mix(in srgb, var(--warning-color) 20%, var(--card-bg));
  }

  .btn--primary {
    border-color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
  }

  .btn--primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--selection-color) 24%, var(--card-bg));
  }

  @media (forced-colors: none) {
    :global(:root[data-theme="liquid-light"]) .btn--danger {
      color: var(--text-primary);
    }

    :global(:root[data-theme="light"]) .btn--primary {
      color: var(--text-primary);
      border-color: color-mix(in srgb, var(--link-color) 38%, var(--border-subtle));
      background: color-mix(in srgb, var(--link-color) 16%, var(--card-bg));
      box-shadow: 0 2px 5px color-mix(in srgb, var(--link-color) 8%, transparent);
    }

    :global(:root[data-theme="light"]) .btn--primary:hover:not(:disabled) {
      color: var(--text-primary);
      background: color-mix(in srgb, var(--link-color) 22%, var(--card-bg));
    }

    :global(:root[data-theme="light"]) .btn--danger {
      color: var(--text-primary);
      background: color-mix(in srgb, var(--danger-color) 12%, var(--card-bg));
    }

    :global(:root[data-theme="light"]) .btn--danger:hover:not(:disabled) {
      color: var(--text-primary);
      background: color-mix(in srgb, var(--danger-color) 18%, var(--card-bg));
    }

    :global(:root[data-theme="light"]) .btn--warning {
      color: var(--text-primary);
      border-color: hsl(from var(--warning-color) h 65% 68%);
      background: hsl(from var(--warning-color) h 90% 88%);
    }

    :global(:root[data-theme="light"]) .btn--warning:hover:not(:disabled) {
      color: var(--text-primary);
      background: hsl(from var(--warning-color) h 90% 84%);
    }
  }

  .btn--danger {
    border-color: color-mix(in srgb, var(--danger-color) 50%, transparent);
    color: color-mix(in srgb, var(--danger-color) 80%, white);
    background: color-mix(in srgb, var(--danger-color) 14%, var(--card-bg));
  }

  .btn--plain:disabled,
  .btn--primary:disabled,
  .btn--danger:disabled,
  .btn--warning:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }

  .btn.busy {
    cursor: wait;
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
