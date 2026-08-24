<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import type { RpcSettings } from "$lib/types/settings";
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

  const rpc = $derived(s.rpc);

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
      <span class="eyebrow">Settings · 集成</span>
      <h2>KeePassRPC</h2>
      <p>KeePassRPC（Kee 4.x 扩展）兼容桥接设置。</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label="关闭">×</button>
  </header>
{/if}

<div class="settings-scroll">
  <SettingToggleCard
    icon="link"
    label="KeePassRPC 桥接"
    description="在 127.0.0.1:12546 启用 KeePassRPC 兼容服务（仅本机回环）"
    checked={rpc.enabled}
    ariaLabel="启用 KeePassRPC 桥接"
    onchange={(enabled) => change("enabled", enabled)}
  />

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>服务状态</strong>
          <p>监听地址 127.0.0.1:12546，Kee 浏览器扩展直接连接本机服务</p>
        </div>
        <span class="value-label" class:status-off={!rpc.enabled}
          >{rpc.enabled
            ? status?.running
              ? `运行中 :${status.port}`
              : (status?.error ?? "启动中…")
            : "已停用"}</span
        >
      </div>
    </div>
  </section>

  <SettingToggleCard
    icon="lock"
    label="锁库后保留会话密钥"
    description="锁定数据库时保留 SRP 会话密钥，解锁后 Kee 扩展无需重新输入旁路密码（与官方 KeePassRPC 一致）。锁库期间扩展仍无法获取任何凭据"
    checked={rpc.keepSessionAfterLock}
    ariaLabel="锁库后保留 RPC 会话密钥"
    onchange={(checked) => change("keepSessionAfterLock", checked)}
  />

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="clock" size={17} /></span>
      <div>
        <strong>会话密钥超时</strong>
        <p>SRP 会话密钥的最长保留时间（秒），每次解锁数据库都会重新计时；0 表示永不过期</p>
      </div>
      <input
        class="settings-input timeout-input"
        type="number"
        min="0"
        max="2592000"
        value={rpc.sessionTimeoutSecs}
        aria-label="会话密钥超时秒数"
        oninput={(e) =>
          change("sessionTimeoutSecs", Math.max(0, Math.floor(Number(e.currentTarget.value) || 0)))}
      />
    </div>
  </section>

  <SettingToggleCard
    icon="shield"
    label="按注册域匹配（KeePassRPC 兼容）"
    description="开启后「域名」匹配按注册域判定（公共后缀表），同一域名下的兄弟子域都会命中——例如 account.aliyun.com 与 passport.aliyun.com 同属 aliyun.com 均可匹配。关闭则仅按 host 或子域严格匹配"
    checked={rpc.matchByRegistrableDomain}
    ariaLabel="按注册域匹配"
    onchange={(checked) => change("matchByRegistrableDomain", checked)}
  />

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>已连接会话</strong>
          <p>当前与 Kee 扩展保持连接的浏览器会话，可手动断开任意一个</p>
        </div>
        <span class="value-label"
          >{sessions.length > 0 ? `${sessions.length} 个连接` : "无连接"}</span
        >
      </div>
    </div>
    {#if sessions.length > 0}
      <ul class="session-list">
        {#each sessions as session (session.id)}
          <li class="session-item">
            <span class="session-identity">
              <span class="session-name" class:unauth={!session.authenticated}>
                {session.authenticated ? (session.username ?? "已认证客户端") : "握手中…"}
              </span>
              <span class="session-meta"
                >{session.peer} · {formatConnectedAt(session.connectedAtMs)}</span
              >
            </span>
            <button
              class="settings-action-button session-close"
              onclick={() => void closeSession(session.id)}>断开</button
            >
          </li>
        {/each}
      </ul>
    {:else}
      <p class="settings-note">暂无已连接会话；在 Kee 扩展中发起连接后显示在这里</p>
    {/if}
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="shield" size={17} /></span>
      <div>
        <strong>连接方式</strong>
        <p>
          Kee 扩展连接时，若数据库已解锁，App 会弹出一次性旁路密码；在 Kee 的对话框中输入该密码完成
          SRP 握手认证
        </p>
      </div>
    </div>
  </section>

  <p class="settings-note">
    旁路密码约 2 分钟有效且仅显示一次；SRP
    密钥保存在内存中。关闭「锁库后保留会话密钥」后，锁定或关闭数据库时会清除 SRP
    密钥，扩展需重新授权；否则密钥在锁库期间仍保留，供解锁后直接复用。数据库未解锁时，扩展无法获取任何凭据。
  </p>

  <p class="auto-save-note">修改即时生效并自动保存</p>
</div>

<style>
  .status-off {
    color: var(--text-faint);
  }

  /* Only the layout override — control chrome comes from `.settings-input`. */
  .timeout-input {
    width: 90px;
    flex: 0 0 auto;
  }

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
  .session-close {
    height: 22px;
    padding: 0 8px;
    background: transparent;
  }
</style>
