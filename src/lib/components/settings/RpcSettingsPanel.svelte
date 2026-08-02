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
          >{rpc.enabled ? (status?.running ? `运行中 :${status.port}` : "启动中…") : "已停用"}</span
        >
      </div>
    </div>
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
    密钥保存在内存中，锁定或关闭数据库时自动清除。数据库未解锁时，扩展无法获取任何凭据。
  </p>

  <p class="auto-save-note">修改即时生效并自动保存</p>
</div>

<style>
  .status-off {
    color: var(--text-faint);
  }
</style>
