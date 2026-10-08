<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  import type { GeneralSettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";
  import SettingToggleCard from "$lib/components/settings/SettingToggleCard.svelte";

  type Section = "general" | "network";

  interface Props {
    onclose: () => void;
    showHeader?: boolean;
    section: Section;
  }

  let { onclose, showHeader = true, section }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const general = $derived(s.general);
  const lang = $derived(s.general.language);

  function change<K extends keyof GeneralSettings>(key: K, value: GeneralSettings[K]): void {
    appSettings.updateGeneral(key, value);
  }
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · {t(lang, "settings.generalTitle")}</span>
      <h2>{t(lang, "settings.generalTitle")}</h2>
      <p>{t(lang, "settings.generalDesc")}</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label={t(lang, "common.close")}>×</button>
  </header>
{/if}

<div class="settings-scroll">
  {#if section === "general"}
    <section class="setting-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
        <div class="heading-inline">
          <div>
            <strong>{t(lang, "settings.language.title")}</strong>
            <p>{t(lang, "settings.language.description")}</p>
          </div>
        </div>
      </div>
      <div class="theme-segmented" role="group" aria-label={t(lang, "settings.language.title")}>
        <button
          class="theme-segment"
          class:active={general.language === "zh-CN"}
          onclick={() => change("language", "zh-CN")}
        >
          {t(lang, "settings.language.zh")}
        </button>
        <button
          class="theme-segment"
          class:active={general.language === "en"}
          onclick={() => change("language", "en")}
        >
          {t(lang, "settings.language.en")}
        </button>
      </div>
    </section>

    <SettingToggleCard
      icon="folder"
      label={t(lang, "settings.rememberLast")}
      description={t(lang, "settings.rememberLastDesc")}
      checked={general.rememberLastDatabase}
      onchange={(checked) => change("rememberLastDatabase", checked)}
    />
  {/if}

  {#if section === "network"}
    <SettingToggleCard
      icon="save"
      label={t(lang, "settings.faviconAutosave")}
      description={t(lang, "settings.faviconAutosaveDesc")}
      checked={s.favicon.autoSave}
      onchange={(checked) => appSettings.updateFavicon("autoSave", checked)}
    />
    <SettingRangeCard
      icon="globe"
      label={t(lang, "settings.faviconConcurrency")}
      description={t(lang, "settings.faviconConcurrencyDesc")}
      value={s.favicon.concurrency}
      valueLabel={t(lang, "settings.countItems", { count: s.favicon.concurrency })}
      min={1}
      max={16}
      onchange={(value) => appSettings.updateFavicon("concurrency", value)}
    />
  {/if}

  <p class="auto-save-note">{t(lang, "settings.autoSaveNote")}</p>
</div>
