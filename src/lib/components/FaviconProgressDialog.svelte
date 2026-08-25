<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import type { FaviconProgress } from "$lib/types/vault";

  import Button from "$lib/components/templates/action/Button.svelte";
  interface Props {
    dialog: {
      phase: "working" | "done";
      progress: FaviconProgress;
      result: string;
      error: boolean;
    };
    onclose: () => void;
  }

  let { dialog, onclose }: Props = $props();

  const progressPct = $derived(
    dialog.progress.total > 0
      ? `${Math.round((dialog.progress.done / dialog.progress.total) * 100)}%`
      : "0%",
  );
</script>

<ModalShell
  title={dialog.error ? "下载图标失败" : "下载网址图标"}
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
    {#if dialog.phase !== "working"}
      <Button variant="primary" onclick={onclose}>关闭</Button>
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
