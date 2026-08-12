<script lang="ts">
  import type { ExpiredEntry } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import { formatLocalDate } from "$lib/utils/date";

  interface Props {
    onclose: () => void;
    onselect?: (uuid: string) => void;
  }

  let { onclose, onselect }: Props = $props();

  let entries = $state<ExpiredEntry[]>([]);
  let loading = $state(true);
  let error = $state("");
  let busyAction = $state(false);

  async function refresh(): Promise<void> {
    try {
      entries = await vault.expiredEntries();
      loading = false;
    } catch (e) {
      error = String(e);
      loading = false;
    }
  }

  $effect(() => {
    void refresh();
  });

  function extendIso(): string {
    return new Date(Date.now() + 30 * 24 * 3600 * 1000).toISOString();
  }

  async function extend(uuids: string[]): Promise<void> {
    if (busyAction || uuids.length === 0) return;
    busyAction = true;
    error = "";
    try {
      await vault.updateEntries(uuids, { expires: extendIso() });
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busyAction = false;
    }
  }

  async function remove(uuids: string[]): Promise<void> {
    if (busyAction || uuids.length === 0) return;
    if (!window.confirm(`将 ${uuids.length} 个过期条目移入回收站？`)) return;
    busyAction = true;
    error = "";
    try {
      await vault.deleteEntries(uuids);
      await refresh();
    } catch (e) {
      error = String(e);
    } finally {
      busyAction = false;
    }
  }
</script>

<ModalShell
  title="过期条目"
  description="集中处理已过期的条目（延期 30 天或移入回收站）"
  size="large"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    {#if loading}
      <p class="note">正在加载…</p>
    {:else if error}
      <p class="note error">{error}</p>
    {:else if entries.length === 0}
      <p class="note">没有过期条目。</p>
    {:else}
      <p class="note">共 {entries.length} 个过期条目：</p>
      <ul class="list">
        {#each entries as entry (entry.uuid)}
          <li class="row">
            <button
              type="button"
              class="main"
              onclick={() => onselect?.(entry.uuid)}
              title="定位条目"
            >
              <span class="title">{entry.title}</span>
              <span class="sub">{entry.username}{entry.url ? ` · ${entry.url}` : ""}</span>
              <span class="when">{formatLocalDate(entry.expires)}</span>
            </button>
            <div class="actions">
              <button type="button" class="mini" onclick={() => void extend([entry.uuid])}>
                延期 30 天
              </button>
              <button type="button" class="mini danger" onclick={() => void remove([entry.uuid])}>
                删除
              </button>
            </div>
          </li>
        {/each}
      </ul>
    {/if}
  {/snippet}
  {#snippet actions()}
    {#if entries.length > 0}
      <button
        class="modal-button"
        disabled={busyAction}
        onclick={() => void extend(entries.map((e) => e.uuid))}
      >
        全部延期 30 天
      </button>
      <button
        class="modal-button danger"
        disabled={busyAction}
        onclick={() => void remove(entries.map((e) => e.uuid))}
      >
        全部删除
      </button>
    {/if}
    <button class="modal-button primary" onclick={onclose}>关闭</button>
  {/snippet}
</ModalShell>

<style>
  .note {
    margin: 8px 0;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
  }

  .note.error {
    color: var(--danger-color);
  }

  .list {
    margin: 0;
    padding: 0;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .row:last-child {
    border-bottom: none;
  }

  .main {
    display: flex;
    flex: 1;
    min-width: 0;
    flex-direction: column;
    gap: 2px;
    padding: 0;
    border: none;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .title {
    overflow: hidden;
    color: var(--text-primary);
    font-size: var(--font-size-secondary, 11px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    overflow: hidden;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .when {
    flex: none;
    color: var(--warning-color);
    font-size: var(--font-size-tiny, 10px);
  }

  .actions {
    display: flex;
    flex: none;
    gap: 6px;
  }

  .mini {
    height: 24px;
    padding: 0 8px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: transparent;
    font-size: var(--font-size-tiny, 10px);
    cursor: pointer;
  }

  .mini:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .mini.danger {
    color: var(--danger-color);
  }
</style>
