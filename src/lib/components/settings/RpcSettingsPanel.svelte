<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import type { RpcSettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import SettingToggleCard from "$lib/components/settings/SettingToggleCard.svelte";

  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import { t } from "$lib/i18n";
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

  const rpc = $derived(s.rpc);
  const lang = $derived(s.general.language);

  interface RpcStatus {
    running: boolean;
    port: number;
    error: string | null;
  }

  interface RpcSessionInfo {
    id: number;
    username: string | null;
    peer: string;
    connectedAtMs: number;
    authenticated: boolean;
  }

  let status = $state<RpcStatus | null>(null);
  let sessions = $state<RpcSessionInfo[]>([]);

  function change<K extends keyof RpcSettings>(key: K, value: RpcSettings[K]): void {
    appSettings.updateRpc(key, value);
  }

  async function refreshStatus(): Promise<void> {
    if (!isTauriRuntime()) {
      status = null;
      sessions = [];
      return;
    }
    try {
      status = await invoke<RpcStatus>("rpc_status");
    } catch {
      status = null;
    }
    try {
      sessions = await invoke<RpcSessionInfo[]>("rpc_sessions");
    } catch {
      sessions = [];
    }
  }

  async function closeSession(id: number): Promise<void> {
    try {
      await invoke("rpc_close_session", { id });
    } catch {
      /* already gone */
    }
    void refreshStatus();
  }

  function formatConnectedAt(ms: number): string {
    return new Date(ms).toLocaleTimeString();
  }

  onMount(() => {
    void refreshStatus();
    const timer = setInterval(() => void refreshStatus(), 3000);
    return () => clearInterval(timer);
  });
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · {t(lang, "bridge.title")}</span>
      <h2>{t(lang, "rpc.title")}</h2>
      <p>{t(lang, "rpc.desc")}</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label={t(lang, "common.close")}>×</button>
  </header>
{/if}

<div class="settings-scroll">
  <SettingToggleCard
    icon="link"
    label={t(lang, "rpc.toggle")}
    description={t(lang, "rpc.toggleDesc")}
    checked={rpc.enabled}
    ariaLabel={t(lang, "rpc.toggleAria")}
    onchange={(enabled) => change("enabled", enabled)}
  />

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>{t(lang, "rpc.status")}</strong>
          <p>{t(lang, "rpc.statusDesc")}</p>
        </div>
        <span class="value-label" class:status-off={!rpc.enabled}
          >{rpc.enabled
            ? status?.running
              ? t(lang, "bridge.running", { port: status.port })
              : (status?.error ?? t(lang, "bridge.starting"))
            : t(lang, "bridge.stopped")}</span
        >
      </div>
    </div>
  </section>

  <SettingToggleCard
    icon="lock"
    label={t(lang, "rpc.keepKeys")}
    description={t(lang, "rpc.keepKeysDesc")}
    checked={rpc.keepSessionAfterLock}
    ariaLabel={t(lang, "rpc.keepKeysAria")}
    onchange={(checked) => change("keepSessionAfterLock", checked)}
  />

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="clock" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>{t(lang, "rpc.keyTimeout")}</strong>
          <p>{t(lang, "rpc.keyTimeoutDesc")}</p>
        </div>
        <div style="width: 120px; flex: 0 0 auto;">
          <TextField
            size="control"
            numeric
            type="number"
            value={String(rpc.sessionTimeoutSecs)}
            ariaLabel={t(lang, "rpc.keyTimeoutAria")}
            oninput={(e) =>
              change(
                "sessionTimeoutSecs",
                Math.max(0, Math.floor(Number(e.currentTarget.value) || 0)),
              )}
          />
        </div>
      </div>
    </div>
  </section>

  <SettingToggleCard
    icon="shield"
    label={t(lang, "rpc.matchDomain")}
    description={t(lang, "rpc.matchDomainDesc")}
    checked={rpc.matchByRegistrableDomain}
    ariaLabel={t(lang, "rpc.matchDomainAria")}
    onchange={(checked) => change("matchByRegistrableDomain", checked)}
  />

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>{t(lang, "rpc.sessions")}</strong>
          <p>{t(lang, "rpc.sessionsDesc")}</p>
        </div>
        <span class="value-label"
          >{sessions.length > 0
            ? t(lang, "rpc.connections", { count: sessions.length })
            : t(lang, "rpc.noConnections")}</span
        >
      </div>
    </div>
    {#if sessions.length > 0}
      <ul class="session-list">
        {#each sessions as session (session.id)}
          <li class="session-item">
            <span class="session-identity">
              <span class="session-name" class:unauth={!session.authenticated}>
                {session.authenticated
                  ? (session.username ?? t(lang, "rpc.authenticated"))
                  : t(lang, "rpc.handshaking")}
              </span>
              <span class="session-meta"
                >{session.peer} · {formatConnectedAt(session.connectedAtMs)}</span
              >
            </span>
            <Button variant="action" onclick={() => void closeSession(session.id)}
              >{t(lang, "rpc.disconnect")}</Button
            >
          </li>
        {/each}
      </ul>
    {:else}
      <p class="settings-note">{t(lang, "rpc.noSessions")}</p>
    {/if}
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="shield" size={17} /></span>
      <div>
        <strong>{t(lang, "rpc.howtoTitle")}</strong>
        <p>
          {t(lang, "rpc.howto")}
        </p>
      </div>
    </div>
  </section>

  <p class="settings-note">
    {t(lang, "rpc.sideNote")}
  </p>

  <p class="auto-save-note">{t(lang, "settings.autoSaveNote")}</p>
</div>

<style>
  .status-off {
    color: var(--text-faint);
  }

  /* Only the layout override — control chrome comes from `.settings-input`. */

  .session-list {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .session-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--card-bg);
  }

  .session-identity {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .session-name {
    color: var(--text-primary);
    font-size: var(--font-size-secondary, 11px);
  }

  .session-name.unauth {
    color: var(--text-faint);
  }

  .session-meta {
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  /* Layout override only — button chrome comes from `.settings-action-button`
   * (mirrors the bridge panel's client-remove row). */
</style>
