<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import type { BridgeSettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
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

  const bridge = $derived(s.bridge);

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
      clientNote = "解锁数据库后显示已授权的浏览器客户端";
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
      <span class="eyebrow">Settings · 集成</span>
      <h2>集成</h2>
      <p>浏览器桥接（KeePassHttp 兼容）设置。</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label="关闭">×</button>
  </header>
{/if}

<div class="settings-scroll">
  <SettingToggleCard
    icon="plug"
    label="浏览器桥接"
    description="在 127.0.0.1:19455 启用 KeePassHttp 兼容服务（仅本机回环）"
    checked={bridge.enabled}
    ariaLabel="启用浏览器桥接"
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
          <strong>服务状态</strong>
          <p>监听地址 127.0.0.1:19455，浏览器扩展直接连接本机服务</p>
        </div>
        <span class="value-label" class:status-off={!bridge.enabled}
          >{bridge.enabled
            ? status?.running
              ? `运行中 :${status.port}`
              : (status?.error ?? "启动中…")
            : "已停用"}</span
        >
      </div>
    </div>
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="shield" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>已授权客户端</strong>
          <p>浏览器首次连接需经你批准，密钥仅存于本次会话</p>
        </div>
        <button
          class="settings-action-button"
          type="button"
          onclick={() => void refreshClients()}
          disabled={!bridge.enabled || !isTauriRuntime()}
        >
          刷新
        </button>
      </div>
    </div>
    {#if clientNote}
      <p class="settings-note">{clientNote}</p>
    {/if}
    {#if clients.length === 0 && !clientNote}
      <p class="settings-note">暂无已授权客户端</p>
    {:else if clients.length > 0}
      <ul class="client-list">
        {#each clients as id (id)}
          <li class="client-row">
            <span class="client-icon"><AppIcon name="key" size={12} /></span>
            <span class="client-id" title={id}>{id}</span>
            <button
              class="settings-action-button client-remove"
              type="button"
              onclick={() => void removeClient(id)}
              aria-label={`移除客户端 ${id}`}
            >
              移除
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <p class="settings-note">
    授权密钥保存在内存中，锁定或关闭数据库时自动清除；服务不监听外部地址，其他设备无法访问。
  </p>

  <p class="auto-save-note">修改即时生效并自动保存</p>
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
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
  }

  .client-remove {
    height: 22px;
    padding: 0 8px;
    background: transparent;
  }
</style>
