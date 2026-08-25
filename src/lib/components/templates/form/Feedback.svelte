<script lang="ts">
  /** Feedback banner template (`.settings-feedback`): danger by default,
   *  `success` flips the palette; `inline` renders in-flow inside a card
   *  instead of pinned to the panel bottom. */
  interface Props {
    message: string;
    success?: boolean;
    inline?: boolean;
  }

  let { message, success = false, inline = false }: Props = $props();
</script>

<p class="feedback" class:success class:inline role="status">{message}</p>

<style>
  .feedback {
    position: absolute;
    right: 18px;
    bottom: 13px;
    left: 18px;
    padding: 8px 10px;
    border: 1px solid color-mix(in srgb, var(--danger-color) 35%, transparent);
    border-radius: var(--settings-feedback-radius, 7px);
    color: color-mix(in srgb, var(--danger-color) 75%, white);
    background: color-mix(in srgb, var(--danger-color) 12%, var(--surface-bg));
    font-size: var(--settings-feedback-size, var(--font-size-secondary, 11px));
  }

  .feedback.success {
    border-color: color-mix(in srgb, var(--success-color) 35%, transparent);
    color: color-mix(in srgb, var(--success-color) 75%, white);
    background: color-mix(in srgb, var(--success-color) 12%, var(--surface-bg));
  }

  .feedback.inline {
    position: static;
    margin: 10px 0 0;
    padding: 7px 9px;
    border-radius: var(--settings-control-radius, 6px);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
  }
</style>
