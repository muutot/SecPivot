<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import type { FaviconProgress } from "$lib/types/vault";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  import Button from "$lib/components/templates/action/Button.svelte";
  interface Props {
    dialog: {
      phase: "working" | "done";
      progress: FaviconProgress;
      result: string;
      error: boolean;
    };
    onclose: () => void;
    oncancel?: () => void;
  }

  let { dialog, onclose, oncancel }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  const progressPct = $derived(
    dialog.progress.total > 0
      ? `${Math.round((dialog.progress.done / dialog.progress.total) * 100)}%`
      : "0%",
  );
</script>

<ModalShell
  title={dialog.error ? t(lang, "favicon.failedTitle") : t(lang, "favicon.title")}
  description={dialog.result}
  size="small"
  tone={dialog.error ? "danger" : "default"}
  closeOnEscape={dialog.phase !== "working"}
  {onclose}
>
  {#snippet icon()}
    <AppIcon name={dialog.error ? "x" : "globe"} size={16} />
  {/snippet}
  {#snippet children()}
    {#if dialog.phase === "working"}
      <div class="progress-track">
        <div
          class="progress-fill"
          class:indeterminate={dialog.progress.total === 0}
          style:--progress-pct={progressPct}
        ></div>
      </div>
    {/if}
  {/snippet}
  {#snippet actions()}
    {#if dialog.phase === "working"}
      <Button onclick={() => oncancel?.()}>{t(lang, "common.cancelWait")}</Button>
    {:else}
      <Button variant="primary" onclick={onclose}>{t(lang, "common.close")}</Button>
    {/if}
  {/snippet}
</ModalShell>

<style>
  .progress-track {
    height: 6px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
    overflow: hidden;
  }

  .progress-fill {
    width: var(--progress-pct, 0%);
    height: 100%;
    border-radius: inherit;
    background: var(--selection-color);
    transition: width 0.2s ease;
  }

  .progress-fill.indeterminate {
    width: 40%;
    animation: progress-slide 1.1s ease-in-out infinite alternate;
  }

  @keyframes progress-slide {
    from {
      transform: translateX(-110%);
    }
    to {
      transform: translateX(260%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .progress-fill {
      transition: none;
    }
    .progress-fill.indeterminate {
      animation: none;
    }
  }
</style>
