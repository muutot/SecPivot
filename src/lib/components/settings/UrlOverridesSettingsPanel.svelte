<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import type { IntegrationSettings, UrlSchemeOverride } from "$lib/types/settings";
  import { t } from "$lib/i18n";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
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

  const integration = $derived(s.integration);
  const lang = $derived(s.general.language);

  interface BuiltInOverrideInfo {
    flag: number;
    scheme: string;
    urlOverride: string;
  }

  let builtins = $state<BuiltInOverrideInfo[]>([]);

  onMount(() => {
    if (!isTauriRuntime()) return;
    void invoke<BuiltInOverrideInfo[]>("list_builtin_url_overrides")
      .then((rows) => (builtins = rows))
      .catch(() => (builtins = []));
  });

  function change<K extends keyof IntegrationSettings>(
    key: K,
    value: IntegrationSettings[K],
  ): void {
    appSettings.updateIntegration(key, value);
  }

  function toggleBuiltin(flag: number, enabled: boolean): void {
    const mask = integration.builtInOverridesEnabled;
    const next = enabled ? mask | flag : mask & ~flag;
    change("builtInOverridesEnabled", next >>> 0);
  }

  function updateCustom(index: number, patch: Partial<UrlSchemeOverride>): void {
    const next = integration.customOverrides.map((rule, i) =>
      i === index ? { ...rule, ...patch } : rule,
    );
    change("customOverrides", next);
  }

  function addCustom(): void {
    change("customOverrides", [
      ...integration.customOverrides,
      { enabled: true, scheme: "", urlOverride: "" },
    ]);
  }

  function removeCustom(index: number): void {
    change(
      "customOverrides",
      integration.customOverrides.filter((_, i) => i !== index),
    );
  }
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · {t(lang, "urlOverrides.title")}</span>
      <h2>{t(lang, "urlOverrides.title")}</h2>
      <p>{t(lang, "urlOverrides.desc")}</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label={t(lang, "common.close")}>×</button>
  </header>
{/if}

<div class="settings-scroll">
  <SettingToggleCard
    icon="link"
    label={t(lang, "urlOverrides.toggle")}
    description={t(lang, "urlOverrides.toggleDesc")}
    checked={integration.urlOverrideEnabled}
    ariaLabel={t(lang, "urlOverrides.toggle")}
    onchange={(enabled) => change("urlOverrideEnabled", enabled)}
  />

  <section class="setting-card">
    <div class="setting-heading override-heading">
      <span class="setting-icon"><AppIcon name="link" size={17} /></span>
      <div>
        <strong>{t(lang, "urlOverrides.global")}</strong>
        <p>{t(lang, "urlOverrides.globalDesc")}</p>
      </div>
    </div>
    <TextField
      value={integration.urlOverride}
      placeholder={t(lang, "urlOverrides.globalPlaceholder")}
      ariaLabel={t(lang, "urlOverrides.global")}
      mono
      size="control"
      oninput={(event) => change("urlOverride", event.currentTarget.value)}
    />
  </section>

  <section class="setting-card">
    <div class="setting-heading override-heading">
      <span class="setting-icon"><AppIcon name="link" size={17} /></span>
      <div>
        <strong>{t(lang, "urlOverrides.builtin")}</strong>
        <p>{t(lang, "urlOverrides.builtinDesc")}</p>
      </div>
    </div>
    <div class="override-table" role="table" aria-label={t(lang, "urlOverrides.builtin")}>
      <div class="override-row override-row--head" role="row">
        <span role="columnheader">{t(lang, "urlOverrides.enabled")}</span>
        <span role="columnheader">{t(lang, "urlOverrides.scheme")}</span>
        <span role="columnheader">{t(lang, "urlOverrides.template")}</span>
      </div>
      {#each builtins as rule (rule.flag)}
        <div class="override-row" role="row">
          <input
            type="checkbox"
            role="cell"
            aria-label={`${rule.scheme} ${t(lang, "urlOverrides.enabled")}`}
            checked={(integration.builtInOverridesEnabled & rule.flag) !== 0}
            onchange={(event) => toggleBuiltin(rule.flag, event.currentTarget.checked)}
          />
          <span class="scheme" role="cell">{rule.scheme}</span>
          <span class="template" role="cell" title={rule.urlOverride}>{rule.urlOverride}</span>
        </div>
      {/each}
    </div>
  </section>

  <section class="setting-card">
    <div class="setting-heading override-heading">
      <span class="setting-icon"><AppIcon name="plus" size={17} /></span>
      <div>
        <strong>{t(lang, "urlOverrides.custom")}</strong>
        <p>{t(lang, "urlOverrides.customDesc")}</p>
      </div>
      <Button variant="action" onclick={addCustom}>{t(lang, "urlOverrides.add")}</Button>
    </div>

    {#if integration.customOverrides.length === 0}
      <p class="empty-note">{t(lang, "urlOverrides.noCustom")}</p>
    {:else}
      <div class="custom-list">
        {#each integration.customOverrides as rule, i (i)}
          <div class="custom-row">
            <input
              type="checkbox"
              aria-label={t(lang, "urlOverrides.enabled")}
              checked={rule.enabled}
              onchange={(event) => updateCustom(i, { enabled: event.currentTarget.checked })}
            />
            <div class="custom-scheme">
              <TextField
                value={rule.scheme}
                placeholder={t(lang, "urlOverrides.scheme")}
                ariaLabel={t(lang, "urlOverrides.scheme")}
                size="control"
                oninput={(event) => updateCustom(i, { scheme: event.currentTarget.value })}
              />
            </div>
            <div class="custom-template">
              <TextField
                value={rule.urlOverride}
                placeholder={t(lang, "urlOverrides.template")}
                ariaLabel={t(lang, "urlOverrides.template")}
                mono
                size="control"
                oninput={(event) => updateCustom(i, { urlOverride: event.currentTarget.value })}
              />
            </div>
            <Button
              variant="action"
              title={t(lang, "urlOverrides.remove")}
              ariaLabel={t(lang, "urlOverrides.remove")}
              onclick={() => removeCustom(i)}
            >
              <AppIcon name="trash" size={14} />
            </Button>
          </div>
        {/each}
      </div>
    {/if}
    <p class="help-note">{t(lang, "urlOverrides.help")}</p>
  </section>
</div>

<style>
  .override-heading {
    margin-bottom: 10px;
  }
  .override-heading > div {
    flex: 1;
    min-width: 0;
  }
  .override-heading p {
    line-height: 1.4;
  }
  .override-table {
    display: flex;
    flex-direction: column;
    gap: 1px;
    max-height: 320px;
    overflow-y: auto;
  }
  .override-row {
    display: grid;
    grid-template-columns: 54px 52px minmax(0, 1fr);
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border-radius: 4px;
    font-size: 11px;
  }
  .override-row:hover {
    background: var(--hover-bg);
  }
  .override-row--head {
    color: var(--text-muted);
    font-weight: 600;
  }
  .override-row .scheme {
    color: var(--text-secondary);
    font-family: var(--font-mono, monospace);
  }
  .override-row .template {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-muted);
    font-family: var(--font-mono, monospace);
  }
  .custom-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .custom-row {
    display: grid;
    grid-template-columns: 20px 90px minmax(0, 1fr) 26px;
    align-items: center;
    gap: 6px;
  }
  .custom-scheme,
  .custom-template {
    min-width: 0;
  }
  @media (max-width: 480px) {
    .custom-row {
      grid-template-columns: 20px minmax(0, 1fr) 26px;
    }
    .custom-template {
      grid-column: 2;
      grid-row: 2;
    }
    .custom-row > :last-child {
      grid-column: 3;
      grid-row: 1 / span 2;
    }
  }
  .empty-note,
  .help-note {
    margin: 8px 0 0;
    font-size: 11px;
    line-height: 1.5;
    color: var(--text-muted);
  }
</style>
