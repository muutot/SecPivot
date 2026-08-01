<script lang="ts">
  import type { TotpCode } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import { copyText } from "$lib/utils/clipboard";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    seed: string;
    entryUuid: string;
  }

  let { seed, entryUuid }: Props = $props();

  let code = $state("");
  let remaining = $state(0);
  let period = $state(30);
  let error = $state("");
  let copied = $state(false);

  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  async function load(): Promise<boolean> {
    try {
      const result: TotpCode = await vault.totpCode(entryUuid);
      code = result.code;
      remaining = result.validFor;
      period = result.period;
      error = "";
      return true;
    } catch (e) {
      code = "";
      error = String(e);
      return false;
    }
  }

  $effect(() => {
    if (!seed) return;
    let timer: ReturnType<typeof setInterval> | undefined;
    const tick = async (): Promise<void> => {
      // A failing seed (invalid TOTP URI) must stop the per-second loop
      // instead of hammering the backend forever.
      if (!(await load()) && timer) clearInterval(timer);
    };
    void tick();
    timer = setInterval(() => {
      remaining -= 1;
      if (remaining <= 0) void tick();
    }, 1000);
    return () => {
      if (timer) clearInterval(timer);
    };
  });

  const fraction = $derived(period > 0 ? Math.max(0, remaining) / period : 0);

  async function copy(): Promise<void> {
    if (!code) return;
    try {
      await copyText(code);
      copied = true;
      if (copiedTimer) clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => {
        copied = false;
        copiedTimer = undefined;
      }, 1200);
    } catch {
      // clipboard unavailable; ignore
    }
  }
</script>

<div class="totp-widget">
  <div class="totp-code-row">
    <span class="totp-code mono">{code || "••••••"}</span>
    {#if code}
      <button class="totp-copy" onclick={() => void copy()} title="复制验证码">
        <AppIcon name={copied ? "check" : "copy"} size={13} />
      </button>
    {/if}
  </div>
  <div class="totp-bar" aria-hidden="true">
    <span class:low={fraction < 0.25} style:width={`${fraction * 100}%`}></span>
  </div>
  <div class="totp-meta">
    {#if error}
      <span class="totp-error">无法生成验证码</span>
    {:else}
      <span>{remaining}s 后刷新</span>
    {/if}
  </div>
</div>

<style>
  .totp-widget {
    min-width: 0;
    flex: 1;
  }

  .totp-code-row {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .totp-code {
    flex: 1;
    min-width: 0;
    color: var(--selection-color);
    font-size: 22px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.06em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .totp-copy {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex: 0 0 auto;
    padding: 0;
    border: 0;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
    cursor: pointer;
  }

  .totp-copy:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .totp-bar {
    height: 4px;
    margin-top: 8px;
    border-radius: 2px;
    background: var(--hover-bg);
    overflow: hidden;
  }

  .totp-bar span {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--success-color);
    transition: width 300ms linear;
  }

  .totp-bar span.low {
    background: var(--warning-color);
  }

  .totp-meta {
    margin-top: 4px;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }

  .totp-error {
    color: var(--danger-color);
  }
</style>
