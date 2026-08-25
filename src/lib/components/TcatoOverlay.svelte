<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import AppIcon from "$lib/components/AppIcon.svelte";

  let title = $state("");
  let hasPassword = $state(false);
  let feedback = $state("");
  let error = $state("");

  onMount(async () => {
    try {
      const info = await invoke<{
        title: string;
        username: string;
        hasPassword: boolean;
      } | null>("tcato_state");
      if (!info) {
        error = "数据库未打开或条目不可用";
        return;
      }
      title = info.title;
      hasPassword = info.hasPassword;
    } catch (e) {
      error = `读取条目失败：${e}`;
    }
  });

  async function send(channel: "username" | "password"): Promise<void> {
    feedback = "";
    error = "";
    try {
      await invoke("tcato_send", { channel });
      feedback = channel === "username" ? "已注入用户名" : "已注入密码";
    } catch (e) {
      error = `${e}`;
    }
  }

  function close(): void {
    // Surface a failed close instead of leaving a dead overlay silently open.
    void invoke("close_tcato_overlay").catch((e) => {
      error = `${e}`;
    });
  }
</script>

<div class="tcato-shell">
  <header>
    <span class="title-icon"><AppIcon name="shield" size={15} /></span>
    <div class="heading">
      <strong>TCATO 两通道填充</strong>
      <p {title}>{title || "正在读取条目…"}</p>
    </div>
    <button class="close-button" onclick={close} aria-label="关闭">×</button>
  </header>

  <p class="hint">请先将焦点移到目标窗口，再点击要注入的内容；密码不经过键盘钩子。</p>

  <div class="actions">
    <button class="channel-button" onclick={() => send("username")}>
      <AppIcon name="user" size={13} />注入用户名
    </button>
    <button class="channel-button primary" onclick={() => send("password")} disabled={!hasPassword}>
      <AppIcon name="key" size={13} />注入密码
    </button>
  </div>

  {#if feedback}
    <p class="feedback ok">{feedback}</p>
  {/if}
  {#if error}
    <p class="feedback error">{error}</p>
  {/if}
</div>

<style>
  .tcato-shell {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100vh;
    padding: 12px;
    box-sizing: border-box;
    background: var(--bg-app);
    color: var(--text-primary);
    font-size: 12px;
    user-select: none;
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .title-icon {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--selection-color) 18%, transparent);
    color: var(--selection-color);
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  .heading strong {
    display: block;
    font-size: 13px;
  }

  .heading p {
    margin: 2px 0 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .close-button {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .close-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .hint {
    margin: 0;
    color: var(--text-secondary);
    font-size: 11px;
    line-height: 1.5;
  }

  .actions {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .channel-button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 30px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: 12px;
    cursor: pointer;
  }

  .channel-button:hover:not(:disabled) {
    border-color: var(--selection-color);
    background: var(--hover-bg);
  }

  .channel-button.primary {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, var(--input-bg));
  }

  .channel-button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .feedback {
    margin: 0;
    font-size: 11px;
  }

  .feedback.ok {
    color: var(--success-color);
  }

  .feedback.error {
    color: var(--danger-color);
  }
</style>
