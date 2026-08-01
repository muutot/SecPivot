<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { vault } from "$lib/services/vault";
  import { rememberCredential } from "$lib/services/security";
  import { isTauriRuntime } from "$lib/services/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    remembered: { path: string; fileName: string } | null;
    onopened: () => void;
    onswitch: () => void;
  }

  let { remembered, onopened, onswitch }: Props = $props();

  let password = $state("");
  let keyfilePath = $state("");
  let showPassword = $state(false);
  let busy = $state(false);
  let error = $state("");
  let helloAvailable = $state(false);
  let inputEl = $state<HTMLInputElement | null>(null);

  $effect(() => {
    inputEl?.focus();
  });

  /** Probe the credential store for the remembered path so the Hello button only shows when usable. */
  $effect(() => {
    const path = remembered?.path;
    if (!path || !isTauriRuntime()) {
      helloAvailable = false;
      return;
    }
    void invoke<{ password?: string } | null>("get_saved_credential", { path })
      .then((result) => {
        helloAvailable = result != null;
      })
      .catch(() => {
        helloAvailable = false;
      });
  });

  async function pickKeyfile(): Promise<void> {
    const selected = await open({
      multiple: false,
      filters: [{ name: "密钥文件", extensions: ["key", "keyx", "xml", "txt"] }],
    });
    if (selected) keyfilePath = String(selected);
  }

  async function unlock(): Promise<void> {
    if (!remembered) return;
    if (!password && !keyfilePath) {
      error = "请输入主密码或选择密钥文件";
      return;
    }
    busy = true;
    error = "";
    try {
      await vault.open(remembered.path, password, keyfilePath || undefined);
      void rememberCredential(remembered.path, password);
      onopened();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function unlockWithHello(): Promise<void> {
    if (!remembered) return;
    busy = true;
    error = "";
    try {
      const saved = await invoke<{ password?: string } | null>("get_saved_credential", {
        path: remembered.path,
      });
      if (!saved?.password) {
        error = "没有已保存的凭据,请先在设置中启用“记住密码(Windows Hello)”";
        helloAvailable = false;
        return;
      }
      await vault.open(remembered.path, saved.password);
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

    {#if isTauriRuntime()}
      <div class="unlock-row">
        <input
          class="text-input"
          type="text"
          bind:value={keyfilePath}
          placeholder="密钥文件(可选)"
          readonly
          disabled={busy}
        />
        <button class="browse-button" onclick={pickKeyfile} title="选择密钥文件">
          <AppIcon name="folder" size={15} />
        </button>
      </div>
    {/if}

    {#if error}
      <p class="modal-error">{error}</p>
    {/if}

    <div class="unlock-actions">
      <button class="welcome-button" onclick={onswitch} disabled={busy} title="选择其他数据库">
        使用其他数据库
      </button>
      {#if helloAvailable}
        <button
          class="welcome-button hello"
          onclick={() => void unlockWithHello()}
          disabled={busy}
          title="使用系统凭据快速解锁"
        >
          <AppIcon name="unlock" size={15} />Windows Hello
        </button>
      {/if}
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

  .welcome-button.hello {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    color: var(--text-primary);
  }

  .welcome-button.hello:hover {
    background: color-mix(in srgb, var(--accent) 14%, var(--card-bg));
  }

  .welcome-button:disabled {
    cursor: wait;
    opacity: 0.6;
  }
</style>
