<script lang="ts">
  import { get } from "svelte/store";
  import { open } from "@tauri-apps/plugin-dialog";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { rememberCredential } from "$lib/services/security";
  import { vault } from "$lib/services/vault";
  import type { SecuritySettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";
  import SettingToggleCard from "$lib/components/settings/SettingToggleCard.svelte";

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
  <SettingRangeCard
    icon="lock"
    label="自动锁定"
    description="无操作达到时长后锁定数据库"
    value={s.security.autoLockMinutes}
    valueLabel={s.security.autoLockMinutes > 0 ? `${s.security.autoLockMinutes} 分钟` : "关闭"}
    min={0}
    max={60}
    onchange={(value) => change("autoLockMinutes", value)}
  />

  <SettingRangeCard
    icon="clock"
    label="剪贴板自动清理"
    description="复制密码后定时清空剪贴板"
    value={s.security.clipboardClearSeconds}
    valueLabel={s.security.clipboardClearSeconds > 0
      ? `${s.security.clipboardClearSeconds} 秒`
      : "关闭"}
    min={0}
    max={120}
    step={5}
    onchange={(value) => change("clipboardClearSeconds", value)}
  />

  <SettingToggleCard
    icon="copy"
    label="锁定后清空剪贴板"
    description="锁定数据库时立即清除剪贴板中的密码"
    checked={security.clearOnLock}
    onchange={(checked) => change("clearOnLock", checked)}
  />

  <SettingToggleCard
    icon="shield"
    label="关闭窗口时最小化到托盘"
    description="点击关闭按钮转入系统托盘而非退出"
    checked={security.minimizeToTray}
    onchange={(checked) => change("minimizeToTray", checked)}
  />

  <SettingToggleCard
    icon="eye-off"
    label="复制后自动锁定"
    description="复制密码后立即锁定数据库"
    checked={security.lockAfterAction}
    onchange={(checked) => change("lockAfterAction", checked)}
  />

  <SettingToggleCard
    icon="unlock"
    label="失去焦点时锁定"
    description="切换窗口或最小化时立即锁定数据库"
    checked={security.lockOnFocusLoss}
    onchange={(checked) => change("lockOnFocusLoss", checked)}
  />

  <SettingToggleCard
    icon="key"
    label="记住密码(Windows Hello)"
    description="将主密码保存到系统凭据管理器,锁定后可用 Windows Hello 快速解锁"
    checked={security.rememberPassword}
    ariaLabel="记住密码"
    onchange={(checked) => change("rememberPassword", checked)}
  />

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
        class="settings-action-button settings-action-button--field"
        type="button"
        onclick={pickKeyfile}
        disabled={!isTauriRuntime()}
      >
        选择密钥文件
      </button>
      {#if newKeyfile}
        <button
          class="settings-action-button settings-action-button--field mk-clear-button"
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
