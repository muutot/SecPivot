<script lang="ts">
  import {
    appSettings,
    findRemoteProfile,
    remoteMirrorPath,
    remoteProfilePath,
    remoteProfilesForKind,
  } from "$lib/services/settings";
  import type { RemoteKind, RemoteProfilePath } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Select from "$lib/components/Select.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";

  interface Props {
    onclose: () => void;
    showHeader?: boolean;
    kind: RemoteKind;
  }

  let { onclose, showHeader = true, kind }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const profiles = $derived(remoteProfilesForKind(s.remoteProfiles, kind));
  const activeProfile = $derived.by(() => {
    const selected = findRemoteProfile(s.remoteProfiles, s.activeRemote);
    return selected?.settings.kind === kind ? selected : profiles[0];
  });
  const activePath = $derived(remoteProfilePath(activeProfile));
  const remote = $derived(activeProfile.settings);
  const activeName = $derived(activeProfile.name);
  const nameConflict = $derived(
    activeName.trim() !== "" &&
      profiles.some(
        (profile) =>
          remoteProfilePath(profile) !== activePath && profile.name.trim() === activeName.trim(),
      ),
  );
  const mirrorPath = $derived(remoteMirrorPath(activeProfile));
  const kindLabel = $derived(kind === "webdav" ? "WebDAV" : "S3");

  $effect(() => {
    if (s.activeRemote !== activePath) appSettings.setActiveRemote(activePath);
  });

  function change<K extends import("$lib/services/settings").RemoteUpdateKey>(
    key: K,
    value: import("$lib/services/settings").RemoteUpdateValue<K>,
  ): void {
    appSettings.updateRemote(activePath, key, value);
  }
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · 远程 · {kindLabel}</span>
      <h2>{kindLabel}</h2>
      <p>管理 {kindLabel} 远程数据库配置；每条配置只属于当前协议。</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label="关闭">×</button>
  </header>
{/if}

<div class="settings-scroll settings-scroll--stack-rows">
  <section class="setting-card">
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="cloud" size={17} /></span>
        <div>
          <strong>{kindLabel} 配置</strong>
          <p>配置路径按“协议/配置名”区分，可在同一协议下维护多套连接</p>
        </div>
      </div>
      <Select
        id="remote-profile-select"
        className="setting-row-input"
        value={activePath}
        ariaLabel={`${kindLabel} 配置`}
        options={profiles.map((profile) => ({
          value: remoteProfilePath(profile),
          label: profile.name,
        }))}
        onchange={(path) => appSettings.setActiveRemote(path as RemoteProfilePath)}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="edit" size={17} /></span>
        <div>
          <strong>配置名称</strong>
          <p>名称会作为配置路径和本地镜像目录的最后一级</p>
        </div>
      </div>
      <input
        id="remote-profile-name"
        class="settings-input setting-row-input"
        class:input-invalid={nameConflict}
        type="text"
        value={activeName}
        spellcheck="false"
        oninput={(event) => appSettings.renameRemoteProfile(activePath, event.currentTarget.value)}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="folder" size={17} /></span>
        <div>
          <strong>配置路径</strong>
          <p>S3 与 WebDAV 使用独立命名空间</p>
        </div>
      </div>
      <code class="mirror-dir">{activePath}</code>
    </div>
    {#if nameConflict}
      <p class="settings-note input-error">同一协议下的配置名称不允许重复</p>
    {/if}
    <div class="profile-actions">
      <button
        class="settings-action-button"
        onclick={() => appSettings.addRemoteProfile(kind, "")}
        type="button"
      >
        添加配置
      </button>
      <button
        class="settings-action-button"
        disabled={profiles.length <= 1}
        onclick={() => appSettings.removeRemoteProfile(activePath)}
        type="button"
      >
        删除当前配置
      </button>
    </div>
  </section>

  <section class="setting-card">
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
        <div>
          <strong>{kind === "webdav" ? "WebDAV 服务地址" : "服务地址"}</strong>
          <p>
            {kind === "webdav"
              ? "基地址（如 https://dav.example.com/remote.php/dav/files/user）"
              : "兼容 AWS S3、MinIO 等 S3 API 服务"}
          </p>
        </div>
      </div>
      <input
        class="settings-input setting-row-input"
        type="text"
        value={remote.endpoint}
        placeholder={kind === "webdav" ? "https://dav.example.com/dav" : "https://s3.amazonaws.com"}
        spellcheck="false"
        oninput={(event) => change("endpoint", event.currentTarget.value)}
      />
    </div>

    {#if remote.kind === "s3"}
      <div class="setting-row">
        <div class="setting-heading">
          <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
          <div>
            <strong>区域</strong>
            <p>存储桶所在的地域</p>
          </div>
        </div>
        <input
          class="settings-input setting-row-input"
          type="text"
          value={remote.region}
          placeholder="us-east-1"
          oninput={(event) => change("region", event.currentTarget.value)}
        />
      </div>
      <div class="setting-row">
        <div class="setting-heading">
          <span class="setting-icon"><AppIcon name="folder" size={17} /></span>
          <div>
            <strong>存储桶</strong>
            <p>对象存储的桶名称</p>
          </div>
        </div>
        <input
          class="settings-input setting-row-input"
          type="text"
          value={remote.bucket}
          placeholder="my-bucket"
          oninput={(event) => change("bucket", event.currentTarget.value)}
        />
      </div>
    {/if}

    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="key" size={17} /></span>
        <div>
          <strong>{kind === "webdav" ? "用户名" : "Access Key"}</strong>
          <p>{kind === "webdav" ? "WebDAV 登录用户名" : "远程存储的访问密钥 ID"}</p>
        </div>
      </div>
      <input
        class="settings-input setting-row-input"
        type="text"
        autocomplete="off"
        spellcheck="false"
        value={remote.accessKey}
        placeholder={kind === "webdav" ? "user" : "AKIA..."}
        oninput={(event) => change("accessKey", event.currentTarget.value)}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="lock" size={17} /></span>
        <div>
          <strong>{kind === "webdav" ? "密码" : "Secret Key"}</strong>
          <p>{kind === "webdav" ? "WebDAV 登录密码" : "与 Access Key 配对的私钥"}</p>
        </div>
      </div>
      <input
        class="settings-input setting-row-input"
        type="password"
        autocomplete="off"
        spellcheck="false"
        value={remote.secretKey}
        placeholder="••••••••"
        oninput={(event) => change("secretKey", event.currentTarget.value)}
      />
    </div>
  </section>

  <p class="settings-note warn">
    风险提示：凭据以 DPAPI 加密后写入
    config.json（属次要凭据）。若泄露仅影响远程存储读写，不会暴露任何数据库内容。
  </p>

  <section class="setting-card">
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="filter" size={17} /></span>
        <div>
          <strong>对象前缀</strong>
          <p>远程文件列表的前缀过滤（可选）</p>
        </div>
      </div>
      <input
        id="remote-prefix"
        class="settings-input setting-row-input"
        type="text"
        value={remote.prefix}
        placeholder="vaults/"
        oninput={(event) => change("prefix", event.currentTarget.value)}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="download" size={17} /></span>
        <div>
          <strong>本地镜像目录</strong>
          <p>“本地镜像”模式按协议和配置名分层落盘</p>
        </div>
      </div>
      <code class="mirror-dir">{mirrorPath}</code>
    </div>
    <p class="settings-note">本地副本保存在 Storage/remote/{mirrorPath}/ 下。</p>
  </section>

  <SettingRangeCard
    icon="clock"
    label="本地备份数量"
    description="每次保存时保留的带时间戳备份份数，0 表示不保留"
    value={remote.backupCount}
    valueLabel={`${remote.backupCount} 份`}
    min={0}
    max={10}
    onchange={(value) => change("backupCount", value)}
  />

  <section class="setting-card">
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="file" size={17} /></span>
        <div>
          <strong>备份文件名模板</strong>
          <p>时间戳备份的命名规则</p>
        </div>
      </div>
      <input
        id="remote-backup-template"
        class="settings-input setting-row-input"
        type="text"
        value={remote.backupTemplate}
        placeholder={"{name}.{timestamp}.{ext}.bak"}
        spellcheck="false"
        oninput={(event) => change("backupTemplate", event.currentTarget.value)}
      />
    </div>
    <p class="settings-note">
      占位符：{"{name}"} 文件主名、{"{timestamp}"} 时间戳、{"{ext}"} 原扩展名
    </p>
  </section>

  <p class="auto-save-note">修改即时生效并自动保存</p>
</div>

<style>
  .profile-actions {
    display: flex;
    gap: 8px;
    margin: 10px 0 2px;
  }
</style>
