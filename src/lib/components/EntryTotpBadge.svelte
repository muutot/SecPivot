<script lang="ts">
  import { computeTotp } from "$lib/utils/totp";
  import { copyText } from "$lib/utils/clipboard";
  import type { TotpCode } from "$lib/types/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    seed: string;
  }

  let { seed }: Props = $props();

  let code = $state("");
  let remaining = $state(0);
  let period = $state(30);
  let error = $state(false);
  let copied = $state(false);

  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  async function refresh(now = Date.now()): Promise<boolean> {
    try {
      const result: TotpCode = await computeTotp(seed, now);
      code = result.code;
      remaining = result.validFor;
      period = result.period;
      error = false;
      return true;
    } catch {
      code = "";
      error = true;
      return false;
    }
  }

  $effect(() => {
    if (!seed) return;
    let timer: ReturnType<typeof setInterval> | undefined;
    const tick = async (): Promise<void> => {
      // A failing seed (invalid TOTP URI) must stop the per-second loop
      // instead of recomputing (WebCrypto) forever.
      if (!(await refresh()) && timer) clearInterval(timer);
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

<button
  class="totp-badge"
  title={code ? "点击复制验证码" : "无法生成验证码"}
  onclick={() => void copy()}
  disabled={!code}
>
  <span class="totp-badge-code">{error ? "—" : code || "••••••"}</span>
  <span class="totp-badge-bar" aria-hidden="true">
    <span class:low={fraction < 0.25} style:width={`${fraction * 100}%`}></span>
  </span>
  {#if copied}
    <span class="totp-badge-copied"><AppIcon name="check" size={11} /></span>
  {/if}
</button>

<style>
  .totp-badge {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 8px;
    border: 1px solid color-mix(in srgb, var(--selection-color) 40%, transparent);
    border-radius: 6px;
    background: color-mix(in srgb, var(--selection-color) 10%, var(--card-bg));
    color: var(--selection-color);
    cursor: pointer;
  }

  .totp-badge:disabled {
    cursor: default;
    opacity: 0.6;
  }

  .totp-badge:hover:not(:disabled) {
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
  }

  .totp-badge-code {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 11px;
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.06em;
    line-height: 1;
  }

  .totp-badge-bar {
    position: relative;
    display: inline-block;
    width: 22px;
    height: 3px;
    border-radius: 2px;
    background: var(--hover-bg);
    overflow: hidden;
  }

  .totp-badge-bar span {
    position: absolute;
    inset: 0 auto 0 0;
    display: block;
    border-radius: 2px;
    background: var(--success-color);
    transition: width 300ms linear;
  }

  .totp-badge-bar span.low {
    background: var(--warning-color);
  }

  .totp-badge-copied {
    position: absolute;
    top: 50%;
    right: 8px;
    transform: translateY(-50%);
    color: var(--success-color);
  }
</style>
