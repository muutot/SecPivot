<script lang="ts">
  import { get } from "svelte/store";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { vault } from "$lib/services/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    onopened: () => void;
  }

  let { onopened }: Props = $props();

  const recentFiles = $derived(get(appSettings).general.recentFiles);

  type Modal = "none" | "open" | "create";

  let modal: Modal = $state("none");
  let busy = $state(false);
  let error = $state("");
  let path = $state("");
  let password = $state("");
  let confirm = $state("");
  let showPassword = $state(false);
  let isDemo = $state(false);

  async function handleOpen(): Promise<void> {
    if (isTauriRuntime()) {
      const selected = await open({
        multiple: false,
        filters: [{ name: "KeePass 数据库", extensions: ["kdbx"] }],
      });
      if (!selected) return;
      path = String(selected);
      isDemo = false;
    } else {
      path = "demo://vault.kdbx";
      isDemo = true;
      password = "";
    }
    error = "";
    modal = "open";
  }

  async function confirmOpen(): Promise<void> {
    if (!isDemo && !path) {
      error = "请选择数据库文件";
      return;
    }
    if (!password) {
      error = "请输入主密码";
      return;
    }
    busy = true;
    error = "";
    try {
      await vault.open(path, password);
      modal = "none";
      onopened();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function openRecent(file: string): void {
    path = file;
    isDemo = false;
    error = "";
    modal = "open";
  }

  function handleCreate(): void {
    path = "";
    password = "";
    confirm = "";
    error = "";
    modal = "create";
  }

  async function pickCreatePath(): Promise<void> {
    const selected = await save({
      defaultPath: "new-vault.kdbx",
      filters: [{ name: "KeePass 数据库", extensions: ["kdbx"] }],
    });
    if (selected) path = String(selected);
  }

  async function confirmCreate(): Promise<void> {
    if (!password) {
      error = "请输入主密码";
      return;
    }
    if (password !== confirm) {
      error = "两次输入的密码不一致";
      return;
    }
    busy = true;
    error = "";
    try {
      let target = path;
      if (!target) {
        if (isTauriRuntime()) {
          const selected = await save({
            defaultPath: "new-vault.kdbx",
            filters: [{ name: "KeePass 数据库", extensions: ["kdbx"] }],
          });
          if (!selected) return;
          target = String(selected);
        } else {
          target = "demo://new-vault.kdbx";
        }
      }
      const settings = get(appSettings);
      await vault.create({
        path: target,
        password,
        kdf: settings.database.kdf,
        cipher: settings.database.cipher,
        compression: settings.database.compression,
      });
      modal = "none";
      onopened();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="welcome">
  <div class="welcome-inner">
    <div class="welcome-logo"><AppIcon name="key" size={34} /></div>
    <h1 class="welcome-title">KeyVault</h1>
    <p class="welcome-subtitle">本地优先的 KeePass 密码管理器</p>

    <div class="welcome-actions">
      <button class="welcome-button primary" onclick={handleOpen} disabled={busy}>
        <AppIcon name="open" size={16} />打开数据库
      </button>
      <button class="welcome-button" onclick={handleCreate} disabled={busy}>
        <AppIcon name="plus" size={16} />新建数据库
      </button>
    </div>

    <p class="welcome-hint">主密码只在你本地使用，绝不存储或上传</p>

    {#if recentFiles.length > 0}
      <div class="recent-section">
        <p class="recent-label">最近打开</p>
        {#each recentFiles as file (file)}
          <button class="recent-item" onclick={() => openRecent(file)} title={file}>
            <AppIcon name="clock" size={12} />
            <span class="recent-name">{file.split(/[\\/]/).pop() || file}</span>
          </button>
        {/each}
      </div>
    {/if}
  </div>
</div>

{#if modal !== "none"}
  <div class="modal-backdrop" role="presentation">
    <div
      class="password-modal"
      role="dialog"
      aria-modal="true"
      aria-label={modal === "open" ? "解锁数据库" : "新建数据库"}
    >
      <div class="modal-head">
        <span class="modal-icon"
          ><AppIcon name={modal === "open" ? "lock" : "folder-plus"} size={18} /></span
        >
        <div>
          <strong
            >{modal === "open" ? (isDemo ? "打开演示数据库" : "解锁数据库") : "新建数据库"}</strong
          >
          <p>{modal === "open" ? path : "选择一个位置并设置主密码"}</p>
        </div>
      </div>

      {#if modal === "create"}
        <label class="field">
          <span>保存路径</span>
          <div class="path-row">
            <input
              class="text-input"
              type="text"
              bind:value={path}
              placeholder={isTauriRuntime() ? "点击右侧选择文件" : "默认保存到浏览器演示存储"}
              disabled={!isTauriRuntime()}
            />
            {#if isTauriRuntime()}
              <button class="browse-button" onclick={pickCreatePath} title="选择保存位置">
                <AppIcon name="folder" size={15} />
              </button>
            {/if}
          </div>
        </label>
      {/if}

      <label class="field">
        <span>主密码</span>
        <div class="path-row">
          <input
            class="text-input"
            type={showPassword ? "text" : "password"}
            bind:value={password}
            placeholder={isDemo ? "演示模式可留空" : "必填"}
          />
          <button
            class="browse-button"
            onclick={() => (showPassword = !showPassword)}
            title="显示密码"
          >
            <AppIcon name={showPassword ? "eye-off" : "eye"} size={15} />
          </button>
        </div>
      </label>

      {#if modal === "create"}
        <label class="field">
          <span>确认主密码</span>
          <div class="path-row">
            <input class="text-input" type="password" bind:value={confirm} />
          </div>
        </label>
      {/if}

      {#if error}
        <p class="modal-error">{error}</p>
      {/if}

      <div class="modal-actions">
        <button class="modal-button" onclick={() => (modal = "none")} disabled={busy}>取消</button>
        <button
          class="modal-button primary"
          onclick={modal === "open" ? confirmOpen : confirmCreate}
          disabled={busy}
        >
          {busy ? "处理中…" : modal === "open" ? "解锁" : "创建"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .welcome {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
  }

  .welcome-inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    max-width: 380px;
    padding: 24px;
    text-align: center;
  }

  .welcome-logo {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 64px;
    height: 64px;
    border: 1px solid var(--border-color);
    border-radius: 16px;
    color: var(--warning-color);
    background: var(--card-bg);
  }

  .welcome-title {
    margin: 16px 0 0;
    font-size: 24px;
    font-weight: 590;
    letter-spacing: 0.01em;
  }

  .welcome-subtitle {
    margin: 6px 0 0;
    color: var(--text-muted);
    font-size: 12px;
  }

  .welcome-actions {
    display: flex;
    gap: 10px;
    margin-top: 28px;
  }

  .welcome-button {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 16px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--card-bg);
    font-size: 12px;
    cursor: pointer;
  }

  .welcome-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .welcome-button.primary {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
  }

  .welcome-button.primary:hover {
    background: color-mix(in srgb, var(--selection-color) 26%, var(--card-bg));
  }

  .welcome-button:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  .welcome-hint {
    margin: 18px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .recent-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
    margin-top: 20px;
  }

  .recent-label {
    margin: 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-align: left;
  }

  .recent-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--card-bg);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .recent-item:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .recent-item:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  .recent-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, #000 45%, transparent);
  }

  .password-modal {
    width: min(380px, calc(100% - 40px));
    padding: 18px;
    border: 1px solid var(--border-color);
    border-radius: 13px;
    background: var(--surface-bg);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
  }

  .modal-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }

  .modal-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-icon-radius, 7px);
    color: var(--selection-color);
    background: var(--hover-bg);
  }

  .modal-head strong {
    display: block;
    font-size: 13px;
    font-weight: 560;
  }

  .modal-head p {
    margin: 2px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    word-break: break-all;
  }

  .field {
    display: block;
    margin-top: 10px;
  }

  .field > span {
    display: block;
    margin-bottom: 5px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .path-row {
    display: flex;
    gap: 6px;
  }

  .text-input {
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: 12px;
  }

  .text-input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .browse-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    cursor: pointer;
  }

  .browse-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .modal-error {
    margin: 10px 0 0;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }

  .modal-button {
    height: 30px;
    padding: 0 14px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--card-bg);
    font-size: 12px;
    cursor: pointer;
  }

  .modal-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .modal-button.primary {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
  }

  .modal-button:disabled {
    cursor: wait;
    opacity: 0.6;
  }
</style>
