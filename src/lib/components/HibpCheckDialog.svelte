<script lang="ts">
  import type { BreachFinding } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";

  interface Props {
    uuids?: string[];
    onclose: () => void;
    onselect?: (uuid: string) => void;
  }

  let { uuids = [], onclose, onselect }: Props = $props();

  // Strict opt-in: the check only runs after the user explicitly clicks
  // "开始检查" on the privacy screen.
  let started = $state(false);
  let running = $state(false);
  let findings = $state<BreachFinding[]>([]);
  let error = $state("");
  const sessionId = vault.getActiveSessionId();

  async function start(): Promise<void> {
    if (running || !sessionId) return;
    running = true;
    error = "";
    started = true;
    try {
      const value = await vault.callInSession(sessionId, () =>
        vault.checkHibp(uuids.length > 0 ? uuids : undefined),
      );
      if (vault.getActiveSessionId() !== sessionId) return;
      findings = value;
    } catch (e) {
      if (vault.getActiveSessionId() !== sessionId) return;
      error = String(e);
    } finally {
      running = false;
    }
  }
</script>

<ModalShell
  title="HIBP 泄露检查"
  description="检查密码是否出现在已知数据泄露中"
  size="medium"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    {#if !started}
      <div class="privacy">
        <p>本功能连接 Have I Been Pwned（api.pwnedpasswords.com）进行 k-anonymity 前缀查询：</p>
        <ul>
          <li>密码与完整 SHA-1 散列绝不会离开本机；只发送散列前 5 位十六进制字符。</li>
          <li>仅在你点击「开始检查」时执行一次，不会自动或后台运行。</li>
          <li>需要网络连接；无网络时检查会失败并提示。</li>
        </ul>
      </div>
    {:else if running}
      <p class="note">正在检查…</p>
    {:else if error}
      <p class="note error">{error}</p>
    {:else if findings.length === 0}
      <p class="note success">未发现密码出现在已知泄露数据中。</p>
    {:else}
      <p class="note">发现 {findings.length} 个密码出现在已知泄露中（按出现次数排序）：</p>
      <ul class="list">
        {#each findings as finding (finding.uuid)}
          <li class="row">
            <button
              type="button"
              class="main"
              onclick={() => onselect?.(finding.uuid)}
              title="定位条目"
            >
              <span class="title">{finding.title}</span>
              <span class="sub">{finding.username}</span>
            </button>
            <span class="count">泄露 {finding.count} 次</span>
          </li>
        {/each}
      </ul>
      <p class="note">建议立即为这些条目更换密码。</p>
    {/if}
  {/snippet}
  {#snippet actions()}
    {#if !started}
      <button class="modal-button" onclick={onclose}>取消</button>
      <button class="modal-button primary" onclick={() => void start()}>开始检查</button>
    {:else}
      <button class="modal-button primary" onclick={onclose}>关闭</button>
    {/if}
  {/snippet}
</ModalShell>

<style>
  .privacy p {
    margin: 0 0 8px;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    line-height: 1.5;
  }

  .privacy ul {
    margin: 0;
    padding-left: 18px;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
    line-height: 1.6;
  }

  .note {
    margin: 8px 0;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
  }

  .note.error {
    color: var(--danger-color);
  }

  .note.success {
    color: var(--success-color);
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

  .count {
    flex: none;
    color: var(--danger-color);
    font-size: var(--font-size-tiny, 10px);
  }
</style>
