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
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import { t } from "$lib/i18n";
  import StandaloneVaultShell from "$lib/components/StandaloneVaultShell.svelte";
  import VaultCredentialFields from "$lib/components/VaultCredentialFields.svelte";
  import { formatBytes } from "$lib/utils/format";

  interface Props {
    onopened: () => void;
  }

  let { onopened }: Props = $props();

  let settings = $state(get(appSettings));
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      settings = value;
    });
    return unsubscribe;
  });

  const recentFiles = $derived(settings.general.recentFiles);
  const lang = $derived(settings.general.language);

  const activeProfile = $derived(activeRemoteProfile(settings));
  const activeRemoteName = $derived(activeProfile.name);
  const remoteMirrorDir = $derived(remoteMirrorPath(activeProfile));
  const activeKindProfiles = $derived(
    remoteProfilesForKind(settings.remoteProfiles, activeProfile.settings.kind),
  );
  const remoteNameConflict = $derived(
    activeRemoteName.trim() !== "" &&
      activeKindProfiles.filter((profile) => profile.name.trim() === activeRemoteName.trim())
        .length > 1,
  );

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

  function isRemoteConfigured(r: RemoteSettings): boolean {
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
        error = t(lang, "welcome.noRemoteDb");
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
      error = t(lang, "welcome.pickRemoteFile");
      return;
    }
    if (!password) {
      error = t(lang, "welcome.needPassword");
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
      error = t(lang, "welcome.remoteKeyRequired");
      return;
    }
    if (!password) {
      error = t(lang, "welcome.needPassword");
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
          { name: t(lang, "welcome.filterKdbx"), extensions: ["kdbx"] },
          { name: t(lang, "welcome.filterAll"), extensions: ["*"] },
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
      error = t(lang, "welcome.pickDbFile");
      return;
    }
    if (!password) {
      error = t(lang, "welcome.needPassword");
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
        { name: t(lang, "welcome.filterKdbx"), extensions: ["kdbx"] },
        { name: t(lang, "welcome.filterAll"), extensions: ["*"] },
      ],
    });
    if (selected) path = String(selected);
  }

  async function confirmCreate(): Promise<void> {
    if (!password) {
      error = t(lang, "welcome.needPassword");
      return;
    }
    if (password !== confirm) {
      error = t(lang, "welcome.passwordMismatch");
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
            { name: t(lang, "welcome.filterKdbx"), extensions: ["kdbx"] },
            { name: t(lang, "welcome.filterAll"), extensions: ["*"] },
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

<StandaloneVaultShell
  title="SecPivot"
  subtitle={t(lang, "welcome.tagline")}
  logoSrc="/app-icon.png"
>
  <div class="welcome-actions">
    <Button variant="primary" onclick={handleOpen} disabled={busy} {busy}>
      <AppIcon name="open" size={14} />{t(lang, "welcome.openDb")}
    </Button>
    <Button onclick={handleCreate} disabled={busy} {busy}>
      <AppIcon name="plus" size={14} />{t(lang, "welcome.newDb")}
    </Button>
    {#if isTauriRuntime()}
      <Button onclick={handleRemoteOpen} disabled={busy}>
        <AppIcon name="cloud" size={14} />{t(lang, "welcome.remoteDb")}
      </Button>
    {/if}
  </div>

  <p class="welcome-hint">{t(lang, "welcome.hint")}</p>

  {#if recentFiles.length > 0}
    <div class="recent-section">
      <p class="recent-label">{t(lang, "welcome.recent")}</p>
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
        <span class="guard-title">{t(lang, "welcome.guardTitle")}</span>
        <span class="guard-desc">{t(lang, "welcome.guardDesc")}</span>
      </div>
      <Toggle
        checked={guardEnabled}
        ariaLabel={t(lang, "welcome.guardTitle")}
        onchange={(next) => {
          appSettings.updateSecurity("screenCaptureGuard", next);
          void appSettings.flush();
        }}
      />
    </div>
  {/if}
</StandaloneVaultShell>

{#if modal === "open"}
  <ModalShell
    title={isDemo ? t(lang, "welcome.unlockTitle") : t(lang, "welcome.openTitle")}
    description={path}
    size="small"
    closeOnEscape
    onclose={() => (modal = "none")}
  >
    {#snippet icon()}<AppIcon name="lock" size={18} />{/snippet}
    {#snippet children()}
      <VaultCredentialFields
        bind:password
        bind:keyfilePath
        bind:showPassword
        {busy}
        {error}
        isCreate={false}
        {isDemo}
        onPickKeyfile={pickKeyfile}
      />
    {/snippet}
    {#snippet actions()}
      <Button onclick={() => (modal = "none")} disabled={busy}>{t(lang, "common.cancel")}</Button>
      <Button variant="primary" onclick={confirmOpen} disabled={busy} {busy}
        >{t(lang, "welcome.unlock")}</Button
      >
    {/snippet}
  </ModalShell>
{:else if modal === "create"}
  <ModalShell
    title={t(lang, "welcome.createTitle")}
    description={t(lang, "welcome.createDesc")}
    size="small"
    closeOnEscape
    onclose={() => (modal = "none")}
  >
    {#snippet icon()}<AppIcon name="folder-plus" size={18} />{/snippet}
    {#snippet children()}
      <VaultCredentialFields
        bind:password
        bind:confirm
        bind:keyfilePath
        bind:showPassword
        bind:path
        {busy}
        {error}
        isCreate={true}
        showPathField={true}
        onPickKeyfile={pickKeyfile}
        onPickCreatePath={pickCreatePath}
      />
    {/snippet}
    {#snippet actions()}
      <Button onclick={() => (modal = "none")} disabled={busy}>{t(lang, "common.cancel")}</Button>
      <Button variant="primary" onclick={confirmCreate} disabled={busy} {busy}
        >{t(lang, "welcome.create")}</Button
      >
    {/snippet}
  </ModalShell>
{:else if modal === "remote"}
  <ModalShell
    title={t(lang, "welcome.remoteTitle", { kind: remoteKindLabel })}
    description={t(lang, "welcome.remoteDesc", { kind: remoteKindLabel })}
    size="medium"
    scrollable
    closeOnEscape
    onclose={() => (modal = "none")}
  >
    {#snippet icon()}<AppIcon name="cloud" size={18} />{/snippet}
    {#snippet headerActions()}
      <Select
        className="remote-kind-picker"
        value={remote.kind}
        ariaLabel={t(lang, "welcome.transportType")}
        options={[
          { value: "webdav", label: "WebDAV" },
          { value: "s3", label: "S3" },
        ]}
        onchange={changeRemoteKind}
      />
    {/snippet}
    {#snippet children()}
      <div class="remote-tabs" role="tablist" aria-label={t(lang, "welcome.remoteOps")}>
        <button
          class="remote-tab"
          class:active={remoteTab === "open"}
          onclick={() => switchRemoteTab("open")}>{t(lang, "welcome.tabOpen")}</button
        >
        <button
          class="remote-tab"
          class:active={remoteTab === "create"}
          onclick={() => switchRemoteTab("create")}>{t(lang, "welcome.tabCreate")}</button
        >
        <button
          class="remote-tab"
          class:active={remoteTab === "config"}
          onclick={() => switchRemoteTab("config")}>{t(lang, "welcome.tabConfig")}</button
        >
      </div>

      {#if remoteTab === "config"}
        <div class="field">
          <span>{t(lang, "welcome.remoteConfig")}</span>
          <div class="profile-bar">
            <Select
              className="profile-select"
              value={settings.activeRemote}
              ariaLabel={t(lang, "welcome.remoteConfig")}
              options={activeKindProfiles.map((profile) => ({
                value: remoteProfilePath(profile),
                label: profile.name,
              }))}
              onchange={(path) => appSettings.setActiveRemote(path as RemoteProfilePath)}
            />
            <Button onclick={() => appSettings.addRemoteProfile(remote.kind, "")}
              >{t(lang, "welcome.addProfile")}</Button
            >
            <Button
              disabled={activeKindProfiles.length <= 1}
              onclick={() => appSettings.removeRemoteProfile(settings.activeRemote)}
              >{t(lang, "welcome.deleteProfile")}</Button
            >
          </div>
        </div>
        <div class="field">
          <span>{t(lang, "welcome.configName")}</span>
          <TextField
            invalid={remoteNameConflict}
            value={activeRemoteName}
            placeholder="config_1"
            spellcheck={false}
            oninput={(e) =>
              appSettings.renameRemoteProfile(settings.activeRemote, e.currentTarget.value)}
          />
          {#if remoteNameConflict}<p class="modal-error">{t(lang, "welcome.nameConflict")}</p>{/if}
        </div>
        <div class="field">
          <span>{t(lang, "welcome.configPath")}</span>
          <code class="remote-profile-path">{settings.activeRemote}</code>
        </div>
        <div class="field">
          <span>{t(lang, "welcome.endpoint")}</span>
          <TextField
            value={remote.endpoint}
            placeholder={remote.kind === "webdav"
              ? "https://dav.example.com/dav"
              : "https://s3.amazonaws.com"}
            spellcheck={false}
            oninput={(e) => changeRemote("endpoint", e.currentTarget.value)}
          />
        </div>
        {#if remote.kind !== "webdav"}
          <div class="remote-config-grid">
            <div class="field">
              <span>{t(lang, "welcome.region")}</span>
              <TextField
                value={remote.region}
                placeholder="us-east-1"
                spellcheck={false}
                oninput={(e) => changeRemote("region", e.currentTarget.value)}
              />
            </div>
            <div class="field">
              <span>{t(lang, "welcome.bucket")}</span>
              <TextField
                value={remote.bucket}
                placeholder="my-bucket"
                spellcheck={false}
                oninput={(e) => changeRemote("bucket", e.currentTarget.value)}
              />
            </div>
          </div>
        {/if}
        <div class="field">
          <span
            >{remote.kind === "webdav" ? t(lang, "welcome.accessKey") : t(lang, "welcome.accessKeyS3")}</span
          >
          <TextField
            value={remote.accessKey}
            placeholder={remote.kind === "webdav" ? "user" : "AKIA..."}
            autocomplete="off"
            spellcheck={false}
            oninput={(e) => changeRemote("accessKey", e.currentTarget.value)}
          />
        </div>
        <div class="field">
          <span>{remote.kind === "webdav" ? t(lang, "welcome.secret") : t(lang, "welcome.secretS3")}</span>
          <TextField
            type="password"
            value={remote.secretKey}
            placeholder="••••••••"
            autocomplete="off"
            spellcheck={false}
            oninput={(e) => changeRemote("secretKey", e.currentTarget.value)}
          />
        </div>
        <p class="remote-config-note">
          {t(lang, "welcome.dpapiNote")}
        </p>
      {:else if remoteTab === "open"}
        <div class="field">
          <span>{t(lang, "welcome.pickRemoteFileTitle")}</span>
          <div class="remote-list">
            {#if remoteLoading && remoteObjects.length === 0}
              <p class="remote-empty">{t(lang, "welcome.loading")}</p>
            {:else if remoteObjects.length === 0}
              <p class="remote-empty">{t(lang, "welcome.noFiles")}</p>
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
            <AppIcon name="refresh" size={13} />{t(lang, "welcome.refresh")}
          </button>
        </div>
      {:else}
        <label class="field">
          <span>{t(lang, "welcome.remoteKey")}</span>
          <TextField bind:value={remoteKey} placeholder="vaults/new.kdbx" spellcheck={false} />
        </label>
      {/if}

      {#if remoteTab !== "config"}
        <div class="field">
          <span>{t(lang, "welcome.saveMode")}</span>
          <div class="remote-mode" role="radiogroup" aria-label={t(lang, "welcome.saveMode")}>
            <button
              class="remote-mode-option"
              class:active={remoteMode === "memory"}
              onclick={() => (remoteMode = "memory")}
            >
              <strong>{t(lang, "welcome.memoryOnly")}</strong
              ><small>{t(lang, "welcome.memoryOnlyDesc")}</small>
            </button>
            <button
              class="remote-mode-option"
              class:active={remoteMode === "local"}
              onclick={() => (remoteMode = "local")}
            >
              <strong>{t(lang, "welcome.mirror")}</strong><small
                >{t(lang, "welcome.mirrorDesc", { dir: remoteMirrorDir })}</small
              >
            </button>
          </div>
        </div>
        <VaultCredentialFields
          bind:password
          bind:keyfilePath
          bind:showPassword
          {busy}
          {error}
          isCreate={false}
          onPickKeyfile={pickKeyfile}
        />
      {/if}
      {#if remoteTab === "config" && error}
        <p class="modal-error">{error}</p>
      {/if}
    {/snippet}
    {#snippet actions()}
      <Button onclick={() => (modal = "none")} disabled={busy}>{t(lang, "common.cancel")}</Button>
      {#if remoteTab !== "config"}
        <Button
          variant="primary"
          onclick={remoteTab === "open" ? confirmRemoteOpen : confirmRemoteCreate}
          disabled={busy}
          {busy}
        >
          {remoteTab === "open" ? t(lang, "welcome.unlock") : t(lang, "welcome.create")}
        </Button>
      {/if}
    {/snippet}
  </ModalShell>
{/if}

<style>
  .welcome-actions {
    display: flex;
    flex-wrap: wrap;
    justify-content: center;
    align-items: center;
    gap: 8px;
    width: 100%;
    margin-top: 28px;
  }

  .welcome-actions :global(.app-icon) {
    flex: 0 0 auto;
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

  :global(.remote-kind-picker) {
    flex: 0 0 auto;
    width: 110px;
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

  .modal-error {
    margin: 10px 0 0;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
  }
</style>
