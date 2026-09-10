<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";

  import Button from "$lib/components/templates/action/Button.svelte";
  interface AssociateRequest {
    token: string;
    id: string;
  }

  let pending = $state<AssociateRequest | null>(null);
  let busy = $state(false);
  let outcome = $state<"ok" | "expired" | "error" | null>(null);
  let outcomeMsg = $state("");

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  let unlisten: UnlistenFn | null = null;
  let dismissTimer: ReturnType<typeof setTimeout> | undefined;

  onMount(() => {
    if (!isTauriRuntime()) return;
    let cancelled = false;
    void listen<AssociateRequest>("bridge-associate-request", (event) => {
      if (dismissTimer) {
        clearTimeout(dismissTimer);
        dismissTimer = undefined;
      }
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
      if (dismissTimer) {
        clearTimeout(dismissTimer);
        dismissTimer = undefined;
      }
    };
  });

  async function decide(allowed: boolean): Promise<void> {
    if (!pending || busy) return;
    busy = true;
    try {
      await invoke("bridge_approve", { token: pending.token, allowed });
      outcome = "ok";
      outcomeMsg = allowed ? t(lang, "approval.allowed") : t(lang, "approval.denied");
    } catch (e) {
      outcome = "expired";
      outcomeMsg = String(e);
    } finally {
      busy = false;
    }
    if (dismissTimer) clearTimeout(dismissTimer);
    dismissTimer = setTimeout(() => {
      pending = null;
      outcome = null;
      outcomeMsg = "";
    }, 1400);
  }
</script>

{#if pending}
  {@const request = pending}
  <ModalShell
    title={t(lang, "approval.title")}
    description={t(lang, "approval.desc")}
    ariaLabel={t(lang, "approval.aria")}
    size="medium"
    prompt
  >
    {#snippet icon()}<AppIcon name="plug" size={18} />{/snippet}
    {#snippet children()}
      <div class="client-card">
        <span class="client-label">{t(lang, "approval.clientId")}</span>
        <code class="client-id">{request.id}</code>
      </div>

      <p class="approval-note">
        {t(lang, "approval.note")}
      </p>

      {#if outcome}
        <p class="outcome" class:outcome-expired={outcome === "expired"}>
          {outcomeMsg}
        </p>
      {/if}
    {/snippet}
    {#snippet actions()}
      <Button onclick={() => void decide(false)} disabled={busy || outcome !== null}
        >{t(lang, "approval.deny")}</Button
      >
      <Button
        variant="primary"
        onclick={() => void decide(true)}
        disabled={busy || outcome !== null}
      >
        {t(lang, "approval.allow")}</Button
      >
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
    font-family: var(--font-mono);
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
