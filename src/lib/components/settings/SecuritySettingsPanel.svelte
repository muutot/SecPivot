<script lang="ts">
  import { appSettings } from "$lib/services/settings";
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

  <p class="auto-save-note">修改即时生效并自动保存</p>
</div>
