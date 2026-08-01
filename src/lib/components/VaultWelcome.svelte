<script lang="ts">
  import { get } from "svelte/store";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { rememberCredential } from "$lib/services/security";
  import { vault } from "$lib/services/vault";
  import type { RemoteMode, RemoteObject } from "$lib/types/vault";
  import type { RemoteSettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    onopened: () => void;
  }

  let { onopened }: Props = $props();

  const recentFiles = $derived(get(appSettings).general.recentFiles);

  const remoteLocalDir = $derived(get(appSettings).remote.localDir || "remote");

  type Modal = "none" | "open" | "create" | "remote";
  type RemoteTab = "open" | "create";

  let modal: Modal = $state("none");
  let busy = $state(false);
  let error = $state("");
  let path = $state("");
  let password = $state("");
  let confirm = $state("");
  let keyfilePath = $state("");
  let showPassword = $state(false);
  let isDemo = $state(false);

  let remoteTab: RemoteTab = $state("open");
  let remoteObjects: RemoteObject[] = $state([]);
  let remoteKey = $state("");
  let remoteMode: RemoteMode = $state("memory");
  let remoteLoading = $state(false);
  let remoteConfigOpen = $state(false);

  const remote = $derived(get(appSettings).remote);
  const remoteConfigured = $derived(
    Boolean(remote.endpoint && remote.bucket && remote.accessKey && remote.secretKey),
  );

  function changeRemote<K extends keyof RemoteSettings>(key: K, value: RemoteSettings[K]): void {
    appSettings.updateRemote(key, value);
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  async function handleRemoteOpen(): Promise<void> {
    remoteTab = "open";
    remoteKey = "";
    remoteMode = "memory";
    keyfilePath = "";
    password = "";
    error = "";
    remoteConfigOpen = !remoteConfigured;
    modal = "remote";
    await loadRemoteObjects();
  }

  async function loadRemoteObjects(): Promise<void> {
    if (!isTauriRuntime()) return;
    remoteLoading = true;
    error = "";
    try {
      remoteObjects = await vault.listRemoteObjects();
      if (remoteObjects.length === 0) {
        error = "未找到远程数据库文件，请检查设置中的存储桶与对象前缀";
      }
    } catch (e) {
      error = String(e);
    } finally {
      remoteLoading = false;
    }
  }

  function switchRemoteTab(tab: RemoteTab): void {
    remoteTab = tab;
    remoteKey = "";
    password = "";
    confirm = "";
    error = "";
  }

  async function confirmRemoteOpen(): Promise<void> {
    if (!remoteKey) {
      error = "请选择远程数据库文件";
      return;
    }
    if (!password) {
      error = "请输入主密码";
      return;
    }
    busy = true;
    error = "";
    try {
      await vault.openRemote(remoteKey, password, keyfilePath || undefined, remoteMode);
      modal = "none";
      onopened();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function confirmRemoteCreate(): Promise<void> {
    if (!remoteKey) {
      error = "请输入远程对象键";
      return;
    }
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
      const settings = get(appSettings);
      await vault.createRemote(
        remoteKey,
        password,
        settings.database.kdf,
        settings.database.cipher,
        settings.database.compression,
        keyfilePath || undefined,
        remoteMode,
      );
      modal = "none";
      onopened();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

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
    keyfilePath = "";
    error = "";
    modal = "open";
  }

  async function pickKeyfile(): Promise<void> {
    const selected = await open({
      multiple: false,
      filters: [{ name: "密钥文件", extensions: ["key", "keyx", "xml", "txt"] }],
    });
    if (selected) keyfilePath = String(selected);
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
      await vault.open(path, password, keyfilePath || undefined);
      void rememberCredential(path, password);
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
    keyfilePath = "";
    error = "";
    modal = "open";
  }

  function handleCreate(): void {
    path = "";
    password = "";
    confirm = "";
    keyfilePath = "";
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
        keyfile: keyfilePath || undefined,
      });
      void rememberCredential(target, password);
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
      {#if isTauriRuntime()}
        <button class="welcome-button" onclick={handleRemoteOpen} disabled={busy}>
          <AppIcon name="cloud" size={16} />远程数据库
        </button>
      {/if}
    </div>

    <p class="welcome-hint">主密码只在你本地使用；远程库仅上传加密后的数据库</p>

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
      aria-label={modal === "open"
        ? "解锁数据库"
        : modal === "create"
          ? "新建数据库"
          : "远程数据库"}
    >
      <div class="modal-head">
        <span class="modal-icon"
          ><AppIcon
            name={modal === "open" ? "lock" : modal === "create" ? "folder-plus" : "cloud"}
            size={18}
          /></span
        >
        <div>
          <strong
            >{modal === "open"
              ? isDemo
                ? "打开演示数据库"
                : "解锁数据库"
              : modal === "create"
                ? "新建数据库"
                : "远程数据库 (S3)"}</strong
          >
          <p>
            {modal === "open"
              ? path
              : modal === "create"
                ? "选择一个位置并设置主密码"
                : "从 S3 打开或创建数据库"}
          </p>
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

      {#if modal === "remote"}
        <div class="remote-config">
          <button
            class="remote-config-toggle"
            type="button"
            aria-expanded={remoteConfigOpen}
            onclick={() => (remoteConfigOpen = !remoteConfigOpen)}
          >
            <AppIcon name="cloud" size={13} />
            <span>S3 连接配置</span>
            <span class="remote-config-status">{remoteConfigured ? "已配置" : "未配置"}</span>
            <AppIcon name={remoteConfigOpen ? "chevron-down" : "chevron-right"} size={12} />
          </button>
          {#if remoteConfigOpen}
            <div class="remote-config-body">
              <div class="field">
                <span>服务地址</span>
                <input
                  class="text-input"
                  type="text"
                  value={remote.endpoint}
                  placeholder="https://s3.amazonaws.com"
                  spellcheck="false"
                  oninput={(e) => changeRemote("endpoint", e.currentTarget.value)}
                />
              </div>
              <div class="remote-config-grid">
                <div class="field">
                  <span>区域</span>
                  <input
                    class="text-input"
                    type="text"
                    value={remote.region}
                    placeholder="us-east-1"
                    spellcheck="false"
                    oninput={(e) => changeRemote("region", e.currentTarget.value)}
                  />
                </div>
                <div class="field">
                  <span>存储桶</span>
                  <input
                    class="text-input"
                    type="text"
                    value={remote.bucket}
                    placeholder="my-bucket"
                    spellcheck="false"
                    oninput={(e) => changeRemote("bucket", e.currentTarget.value)}
                  />
                </div>
              </div>
              <div class="field">
                <span>Access Key</span>
                <input
                  class="text-input"
                  type="text"
                  value={remote.accessKey}
                  placeholder="AKIA..."
                  autocomplete="off"
                  spellcheck="false"
                  oninput={(e) => changeRemote("accessKey", e.currentTarget.value)}
                />
              </div>
              <div class="field">
                <span>Secret Key</span>
                <input
                  class="text-input"
                  type="password"
                  value={remote.secretKey}
                  placeholder="••••••••"
                  autocomplete="off"
                  spellcheck="false"
                  oninput={(e) => changeRemote("secretKey", e.currentTarget.value)}
                />
              </div>
              <p class="remote-config-note">
                凭据明文保存在 config.json，仅用于访问远程存储；修改后点「刷新列表」生效。
              </p>
            </div>
          {/if}
        </div>

        <div class="remote-tabs" role="tablist" aria-label="远程操作">
          <button
            class="remote-tab"
            class:active={remoteTab === "open"}
            onclick={() => switchRemoteTab("open")}
          >
            打开
          </button>
          <button
            class="remote-tab"
            class:active={remoteTab === "create"}
            onclick={() => switchRemoteTab("create")}
          >
            新建
          </button>
        </div>

        {#if remoteTab === "open"}
          <div class="field">
            <span>选择远程文件</span>
            <div class="remote-list">
              {#if remoteLoading}
                <p class="remote-empty">正在加载…</p>
              {:else if remoteObjects.length === 0}
                <p class="remote-empty">暂无文件</p>
              {:else}
                {#each remoteObjects as obj (obj.key)}
                  <button
                    class="remote-item"
                    class:active={remoteKey === obj.key}
                    onclick={() => (remoteKey = obj.key)}
                  >
                    <AppIcon name="file" size={13} />
                    <span class="remote-item-name" title={obj.key}>{obj.key}</span>
                    <span class="remote-item-size">{formatBytes(obj.size)}</span>
                  </button>
                {/each}
              {/if}
            </div>
            <button
              class="remote-refresh"
              onclick={loadRemoteObjects}
              disabled={remoteLoading || busy}
            >
              <AppIcon name="refresh" size={13} />刷新列表
            </button>
          </div>
        {:else}
          <label class="field">
            <span>远程对象键</span>
            <input
              class="text-input"
              type="text"
              bind:value={remoteKey}
              placeholder="vaults/new.kdbx"
              spellcheck="false"
            />
          </label>
        {/if}

        <div class="field">
          <span>保存方式</span>
          <div class="remote-mode" role="radiogroup" aria-label="保存方式">
            <button
              class="remote-mode-option"
              class:active={remoteMode === "memory"}
              onclick={() => (remoteMode = "memory")}
            >
              <strong>仅在内存</strong>
              <small>保存时只上传回 S3</small>
            </button>
            <button
              class="remote-mode-option"
              class:active={remoteMode === "local"}
              onclick={() => (remoteMode = "local")}
            >
              <strong>保存到本地</strong>
              <small>镜像到 Storage/remote/{remoteLocalDir} 并轮换备份</small>
            </button>
          </div>
        </div>
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

      {#if isTauriRuntime()}
        <label class="field">
          <span>密钥文件(可选)</span>
          <div class="path-row">
            <input
              class="text-input"
              type="text"
              bind:value={keyfilePath}
              placeholder="点击右侧选择密钥文件"
              readonly
            />
            <button class="browse-button" onclick={pickKeyfile} title="选择密钥文件">
              <AppIcon name="folder" size={15} />
            </button>
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
          onclick={modal === "open"
            ? confirmOpen
            : modal === "create"
              ? confirmCreate
              : remoteTab === "open"
                ? confirmRemoteOpen
                : confirmRemoteCreate}
          disabled={busy}
        >
          {busy
            ? "处理中…"
            : modal === "open"
              ? "解锁"
              : modal === "create"
                ? "创建"
                : remoteTab === "open"
                  ? "解锁"
                  : "创建"}
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
    flex-wrap: nowrap;
    justify-content: center;
    gap: 6px;
    margin-top: 28px;
  }

  .welcome-button {
    display: inline-flex;
    align-items: center;
    white-space: nowrap;
    gap: 6px;
    height: 34px;
    padding: 0 10px;
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

  .remote-tabs {
    display: flex;
    gap: 6px;
    margin-top: 12px;
  }

  .remote-tab {
    height: 28px;
    padding: 0 14px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .remote-tab.active {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--input-bg));
  }

  .remote-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 180px;
    margin-top: 5px;
    padding: 6px;
    overflow-y: auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
  }

  .remote-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 8px;
    border: 1px solid transparent;
    border-radius: var(--settings-control-radius, 5px);
    color: var(--text-secondary);
    background: transparent;
    font-size: 11px;
    text-align: left;
    cursor: pointer;
  }

  .remote-item:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .remote-item.active {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--hover-bg));
  }

  .remote-item-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .remote-item-size {
    flex: 0 0 auto;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }

  .remote-empty {
    margin: 0;
    padding: 10px 8px;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
    text-align: center;
  }

  .remote-refresh {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    margin-top: 6px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--card-bg);
    font-size: var(--font-size-tiny, 10px);
    cursor: pointer;
  }

  .remote-refresh:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .remote-refresh:disabled {
    cursor: wait;
    opacity: 0.6;
  }

  .remote-mode {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    margin-top: 5px;
  }

  .remote-mode-option {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 8px 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--input-bg);
    text-align: left;
    cursor: pointer;
  }

  .remote-mode-option.active {
    border-color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--input-bg));
  }

  .remote-mode-option strong {
    font-size: var(--font-size-secondary, 11px);
    font-weight: 560;
  }

  .remote-mode-option small {
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    line-height: 1.4;
  }

  .remote-config {
    margin-top: 12px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
  }

  .remote-config-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border: none;
    border-radius: inherit;
    color: var(--text-secondary);
    background: transparent;
    font-size: var(--font-size-secondary, 11px);
    text-align: left;
    cursor: pointer;
  }

  .remote-config-toggle:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .remote-config-status {
    flex: 1;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-align: right;
  }

  .remote-config-body {
    padding: 2px 10px 10px;
    border-top: 1px solid var(--border-subtle);
  }

  .remote-config-body .field {
    margin-top: 8px;
  }

  .remote-config-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .remote-config-note {
    margin: 8px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    line-height: 1.5;
  }
</style>
