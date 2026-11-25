<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import type { FaviconProgress } from "$lib/types/vault";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";

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
  {#snippet headerActions()}
    <!-- High-density header action: the wait is cancelled from the title bar
         (mirrors the entry editor's header buttons), so the body keeps only
         the progress bar. Stop-waiting routes to oncancel, which lets the
         backend finish into the done phase. -->
    <button
      class="icon-btn"
      type="button"
      onclick={dialog.phase === "working" ? () => oncancel?.() : onclose}
      aria-label={dialog.phase === "working"
        ? t(lang, "common.cancelWait")
        : t(lang, "common.close")}
      title={dialog.phase === "working" ? t(lang, "common.cancelWait") : t(lang, "common.close")}
    >
      <AppIcon name="x" size={15} />
    </button>
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
</ModalShell>

<style>
  /* Mirrors the entry editor's header icon buttons (EntryEditorDialog
     `.icon-btn`): 32px bordered square, token-driven colors. */
  .icon-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    cursor: pointer;
  }

  .icon-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

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
