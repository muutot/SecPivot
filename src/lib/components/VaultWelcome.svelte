<script lang="ts">
  import { get } from "svelte/store";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import {
    activeRemoteProfile,
    appSettings,
    isTauriRuntime,
    remoteMirrorPath,
    remoteProfilePath,
    remoteProfilesForKind,
  } from "$lib/services/settings";
  import { rememberCredential } from "$lib/services/security";
  import { vault } from "$lib/services/vault";
  import type { RemoteMode, RemoteObject } from "$lib/types/vault";
  import type { RemoteKind, RemoteProfilePath, RemoteSettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Select from "$lib/components/Select.svelte";
  import Toggle from "$lib/components/templates/form/Toggle.svelte";
  import { formatBytes } from "$lib/utils/format";

  interface Props {
    onopened: () => void;
  }

  let { onopened }: Props = $props();

  /** Reactive mirror of the settings store. `$derived(get(appSettings))` would
   * evaluate once and freeze (get() is untracked in Svelte 5); subscribing to
   * a $state mirror keeps every derived below fresh. */
  let settings = $state(get(appSettings));
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      settings = value;
    });
    return unsubscribe;
  });

  const recentFiles = $derived(settings.general.recentFiles);

  const activeProfile = $derived(activeRemoteProfile(settings));
  const activeRemoteName = $derived(activeProfile.name);
  /** The mirror folder actually created for "本地镜像" mode. */
  const remoteMirrorDir = $derived(remoteMirrorPath(activeProfile));
  const activeKindProfiles = $derived(
    remoteProfilesForKind(settings.remoteProfiles, activeProfile.settings.kind),
  );
  /** True while the 配置名称 field holds a duplicate of another profile's name. */
  const remoteNameConflict = $derived(
    activeRemoteName.trim() !== "" &&
      activeKindProfiles.filter((profile) => profile.name.trim() === activeRemoteName.trim())
        .length > 1,
  );

  /** Opt-in screen-capture guard: excludes the main window from screenshots/recordings while a vault is open (Windows only). */
  const guardEnabled = $derived(settings.security.screenCaptureGuard);

  type Modal = "none" | "open" | "create" | "remote";
  type RemoteTab = "open" | "create" | "config";

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

  const remote = $derived(activeProfile.settings);
  const remoteKindLabel = $derived(remote.kind === "webdav" ? "WebDAV" : "S3");
  const remoteConfigured = $derived(
    remote.kind === "webdav"
      ? Boolean(remote.endpoint)
      : Boolean(remote.endpoint && remote.bucket && remote.accessKey && remote.secretKey),
  );

  /** True when the active profile's transport has the minimum required fields. */
  function isRemoteConfigured(r: import("$lib/types/settings").RemoteSettings): boolean {
    return r.kind === "webdav"
      ? Boolean(r.endpoint)
      : Boolean(r.endpoint && r.bucket && r.accessKey && r.secretKey);
  }

  function changeRemote<K extends import("$lib/services/settings").RemoteUpdateKey>(
    key: K,
    value: import("$lib/services/settings").RemoteUpdateValue<K>,
  ): void {
    appSettings.updateRemote(settings.activeRemote, key, value);
  }

  /** Kind switch from the modal-head picker: reset the stale object list and
   *  reload for the new transport. On the 打开 tab, an unconfigured new kind
   *  drops into 配置 instead of showing another transport's files. */
  async function changeRemoteKind(v: string): Promise<void> {
    const kind = v as RemoteKind;
    if (kind === remote.kind) return;
    const first = remoteProfilesForKind(get(appSettings).remoteProfiles, kind)[0];
    if (!first) return;
    appSettings.setActiveRemote(remoteProfilePath(first));
    remoteKey = "";
    error = "";
    remoteObjects = [];
    const configured = isRemoteConfigured(activeRemoteProfile(get(appSettings)).settings);
    if (remoteTab === "open") {
      if (configured) {
        await loadRemoteObjects();
      } else {
        remoteTab = "config";
      }
    }
  }

  async function handleRemoteOpen(): Promise<void> {
    remoteTab = remoteConfigured ? "open" : "config";
    remoteKey = "";
    remoteMode = "memory";
    keyfilePath = "";
    password = "";
    error = "";
    modal = "remote";
    if (remoteConfigured) await loadRemoteObjects();
  }

  async function loadRemoteObjects(): Promise<void> {
    if (!isTauriRuntime()) return;
    remoteLoading = true;
    error = "";
    try {
      remoteObjects = await vault.listRemoteObjects();
      if (remoteObjects.length === 0) {
        error =
          "远程目录连接成功，但该目录下没有数据库文件（.kdbx）。请确认对象前缀指向包含数据库文件的目录，或切换到“新建”在此目录创建数据库";
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
    if (tab === "open") void loadRemoteObjects();
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
        filters: [
          { name: "KeePass 数据库", extensions: ["kdbx"] },
          { name: "所有文件", extensions: ["*"] },
        ],
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
    const selected = await open({ multiple: false });
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
      filters: [
        { name: "KeePass 数据库", extensions: ["kdbx"] },
        { name: "所有文件", extensions: ["*"] },
      ],
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
            filters: [
              { name: "KeePass 数据库", extensions: ["kdbx"] },
              { name: "所有文件", extensions: ["*"] },
            ],
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
    <div class="welcome-header">
      <div class="welcome-logo">
        <img class="welcome-logo-img" src="/app-icon.png" alt="SecPivot" />
      </div>
      <div class="welcome-heading">
        <h1 class="welcome-title">SecPivot</h1>
        <p class="welcome-subtitle">本地优先的 KeePass 密码管理器</p>
      </div>
    </div>

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

    {#if isTauriRuntime()}
      <div class="welcome-guard">
        <div class="guard-info">
          <span class="guard-title">防截屏守卫</span>
          <span class="guard-desc">库打开期间窗口不出现在截屏/录屏中</span>
        </div>
        <Toggle
          checked={guardEnabled}
          ariaLabel="防截屏守卫"
          onchange={(next) => {
            appSettings.updateSecurity("screenCaptureGuard", next);
            void appSettings.flush();
          }}
        />
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
                : `远程数据库 (${remoteKindLabel})`}</strong
          >
          <p>
            {modal === "open"
              ? path
              : modal === "create"
                ? "选择一个位置并设置主密码"
                : `从 ${remoteKindLabel} 打开或创建数据库`}
          </p>
        </div>
        {#if modal === "remote"}
          <Select
            className="remote-kind-picker"
            value={remote.kind}
            ariaLabel="传输类型"
            options={[
              { value: "webdav", label: "WebDAV" },
              { value: "s3", label: "S3" },
            ]}
            onchange={changeRemoteKind}
          />
        {/if}
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
          <button
            class="remote-tab"
            class:active={remoteTab === "config"}
            onclick={() => switchRemoteTab("config")}
          >
            配置
          </button>
        </div>

        {#if remoteTab === "config"}
          <div class="field">
            <span>远程配置</span>
            <div class="profile-bar">
              <Select
                className="profile-select"
                value={settings.activeRemote}
                ariaLabel="远程配置"
                options={activeKindProfiles.map((profile) => ({
                  value: remoteProfilePath(profile),
                  label: profile.name,
                }))}
                onchange={(path) => appSettings.setActiveRemote(path as RemoteProfilePath)}
              />
              <button
                class="welcome-button"
                type="button"
                onclick={() => appSettings.addRemoteProfile(remote.kind, "")}
              >
                添加
              </button>
              <button
                class="welcome-button"
                type="button"
                disabled={activeKindProfiles.length <= 1}
                onclick={() => appSettings.removeRemoteProfile(settings.activeRemote)}
              >
                删除
              </button>
            </div>
          </div>
          <div class="field">
            <span>配置名称</span>
            <input
              class="text-input"
              class:input-invalid={remoteNameConflict}
              type="text"
              value={activeRemoteName}
              placeholder="config_1"
              spellcheck="false"
              oninput={(e) =>
                appSettings.renameRemoteProfile(settings.activeRemote, e.currentTarget.value)}
            />
            {#if remoteNameConflict}
              <p class="modal-error">同一协议下的配置名不允许重复</p>
            {/if}
          </div>
          <div class="field">
            <span>配置路径</span>
            <code class="remote-profile-path">{settings.activeRemote}</code>
          </div>
          <div class="field">
            <span>服务地址</span>
            <input
              class="text-input"
              type="text"
              value={remote.endpoint}
              placeholder={remote.kind === "webdav"
                ? "https://dav.example.com/dav"
                : "https://s3.amazonaws.com"}
              spellcheck="false"
              oninput={(e) => changeRemote("endpoint", e.currentTarget.value)}
            />
          </div>
          {#if remote.kind !== "webdav"}
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
          {/if}
          <div class="field">
            <span>{remote.kind === "webdav" ? "用户名" : "Access Key"}</span>
            <input
              class="text-input"
              type="text"
              value={remote.accessKey}
              placeholder={remote.kind === "webdav" ? "user" : "AKIA..."}
              autocomplete="off"
              spellcheck="false"
              oninput={(e) => changeRemote("accessKey", e.currentTarget.value)}
            />
          </div>
          <div class="field">
            <span>{remote.kind === "webdav" ? "密码" : "Secret Key"}</span>
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
            凭据以 DPAPI 加密后保存在
            config.json，仅用于访问远程存储；配置完成后切到「打开」标签查看远程文件。
          </p>
        {:else if remoteTab === "open"}
          <div class="field">
            <span>选择远程文件</span>
            <div class="remote-list">
              {#if remoteLoading && remoteObjects.length === 0}
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

        {#if remoteTab !== "config"}
          <div class="field">
            <span>保存方式</span>
            <div class="remote-mode" role="radiogroup" aria-label="保存方式">
              <button
                class="remote-mode-option"
                class:active={remoteMode === "memory"}
                onclick={() => (remoteMode = "memory")}
              >
                <strong>仅在内存</strong>
                <small>保存时只上传回远程存储</small>
              </button>
              <button
                class="remote-mode-option"
                class:active={remoteMode === "local"}
                onclick={() => (remoteMode = "local")}
              >
                <strong>本地镜像</strong>
                <small>保存时上传回远程并镜像到 Storage/remote/{remoteMirrorDir}</small>
              </button>
            </div>
          </div>
        {/if}
      {/if}

      {#if !(modal === "remote" && remoteTab === "config")}
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
      {/if}

      {#if modal === "create"}
        <label class="field">
          <span>确认主密码</span>
          <div class="path-row">
            <input class="text-input" type="password" bind:value={confirm} />
          </div>
        </label>
      {/if}

      {#if isTauriRuntime() && !(modal === "remote" && remoteTab === "config")}
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
        {#if !(modal === "remote" && remoteTab === "config")}
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
        {/if}
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
    transform: translateY(10vh);
  }

  .welcome-header {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .welcome-logo {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 56px;
    flex: 0 0 auto;
    overflow: hidden;
  }

  .welcome-logo-img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .welcome-heading {
    min-width: 0;
    text-align: left;
  }

  .welcome-title {
    margin: 0;
    font-size: 24px;
    font-weight: 590;
    letter-spacing: 0.01em;
  }

  .welcome-subtitle {
    margin: 4px 0 0;
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

  .welcome-guard {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    margin-top: 20px;
    padding: 9px 12px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--card-bg);
  }

  .guard-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    text-align: left;
  }

  .guard-title {
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
  }

  .guard-desc {
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
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

  .modal-head > div {
    min-width: 0;
    flex: 1 1 auto;
  }

  :global(.remote-kind-picker) {
    flex: 0 0 auto;
    width: 110px;
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

  /* .text-input / .modal-actions / .modal-button come from the shared
   * modal primitives (modal-shared.css via app.css); only this surface's
   * unique layout lives here. */

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
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
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

  .remote-config-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .profile-bar {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  :global(.profile-select) {
    flex: 1;
  }

  .remote-profile-path {
    display: block;
    min-height: 30px;
    padding: 7px 10px;
    overflow: hidden;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .remote-config-note {
    margin: 8px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    line-height: 1.5;
  }
</style>
