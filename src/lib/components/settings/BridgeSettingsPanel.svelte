<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import type { BridgeSettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import SettingToggleCard from "$lib/components/settings/SettingToggleCard.svelte";
  import { t } from "$lib/i18n";

  import Button from "$lib/components/templates/action/Button.svelte";
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

  const bridge = $derived(s.bridge);
  const lang = $derived(s.general.language);

  interface BridgeStatus {
    running: boolean;
    port: number;
    error: string | null;
  }

  let status = $state<BridgeStatus | null>(null);
  let clients = $state<string[]>([]);
  let clientNote = $state("");

  function change<K extends keyof BridgeSettings>(key: K, value: BridgeSettings[K]): void {
    appSettings.updateBridge(key, value);
  }

  async function refreshStatus(): Promise<void> {
    if (!isTauriRuntime()) {
      status = null;
      return;
    }
    try {
      status = await invoke<BridgeStatus>("bridge_status");
    } catch {
      status = null;
    }
  }

  async function refreshClients(): Promise<void> {
    if (!isTauriRuntime()) return;
    try {
      clients = await invoke<string[]>("bridge_clients");
      clientNote = "";
    } catch {
      clients = [];
      clientNote = t(lang, "bridge.clientNoteLocked");
    }
  }

  async function removeClient(id: string): Promise<void> {
    try {
      clients = await invoke<string[]>("bridge_remove_client", { id });
    } catch (e) {
      clientNote = String(e);
    }
  }

  onMount(() => {
    void refreshStatus();
    if (bridge.enabled) void refreshClients();
    const timer = setInterval(() => void refreshStatus(), 3000);
    return () => clearInterval(timer);
  });
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · {t(lang, "bridge.title")}</span>
      <h2>{t(lang, "bridge.title")}</h2>
      <p>{t(lang, "bridge.desc")}</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label={t(lang, "common.close")}>×</button>
  </header>
{/if}

<div class="settings-scroll">
  <SettingToggleCard
    icon="plug"
    label={t(lang, "bridge.toggle")}
    description={t(lang, "bridge.toggleDesc")}
    checked={bridge.enabled}
    ariaLabel={t(lang, "bridge.toggleAria")}
    onchange={(enabled) => {
      change("enabled", enabled);
      if (enabled) void refreshClients();
    }}
  />

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>{t(lang, "bridge.status")}</strong>
          <p>{t(lang, "bridge.statusDesc")}</p>
        </div>
        <span class="value-label" class:status-off={!bridge.enabled}
          >{bridge.enabled
            ? status?.running
              ? t(lang, "bridge.running", { port: status.port })
              : (status?.error ?? t(lang, "bridge.starting"))
            : t(lang, "bridge.stopped")}</span
        >
      </div>
    </div>
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="shield" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>{t(lang, "bridge.clients")}</strong>
          <p>{t(lang, "bridge.clientsDesc")}</p>
        </div>
        <Button
          variant="action"
          onclick={() => void refreshClients()}
          disabled={!bridge.enabled || !isTauriRuntime()}
        >
          {t(lang, "bridge.refresh")}
        </Button>
      </div>
    </div>
    {#if clientNote}
      <p class="settings-note">{clientNote}</p>
    {/if}
    {#if clients.length === 0 && !clientNote}
      <p class="settings-note">{t(lang, "bridge.noClients")}</p>
    {:else if clients.length > 0}
      <ul class="client-list">
        {#each clients as id (id)}
          <li class="client-row">
            <span class="client-icon"><AppIcon name="key" size={12} /></span>
            <span class="client-id" title={id}>{id}</span>
            <Button
              variant="action"
              onclick={() => void removeClient(id)}
              ariaLabel={t(lang, "bridge.removeClient", { id })}
            >
              {t(lang, "bridge.remove")}
            </Button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <p class="settings-note">
    {t(lang, "bridge.keyNote")}
  </p>

  <p class="auto-save-note">{t(lang, "settings.autoSaveNote")}</p>
</div>

<style>
  .status-off {
    color: var(--text-faint);
  }

  .client-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
  }

  .client-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--card-bg);
  }

  .client-icon {
    display: inline-flex;
    flex: 0 0 auto;
    color: var(--text-faint);
  }

  .client-id {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--font-mono);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
  }
</style>
