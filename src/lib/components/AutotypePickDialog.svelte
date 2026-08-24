<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import type { AutotypeCandidate } from "$lib/types/vault";

  interface Props {
    candidates: AutotypeCandidate[];
    onclose: () => void;
    /** Failure feedback for a picked candidate (page toast). */
    onerror: (message: string) => void;
  }

  let { candidates, onclose, onerror }: Props = $props();

  function pick(candidate: AutotypeCandidate): void {
    // Close the picker immediately (responsive UX) but surface a failed pick
    // instead of leaving the user with nothing typed and no feedback.
    void invoke("autotype_pick", {
      sessionId: candidate.sessionId,
      uuid: candidate.uuid,
    }).catch((e) => onerror(`自动键入失败：${e}`));
    onclose();
  }
</script>

<ModalShell
  title="选择要自动填充的条目"
  description="多个条目匹配当前窗口，请选择其一"
  size="small"
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    <div class="autotype-pick-list" role="listbox" aria-label="自动填充候选">
      {#each candidates as candidate (candidate.uuid)}
        <button
          type="button"
          class="autotype-pick-item"
          role="option"
          aria-selected="false"
          onclick={() => pick(candidate)}
        >
          <span class="autotype-pick-title">{candidate.title || "未命名条目"}</span>
          {#if candidate.username}
            <span class="autotype-pick-username">{candidate.username}</span>
          {/if}
        </button>
      {/each}
    </div>
  {/snippet}
</ModalShell>

<style>
  .autotype-pick-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 40vh;
    overflow: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .autotype-pick-item {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    padding: 8px 10px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
    color: var(--text-primary);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .autotype-pick-item:hover {
    background: var(--hover-bg);
  }

  .autotype-pick-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .autotype-pick-username {
    flex: 1;
    overflow: hidden;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
