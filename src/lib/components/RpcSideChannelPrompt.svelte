<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { isTauriRuntime } from "$lib/services/settings";
  import { copyText } from "$lib/utils/clipboard";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface SideChannelRequest {
    password: string;
    expiresInSecs: number;
  }

  let pending = $state<SideChannelRequest | null>(null);
  let remaining = $state(0);
  let copied = $state(false);

  let unlisten: UnlistenFn | null = null;
  let tick: ReturnType<typeof setInterval> | null = null;

  function stopTick(): void {
    if (tick) {
      clearInterval(tick);
      tick = null;
    }
  }

  function startCountdown(secs: number): void {
    stopTick();
    remaining = Math.max(1, Math.round(secs));
    tick = setInterval(() => {
      remaining -= 1;
      if (remaining <= 0) stopTick();
    }, 1000);
  }

  onMount(() => {
    if (!isTauriRuntime()) return;
    let cancelled = false;
    void listen<SideChannelRequest>("rpc-side-channel-request", (event) => {
      pending = event.payload;
      copied = false;
      startCountdown(event.payload.expiresInSecs);
    }).then((stop) => {
      if (cancelled) {
        stop();
      } else {
        unlisten = stop;
      }
    });
    return () => {
      cancelled = true;
      unlisten?.();
      stopTick();
    };
  });

  const expired = $derived(pending !== null && remaining <= 0);

  const timeText = $derived.by(() => {
    const m = Math.floor(remaining / 60);
    const s = remaining % 60;
    return `${m}:${String(s).padStart(2, "0")}`;
  });

  function close(): void {
    pending = null;
    copied = false;
    stopTick();
  }

  async function copyPassword(): Promise<void> {
    if (!pending || copied || expired) return;
    try {
      await copyText(pending.password);
      copied = true;
    } catch {
      // clipboard unavailable; the password stays readable in the modal
    }
  }
</script>

{#if pending}
  <div class="modal-backdrop" role="presentation">
    <div class="approval-modal" role="dialog" aria-modal="true" aria-label="KeePassRPC 旁路密码">
      <div class="modal-head">
        <span class="modal-icon"><AppIcon name="link" size={18} /></span>
        <div>
          <strong>Kee 请求连接</strong>
          <p>在 Kee 的对话框中输入以下一次性密码完成认证</p>
        </div>
      </div>

      <div class="side-channel-card">
        <code class="side-channel-password">{pending.password}</code>
        <button
          class="copy-button"
          type="button"
          onclick={() => void copyPassword()}
          disabled={copied || expired}
        >
          {copied ? "已复制" : "复制"}
        </button>
      </div>

      <p class="approval-note">
        密码剩余
        <strong class="countdown" class:countdown-urgent={!expired && remaining <= 10}
          >{timeText}</strong
        >
        有效，仅本次连接可用；锁定数据库会立即终止连接。过期后请在 Kee 中重新连接。
      </p>

      {#if expired}
        <p class="outcome outcome-expired" aria-live="polite">旁路密码已过期，连接已关闭</p>
      {/if}

      <div class="modal-actions">
        <button class="modal-button" onclick={close}>关闭</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 80;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, #000 45%, transparent);
  }

  .approval-modal {
    display: flex;
    flex-direction: column;
    width: min(400px, calc(100% - 40px));
    padding: 18px;
    border: 1px solid var(--border-color);
    border-radius: 13px;
    background: var(--surface-bg);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
  }

  .modal-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }

  .modal-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-icon-radius, 7px);
    color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .modal-head strong {
    display: block;
    color: var(--text-primary);
    font-size: 14px;
    font-weight: 590;
  }

  .modal-head p {
    margin: 2px 0 0;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .side-channel-card {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--card-bg);
  }

  .side-channel-password {
    flex: 1;
    min-width: 0;
    color: var(--text-primary);
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: 19px;
    font-weight: 590;
    letter-spacing: 0.08em;
    word-break: break-all;
    user-select: all;
  }

  .copy-button {
    height: 26px;
    padding: 0 10px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--hover-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    cursor: pointer;
  }

  .copy-button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .approval-note {
    margin: 10px 0 0;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
    line-height: 1.5;
  }

  .countdown {
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .countdown-urgent {
    color: var(--warning-color);
  }

  .outcome {
    margin: 10px 0 0;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }

  .modal-button {
    height: 30px;
    padding: 0 14px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--hover-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    cursor: pointer;
  }

  .modal-button:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--card-bg);
  }
</style>
