<script lang="ts">
  import type { SimilarPasswordGroup } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";

  interface Props {
    onclose: () => void;
    onselect?: (uuid: string) => void;
  }

  let { onclose, onselect }: Props = $props();

  let groups = $state<SimilarPasswordGroup[]>([]);
  let loading = $state(true);
  let error = $state("");

  $effect(() => {
    void vault
      .similarPasswords()
      .then((value) => {
        groups = value;
        loading = false;
      })
      .catch((e) => {
        error = String(e);
        loading = false;
      });
  });
</script>

<ModalShell
  title="相似密码"
  description="编辑距离 ≤ 2 的密码分组（密码不会离开本机）"
  size="large"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    {#if loading}
      <p class="note">正在分析…</p>
    {:else if error}
      <p class="note error">{error}</p>
    {:else if groups.length === 0}
      <p class="note">未发现相似密码。</p>
    {:else}
      <p class="note">共 {groups.length} 组：</p>
      {#each groups as group, gi (gi)}
        <section class="group">
          <h3 class="group-title">组 {gi + 1}（{group.entries.length} 个条目）</h3>
          <ul class="members">
            {#each group.entries as member (member.uuid)}
              <li>
                <button
                  type="button"
                  class="member"
                  onclick={() => onselect?.(member.uuid)}
                  title="定位条目"
                >
                  <span class="member-title">{member.title}</span>
                  <span class="member-user">{member.username}</span>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    {/if}
  {/snippet}
  {#snippet actions()}
    <button class="modal-button" onclick={onclose}>关闭</button>
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

  .group {
    margin-bottom: 14px;
  }

  .group-title {
    margin: 0 0 6px;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    font-weight: 600;
  }

  .members {
    margin: 0;
    padding: 0;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    list-style: none;
  }

  .member {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border: none;
    border-bottom: 1px solid var(--border-subtle);
    color: var(--text-primary);
    background: transparent;
    font-size: var(--font-size-secondary, 11px);
    text-align: left;
    cursor: pointer;
  }

  .member:last-child {
    border-bottom: none;
  }

  .member:hover {
    background: var(--hover-bg);
  }

  .member-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .member-user {
    color: var(--text-faint);
  }
</style>
