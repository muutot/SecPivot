<script lang="ts">
  import { useTotpCode } from "$lib/composables/useTotpCode.svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    entryUuid: string;
  }

  let { entryUuid }: Props = $props();

  const totp = useTotpCode(() => entryUuid);
</script>

<button
  class="totp-badge"
  title={totp.code ? "点击复制验证码" : "无法生成验证码"}
  onclick={() => void totp.copy()}
  disabled={!totp.code}
>
  <span class="totp-badge-code">{totp.error ? "—" : totp.code || "••••••"}</span>
  {#if !totp.isHotp}
    <span class="totp-badge-bar" aria-hidden="true">
      <span class:low={totp.fraction < 0.25} style:width={`${totp.fraction * 100}%`}></span>
    </span>
  {/if}
  {#if totp.copied}
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