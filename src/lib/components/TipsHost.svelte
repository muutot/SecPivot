<script lang="ts">
  import { tips } from "$lib/services/tips";

  let list = $state<{ id: number; message: string; kind: "success" | "error" }[]>([]);
  $effect(() => {
    const unsubscribe = tips.subscribe((value) => {
      list = value;
    });
    return unsubscribe;
  });
</script>

<div class="tips-host" aria-live="polite">
  {#each list as tip (tip.id)}
    <p class="tip" class:error={tip.kind === "error"}>{tip.message}</p>
  {/each}
</div>

<style>
  .tips-host {
    position: fixed;
    left: 50%;
    bottom: 12px;
    z-index: 1000;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    transform: translateX(-50%);
    pointer-events: none;
  }

  .tip {
    margin: 0;
    padding: 6px 12px;
    border: 1px solid color-mix(in srgb, var(--success-color) 40%, transparent);
    border-radius: var(--settings-feedback-radius, 7px);
    color: color-mix(in srgb, var(--success-color) 80%, white);
    background: color-mix(in srgb, var(--success-color) 12%, var(--surface-bg));
    font-size: var(--font-size-secondary, 11px);
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.35);
  }

  .tip.error {
    border-color: color-mix(in srgb, var(--danger-color) 40%, transparent);
    color: color-mix(in srgb, var(--danger-color) 80%, white);
    background: color-mix(in srgb, var(--danger-color) 12%, var(--surface-bg));
  }
</style>
