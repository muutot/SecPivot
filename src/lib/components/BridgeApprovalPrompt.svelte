<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { isTauriRuntime } from "$lib/services/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";

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
  {@const request = pending}
  <ModalShell
    title="浏览器请求关联"
    description="浏览器客户端请求读取当前数据库凭据"
    ariaLabel="浏览器关联授权"
    size="medium"
    prompt
  >
    {#snippet icon()}<AppIcon name="plug" size={18} />{/snippet}
    {#snippet children()}
      <div class="client-card">
        <span class="client-label">客户端 ID</span>
        <code class="client-id">{request.id}</code>
      </div>

      <p class="approval-note">
        批准后将允许该客户端获取此数据库中的用户名与密码；密钥仅在本次会话内有效，锁定数据库即失效。
      </p>

      {#if outcome}
        <p class="outcome" class:outcome-expired={outcome === "expired"}>
          {outcomeMsg}
        </p>
      {/if}
    {/snippet}
    {#snippet actions()}
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
    {/snippet}
  </ModalShell>
{/if}

<style>
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
</style>
