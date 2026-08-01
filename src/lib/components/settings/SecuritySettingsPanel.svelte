<script lang="ts">
  import { get } from "svelte/store";
  import { open } from "@tauri-apps/plugin-dialog";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { rememberCredential } from "$lib/services/security";
  import { vault } from "$lib/services/vault";
  import type { SecuritySettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    onclose: () => void;
    showHeader?: boolean;
  }

  let { onclose, showHeader = true }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const security = $derived(s.security);

  function change<K extends keyof SecuritySettings>(key: K, value: SecuritySettings[K]): void {
    appSettings.updateSecurity(key, value);
  }

  function sliderPercentage(value: number, min: number, max: number): number {
    return Math.round(((value - min) / (max - min)) * 100);
  }

  let newPassword = $state("");
  let confirmPassword = $state("");
  let newKeyfile = $state<string | null>(null);
  let busy = $state(false);
  let feedback = $state<{ ok: boolean; message: string } | null>(null);

  async function pickKeyfile(): Promise<void> {
    if (!isTauriRuntime()) return;
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: "密钥文件", extensions: ["key", "kdbx", "keyx", "xml"] }],
    });
    if (typeof picked === "string" && picked.length > 0) {
      newKeyfile = picked;
    }
  }

  async function submitMasterKeyChange(): Promise<void> {
    feedback = null;
    if (newPassword.length > 0 && newPassword !== confirmPassword) {
      feedback = { ok: false, message: "两次输入的新主密码不一致" };
      return;
    }
    if (newPassword.length === 0 && !newKeyfile) {
      feedback = { ok: false, message: "请输入新主密码或选择新的密钥文件" };
      return;
    }
    busy = true;
    try {
      const state = await vault.changeMasterKey(newPassword, newKeyfile);
      if (state.path && newPassword.length > 0) {
        await rememberCredential(state.path, newPassword);
      }
      newPassword = "";
      confirmPassword = "";
      newKeyfile = null;
      feedback = { ok: true, message: "主密钥已更改并保存,新密钥立即生效" };
    } catch (err) {
      feedback = { ok: false, message: String(err) };
    } finally {
      busy = false;
    }
  }
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · 安全</span>
      <h2>安全</h2>
      <p>锁定策略与剪贴板清理设置。</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label="关闭">×</button>
  </header>
{/if}

<div class="settings-scroll">
  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="lock" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>自动锁定</strong>
          <p>无操作达到时长后锁定数据库</p>
        </div>
        <span class="value-label"
          >{s.security.autoLockMinutes > 0 ? `${s.security.autoLockMinutes} 分钟` : "关闭"}</span
        >
      </div>
    </div>
    <input
      type="range"
      class="transparency-slider"
      min="0"
      max="60"
      step="1"
      value={s.security.autoLockMinutes}
      style:--slider-pct={sliderPercentage(s.security.autoLockMinutes, 0, 60)}
      oninput={(e) => change("autoLockMinutes", Number(e.currentTarget.value))}
    />
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="clock" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>剪贴板自动清理</strong>
          <p>复制密码后定时清空剪贴板</p>
        </div>
        <span class="value-label"
          >{s.security.clipboardClearSeconds > 0
            ? `${s.security.clipboardClearSeconds} 秒`
            : "关闭"}</span
        >
      </div>
    </div>
    <input
      type="range"
      class="transparency-slider"
      min="0"
      max="120"
      step="5"
      value={s.security.clipboardClearSeconds}
      style:--slider-pct={sliderPercentage(s.security.clipboardClearSeconds, 0, 120)}
      oninput={(e) => change("clipboardClearSeconds", Number(e.currentTarget.value))}
    />
  </section>

  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="copy" size={17} /></span>
      <div>
        <strong>锁定后清空剪贴板</strong>
        <p>锁定数据库时立即清除剪贴板中的密码</p>
      </div>
    </div>
    <button
      class="toggle-switch"
      class:active={security.clearOnLock}
      role="switch"
      aria-checked={security.clearOnLock}
      onclick={() => change("clearOnLock", !security.clearOnLock)}
    >
      <span class="toggle-knob"></span>
    </button>
  </section>

  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="shield" size={17} /></span>
      <div>
        <strong>关闭窗口时最小化到托盘</strong>
        <p>点击关闭按钮转入系统托盘而非退出</p>
      </div>
    </div>
    <button
      class="toggle-switch"
      class:active={security.minimizeToTray}
      role="switch"
      aria-checked={security.minimizeToTray}
      onclick={() => change("minimizeToTray", !security.minimizeToTray)}
    >
      <span class="toggle-knob"></span>
    </button>
  </section>

  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="eye-off" size={17} /></span>
      <div>
        <strong>复制后自动锁定</strong>
        <p>复制密码后立即锁定数据库</p>
      </div>
    </div>
    <button
      class="toggle-switch"
      class:active={security.lockAfterAction}
      role="switch"
      aria-checked={security.lockAfterAction}
      onclick={() => change("lockAfterAction", !security.lockAfterAction)}
    >
      <span class="toggle-knob"></span>
    </button>
  </section>

  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="unlock" size={17} /></span>
      <div>
        <strong>失去焦点时锁定</strong>
        <p>切换窗口或最小化时立即锁定数据库</p>
      </div>
    </div>
    <button
      class="toggle-switch"
      class:active={security.lockOnFocusLoss}
      role="switch"
      aria-checked={security.lockOnFocusLoss}
      onclick={() => change("lockOnFocusLoss", !security.lockOnFocusLoss)}
    >
      <span class="toggle-knob"></span>
    </button>
  </section>

  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="key" size={17} /></span>
      <div>
        <strong>记住密码(Windows Hello)</strong>
        <p>将主密码保存到系统凭据管理器,锁定后可用 Windows Hello 快速解锁</p>
      </div>
    </div>
    <button
      class="toggle-switch"
      class:active={security.rememberPassword}
      role="switch"
      aria-checked={security.rememberPassword}
      onclick={() => change("rememberPassword", !security.rememberPassword)}
    >
      <span class="toggle-knob"></span>
    </button>
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="unlock" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>更改主密钥</strong>
          <p>重新加密数据库,新的主密码或密钥文件立即生效</p>
        </div>
      </div>
    </div>
    <label class="settings-label" for="mk-new-password">新主密码(留空则仅使用密钥文件)</label>
    <input
      id="mk-new-password"
      class="settings-input settings-full-input"
      type="password"
      placeholder="新主密码"
      bind:value={newPassword}
      autocomplete="new-password"
    />
    <label class="settings-label" for="mk-confirm-password">确认新主密码</label>
    <input
      id="mk-confirm-password"
      class="settings-input settings-full-input"
      type="password"
      placeholder="再次输入新主密码"
      bind:value={confirmPassword}
      autocomplete="new-password"
    />
    <div class="setting-row mk-keyfile-row">
      <span class="mk-keyfile-name" class:mk-empty={!newKeyfile}>
        {newKeyfile ?? "未选择密钥文件(可选)"}
      </span>
      <button
        class="settings-action-button"
        type="button"
        onclick={pickKeyfile}
        disabled={!isTauriRuntime()}
      >
        选择密钥文件
      </button>
      {#if newKeyfile}
        <button
          class="settings-action-button mk-clear-button"
          type="button"
          onclick={() => (newKeyfile = null)}
        >
          清除
        </button>
      {/if}
    </div>
    <div class="mk-submit-row">
      <button
        class="settings-submit-button"
        type="button"
        onclick={submitMasterKeyChange}
        disabled={busy}
      >
        {busy ? "正在更改…" : "更改主密钥并保存"}
      </button>
    </div>
    {#if feedback}
      <p class="mk-feedback" class:mk-feedback-ok={feedback.ok}>{feedback.message}</p>
    {/if}
  </section>

  <p class="auto-save-note">修改即时生效并自动保存</p>
</div>

<style>
  .settings-full-input {
    width: 100%;
    box-sizing: border-box;
  }

  .mk-keyfile-row {
    margin-top: 12px;
  }

  .mk-keyfile-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
  }

  .mk-keyfile-name.mk-empty {
    color: var(--text-faint);
  }

  .settings-action-button {
    height: 30px;
    padding: 0 12px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--hover-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    flex: 0 0 auto;
  }

  .settings-action-button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .mk-clear-button {
    background: transparent;
  }

  .mk-submit-row {
    margin-top: 14px;
  }

  .settings-submit-button {
    width: 100%;
    height: 32px;
    border: 1px solid var(--selection-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    font-weight: 560;
  }

  .settings-submit-button:hover {
    background: color-mix(in srgb, var(--selection-color) 26%, var(--card-bg));
  }

  .settings-submit-button:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  .mk-feedback {
    margin: 10px 0 0;
    padding: 7px 9px;
    border: 1px solid color-mix(in srgb, var(--danger-color) 35%, transparent);
    border-radius: var(--settings-control-radius, 6px);
    color: color-mix(in srgb, var(--danger-color) 75%, white);
    background: color-mix(in srgb, var(--danger-color) 12%, var(--surface-bg));
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
  }

  .mk-feedback.mk-feedback-ok {
    border-color: color-mix(in srgb, var(--success-color) 35%, transparent);
    color: color-mix(in srgb, var(--success-color) 75%, white);
    background: color-mix(in srgb, var(--success-color) 12%, var(--surface-bg));
  }
</style>
