<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import type { RpcSettings } from "$lib/types/settings";
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

  const rpc = $derived(s.rpc);

  interface RpcStatus {
    running: boolean;
    port: number;
    error: string | null;
  }

  let status = $state<RpcStatus | null>(null);

  function change<K extends keyof RpcSettings>(key: K, value: RpcSettings[K]): void {
    appSettings.updateRpc(key, value);
  }

  async function refreshStatus(): Promise<void> {
    if (!isTauriRuntime()) {
      status = null;
      return;
    }
    try {
      status = await invoke<RpcStatus>("rpc_status");
    } catch {
      status = null;
    }
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
  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="link" size={17} /></span>
      <div>
        <strong>KeePassRPC 桥接</strong>
        <p>在 127.0.0.1:12546 启用 KeePassRPC 兼容服务（仅本机回环）</p>
      </div>
    </div>
    <button
      class="toggle-switch"
      class:active={rpc.enabled}
      role="switch"
      aria-checked={rpc.enabled}
      aria-label="启用 KeePassRPC 桥接"
      onclick={() => {
        change("enabled", !rpc.enabled);
      }}
    >
      <span class="toggle-knob"></span>
    </button>
  </section>

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

  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="lock" size={17} /></span>
      <div>
        <strong>锁库后保留会话密钥</strong>
        <p>
          锁定数据库时保留 SRP 会话密钥，解锁后 Kee 扩展无需重新输入旁路密码（与官方 KeePassRPC
          一致）。锁库期间扩展仍无法获取任何凭据
        </p>
      </div>
    </div>
    <button
      class="toggle-switch"
      class:active={rpc.keepSessionAfterLock}
      role="switch"
      aria-checked={rpc.keepSessionAfterLock}
      aria-label="锁库后保留 RPC 会话密钥"
      onclick={() => {
        change("keepSessionAfterLock", !rpc.keepSessionAfterLock);
      }}
    >
      <span class="toggle-knob"></span>
    </button>
  </section>

  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="shield" size={17} /></span>
      <div>
        <strong>按注册域匹配（KeePassRPC 兼容）</strong>
        <p>
          开启后「域名」匹配按注册域判定（公共后缀表），同一域名下的兄弟子域都会命中——例如
          account.aliyun.com 与 passport.aliyun.com 同属 aliyun.com 均可匹配。关闭则仅按 host
          或子域严格匹配
        </p>
      </div>
    </div>
    <button
      class="toggle-switch"
      class:active={rpc.matchByRegistrableDomain}
      role="switch"
      aria-checked={rpc.matchByRegistrableDomain}
      aria-label="按注册域匹配"
      onclick={() => {
        change("matchByRegistrableDomain", !rpc.matchByRegistrableDomain);
      }}
    >
      <span class="toggle-knob"></span>
    </button>
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
</style>
