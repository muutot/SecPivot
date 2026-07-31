<script lang="ts">
  import { vault } from "$lib/services/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    remembered: { path: string; fileName: string } | null;
    onopened: () => void;
    onswitch: () => void;
  }

  let { remembered, onopened, onswitch }: Props = $props();

  let password = $state("");
  let showPassword = $state(false);
  let busy = $state(false);
  let error = $state("");
  let inputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    inputEl?.focus();
  });

  async function unlock(): Promise<void> {
    if (!remembered) return;
    if (!password) {
      error = "请输入主密码";
      return;
    }
    busy = true;
    error = "";
    try {
      await vault.open(remembered.path, password);
      onopened();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="welcome">
  <div class="welcome-inner">
    <div class="welcome-logo"><AppIcon name="lock" size={34} /></div>
    <h1 class="welcome-title">数据库已锁定</h1>
    {#if remembered}
      <p class="lock-file">{remembered.fileName}</p>
      <p class="lock-path" title={remembered.path}>{remembered.path}</p>
    {/if}

    <div class="unlock-row">
      <input
        class="text-input"
        type={showPassword ? "text" : "password"}
        bind:value={password}
        placeholder="输入主密码解锁"
        bind:this={inputEl}
        disabled={busy}
        onkeydown={(e) => {
          if (e.key === "Enter") void unlock();
        }}
      />
      <button class="browse-button" onclick={() => (showPassword = !showPassword)} title="显示密码">
        <AppIcon name={showPassword ? "eye-off" : "eye"} size={15} />
      </button>
    </div>

    {#if error}
      <p class="modal-error">{error}</p>
    {/if}

    <div class="unlock-actions">
      <button class="welcome-button" onclick={onswitch} disabled={busy} title="选择其他数据库">
        使用其他数据库
      </button>
      <button
        class="welcome-button primary"
        onclick={() => void unlock()}
        disabled={busy || !password}
      >
        {busy ? "解锁中…" : "解锁"}
      </button>
    </div>
  </div>
</div>

<style>
  .welcome {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
  }

  .welcome-inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    max-width: 380px;
    padding: 24px;
    text-align: center;
  }

  .welcome-logo {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 64px;
    height: 64px;
    border: 1px solid var(--border-color);
    border-radius: 16px;
    color: var(--warning-color);
    background: var(--card-bg);
  }

  .welcome-title {
    margin: 16px 0 0;
    font-size: 24px;
    font-weight: 590;
    letter-spacing: 0.01em;
  }

  .lock-file {
    margin: 10px 0 0;
    color: var(--text-secondary);
    font-size: 13px;
  }

  .lock-path {
    margin: 3px 0 0;
    max-width: 100%;
    overflow: hidden;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .unlock-row {
    display: flex;
    gap: 6px;
    width: 100%;
    margin-top: 20px;
  }

  .text-input {
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: 12px;
  }

  .text-input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .browse-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    cursor: pointer;
  }

  .browse-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .modal-error {
    margin: 10px 0 0;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
  }

  .unlock-actions {
    display: flex;
    gap: 10px;
    margin-top: 18px;
  }

  .welcome-button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--card-bg);
    font-size: 12px;
    cursor: pointer;
  }

  .welcome-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .welcome-button.primary {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
  }

  .welcome-button.primary:hover {
    background: color-mix(in srgb, var(--selection-color) 26%, var(--card-bg));
  }

  .welcome-button:disabled {
    cursor: wait;
    opacity: 0.6;
  }
</style>
