<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    onclose: () => void;
    showHeader?: boolean;
    appVersion?: string;
  }

  let { onclose, showHeader = true, appVersion = "0.1.0" }: Props = $props();
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · 关于</span>
      <h2>关于</h2>
      <p>版本与技术信息。</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label="关闭">×</button>
  </header>
{/if}

<div class="settings-scroll">
  <section class="setting-card about-card">
    <div class="about-brand">
      <span class="about-logo"><AppIcon name="key" size={26} /></span>
      <div>
        <strong class="about-name">KeyVault</strong>
        <p class="about-version">v{appVersion}</p>
        <p class="about-tagline">专业、紧凑、高密度的开源 KeePass 客户端</p>
      </div>
    </div>
    <dl class="about-grid">
      <div class="about-item">
        <dt>技术栈</dt>
        <dd>Svelte 5 · Tauri 2 · Rust</dd>
      </div>
      <div class="about-item">
        <dt>格式</dt>
        <dd>KDBX 4.0（兼容 KeePass 2.x）</dd>
      </div>
      <div class="about-item">
        <dt>加密</dt>
        <dd>AES-256 / ChaCha20 · Argon2id</dd>
      </div>
      <div class="about-item">
        <dt>许可证</dt>
        <dd>MIT</dd>
      </div>
    </dl>
  </section>

  <p class="auto-save-note">数据仅保存在本地数据库文件中</p>
</div>

<style>
  .about-brand {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .about-logo {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 52px;
    height: 52px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-card-radius, 9px);
    color: var(--warning-color);
    background: var(--hover-bg);
  }

  .about-name {
    display: block;
    color: var(--text-primary);
    font-size: 16px;
    font-weight: 590;
  }

  .about-version {
    margin: 2px 0 0;
    color: var(--text-faint);
    font-size: var(--settings-note-size, var(--font-size-tiny, 10px));
    font-variant-numeric: tabular-nums;
  }

  .about-tagline {
    margin: 6px 0 0;
    color: var(--text-muted);
    font-size: var(--settings-description-size, var(--font-size-secondary, 11px));
  }

  .about-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px 18px;
    margin: 16px 0 0;
  }

  .about-item {
    min-width: 0;
  }

  .about-item dt {
    margin: 0 0 2px;
    color: var(--text-faint);
    font-size: var(--settings-note-size, var(--font-size-tiny, 10px));
  }

  .about-item dd {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
  }
</style>
