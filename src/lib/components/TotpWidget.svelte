<script lang="ts">
  import { useTotpCode } from "$lib/composables/useTotpCode.svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    entryUuid: string;
  }

  let { entryUuid }: Props = $props();

  const totp = useTotpCode(() => entryUuid);
</script>

<div class="totp-widget">
  <div class="totp-code-row">
    <span class="totp-code mono">{totp.code || "••••••"}</span>
    {#if totp.code}
      <button class="totp-copy" onclick={() => void totp.copy()} title="复制验证码">
        <AppIcon name={totp.copied ? "check" : "copy"} size={13} />
      </button>
    {/if}
  </div>
  {#if !totp.isHotp}
    <div class="totp-bar" aria-hidden="true">
      <span class:low={totp.fraction < 0.25} style:width={`${totp.fraction * 100}%`}></span>
    </div>
  {/if}
  <div class="totp-meta">
    {#if totp.error}
      <span class="totp-error">无法生成验证码</span>
    {:else if totp.isHotp}
      <span>HOTP · 第 {totp.counter ?? 0} 次</span>
    {:else if totp.kind === "steam"}
      <span>Steam · {totp.remaining}s 后刷新</span>
    {:else}
      <span>{totp.remaining}s 后刷新</span>
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
