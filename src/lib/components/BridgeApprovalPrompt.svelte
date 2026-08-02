<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauriRuntime } from "$lib/services/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface AssociateRequest {
    token: string;
    id: string;
  }

  let pending = $state<AssociateRequest | null>(null);
  let busy = $state(false);
  let outcome = $state<"ok" | "expired" | "error" | null>(null);
  let outcomeMsg = $state("");

  let unlisten: UnlistenFn | null = null;

  onMount(() => {
    if (!isTauriRuntime()) return;
    let cancelled = false;
    void listen<AssociateRequest>("bridge-associate-request", (event) => {
      pending = event.payload;
      busy = false;
      outcome = null;
      outcomeMsg = "";
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
    };
  });

  async function decide(allowed: boolean): Promise<void> {
    if (!pending || busy) return;
    busy = true;
    try {
      await invoke("bridge_approve", { token: pending.token, allowed });
      outcome = "ok";
      outcomeMsg = allowed ? "已授权该客户端" : "已拒绝该客户端";
    } catch (e) {
      outcome = "expired";
      outcomeMsg = String(e);
    } finally {
      busy = false;
    }
    setTimeout(() => {
      pending = null;
      outcome = null;
      outcomeMsg = "";
    }, 1400);
  }
</script>

{#if pending}
  <div class="modal-backdrop" role="presentation">
    <div class="approval-modal" role="dialog" aria-modal="true" aria-label="浏览器关联授权">
      <div class="modal-head">
        <span class="modal-icon"><AppIcon name="plug" size={18} /></span>
        <div>
          <strong>浏览器请求关联</strong>
          <p>浏览器客户端请求读取当前数据库凭据</p>
        </div>
      </div>

      <div class="client-card">
        <span class="client-label">客户端 ID</span>
        <code class="client-id">{pending.id}</code>
      </div>

      <p class="approval-note">
        批准后将允许该客户端获取此数据库中的用户名与密码；密钥仅在本次会话内有效，锁定数据库即失效。
      </p>

      {#if outcome}
        <p class="outcome" class:outcome-expired={outcome === "expired"}>
          {outcomeMsg}
        </p>
      {/if}

      <div class="modal-actions">
        <button
          class="modal-button"
          onclick={() => void decide(false)}
          disabled={busy || outcome !== null}
        >
          拒绝
        </button>
        <button
          class="modal-button primary"
          onclick={() => void decide(true)}
          disabled={busy || outcome !== null}
        >
          允许
        </button>
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

  .client-card {
    padding: 10px 12px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--card-bg);
  }

  .client-label {
    display: block;
    margin-bottom: 4px;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .client-id {
    display: block;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary);
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: var(--font-size-secondary, 11px);
  }

  .approval-note {
    margin: 10px 0 0;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
    line-height: 1.5;
  }

  .outcome {
    margin: 10px 0 0;
    color: var(--success-color);
    font-size: var(--font-size-secondary, 11px);
  }

  .outcome.outcome-expired {
    color: var(--warning-color);
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

  .modal-button.primary {
    border-color: color-mix(in srgb, var(--selection-color) 45%, transparent);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 16%, var(--card-bg));
  }

  .modal-button.primary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--selection-color) 24%, var(--card-bg));
  }

  .modal-button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }
</style>
