<script lang="ts">
  import { onMount } from "svelte";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  import { copyText } from "$lib/utils/clipboard";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";

  import Button from "$lib/components/templates/action/Button.svelte";
  interface SideChannelRequest {
    password: string;
    expiresInSecs: number;
  }

  let pending = $state<SideChannelRequest | null>(null);
  let remaining = $state(0);
  let copied = $state(false);

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  let unlisten: UnlistenFn | null = null;
  let tick: ReturnType<typeof setInterval> | null = null;
  let deadline = 0;

  function stopTick(): void {
    if (tick !== null) {
      clearInterval(tick);
      tick = null;
    }
  }

  function refreshCountdown(): void {
    if (!pending) return;
    remaining = Math.max(0, Math.ceil((deadline - Date.now()) / 1000));
    if (remaining === 0) close();
  }

  function startCountdown(secs: number): void {
    stopTick();
    if (!Number.isFinite(secs) || secs <= 0) {
      close();
      return;
    }
    deadline = Date.now() + secs * 1000;
    refreshCountdown();
    if (!pending) return;
    const request = pending;
    tick = setInterval(() => {
      if (pending === request) refreshCountdown();
    }, 1000);
  }

  onMount(() => {
    if (!isTauriRuntime()) return;
    let cancelled = false;
    document.addEventListener("visibilitychange", refreshCountdown);
    window.addEventListener("focus", refreshCountdown);
    void listen<SideChannelRequest>("rpc-side-channel-request", (event) => {
      if (cancelled) return;
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
      document.removeEventListener("visibilitychange", refreshCountdown);
      window.removeEventListener("focus", refreshCountdown);
      close();
    };
  });

  const timeText = $derived.by(() => {
    const m = Math.floor(remaining / 60);
    const s = remaining % 60;
    return `${m}:${String(s).padStart(2, "0")}`;
  });

  function close(): void {
    pending = null;
    copied = false;
    remaining = 0;
    deadline = 0;
    stopTick();
  }

  async function copyPassword(): Promise<void> {
    // Timers may be suspended while the window is hidden or the device sleeps.
    refreshCountdown();
    const request = pending;
    if (!request || copied) return;
    try {
      await copyText(request.password);
      if (pending !== request) return;
      refreshCountdown();
      if (pending === request) copied = true;
    } catch {
      // clipboard unavailable; the password stays readable in the modal
    }
  }
</script>

{#if pending}
  {@const request = pending}
  <ModalShell
    title={t(lang, "rpcprompt.title")}
    description={t(lang, "rpcprompt.desc")}
    ariaLabel={t(lang, "rpcprompt.aria")}
    size="medium"
    prompt
  >
    {#snippet icon()}<AppIcon name="link" size={18} />{/snippet}
    {#snippet children()}
      <div class="side-channel-card">
        <code class="side-channel-password">{request.password}</code>
        <button
          class="copy-button"
          type="button"
          onclick={() => void copyPassword()}
          disabled={copied}
        >
          {copied ? t(lang, "rpcprompt.copied") : t(lang, "rpcprompt.copy")}
        </button>
      </div>

      <p class="approval-note">
        {t(lang, "rpcprompt.remaining")}
        <strong class="countdown" class:countdown-urgent={remaining <= 10}>{timeText}</strong>
        {t(lang, "rpcprompt.note")}
      </p>
    {/snippet}
    {#snippet actions()}
      <Button onclick={close}>{t(lang, "common.close")}</Button>
    {/snippet}
  </ModalShell>
{/if}

<style>
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
    font-family: var(--font-mono);
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
    background: var(--card-bg);
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
</style>
