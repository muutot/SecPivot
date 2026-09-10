<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  interface Props {
    onclose: () => void;
    showHeader?: boolean;
    appVersion?: string;
  }

  let { onclose, showHeader = true, appVersion = "0.1.0" }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  interface AppInfo {
    portable: boolean;
    configPath: string;
    dataDir: string;
  }

  let info = $state<AppInfo | null>(null);

  $effect(() => {
    void (async () => {
      if (!isTauriRuntime()) return;
      try {
        info = await invoke<AppInfo>("app_info");
      } catch {
        info = null;
      }
    })();
  });
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · {t(lang, "about.title")}</span>
      <h2>{t(lang, "about.title")}</h2>
      <p>{t(lang, "about.desc")}</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label={t(lang, "common.close")}>×</button>
  </header>
{/if}

<div class="settings-scroll">
  <section class="setting-card about-card">
    <div class="about-brand">
      <span class="about-logo"><AppIcon name="key" size={26} /></span>
      <div>
        <strong class="about-name">SecPivot</strong>
        <p class="about-version">v{appVersion}</p>
        <p class="about-tagline">{t(lang, "about.tagline")}</p>
      </div>
    </div>
    <dl class="about-grid">
      <div class="about-item">
        <dt>{t(lang, "about.stack")}</dt>
        <dd>Svelte 5 · Tauri 2 · Rust</dd>
      </div>
      <div class="about-item">
        <dt>{t(lang, "about.format")}</dt>
        <dd>{t(lang, "about.formatValue")}</dd>
      </div>
      <div class="about-item">
        <dt>{t(lang, "about.crypto")}</dt>
        <dd>AES-256 / ChaCha20 · Argon2id</dd>
      </div>
      <div class="about-item">
        <dt>{t(lang, "about.license")}</dt>
        <dd>MIT</dd>
      </div>
      {#if info}
        <div class="about-item">
          <dt>{t(lang, "about.mode")}</dt>
          <dd>{info.portable ? t(lang, "about.portable") : t(lang, "about.installed")}</dd>
        </div>
        <div class="about-item about-item-wide">
          <dt>{t(lang, "about.dataDir")}</dt>
          <dd class="mono" title={info.dataDir}>{info.dataDir}</dd>
        </div>
      {/if}
    </dl>
  </section>

  <p class="auto-save-note">{t(lang, "about.localOnly")}</p>
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

  .about-item-wide {
    grid-column: 1 / -1;
  }

  .about-item-wide dd {
    overflow: hidden;
    direction: rtl;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
    user-select: text;
  }
</style>
