<script lang="ts">
  import { appSettings, sanitizeDirName } from "$lib/services/settings";
  import type { RemoteSettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Select from "$lib/components/Select.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";

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

  const remote = $derived(s.remote);

  const activeName = $derived(s.remoteProfiles[s.activeRemote].name);
  /** True while the name field holds a duplicate of another profile's name. */
  const nameConflict = $derived(
    activeName.trim() !== "" &&
      s.remoteProfiles.filter((p) => p.name.trim() === activeName.trim()).length > 1,
  );
  /** The mirror folder actually created for "本地镜像" mode. */
  const mirrorDir = $derived(sanitizeDirName(activeName));

  function change<K extends import("$lib/services/settings").RemoteUpdateKey>(
    key: K,
    value: import("$lib/services/settings").RemoteBlockValue<K>,
  ): void {
    appSettings.updateRemote(key, value);
  }
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · 远程</span>
      <h2>远程</h2>
      <p>S3 兼容对象存储或 WebDAV 配置，用于远程打开与创建数据库。</p>
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
          <strong>配置</strong>
          <p>选择当前使用的远程配置，可维护多套并存</p>
        </div>
      </div>
      <Select
        id="remote-profile-select"
        className="setting-row-input"
        value={s.activeRemote}
        ariaLabel="远程配置"
        options={s.remoteProfiles.map((p, i) => ({ value: i, label: p.name }))}
        onchange={(v) => appSettings.setActiveRemote(Number(v))}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="edit" size={17} /></span>
        <div>
          <strong>配置名称</strong>
          <p>当前配置的显示名称，也是本地镜像目录名，不允许重复</p>
        </div>
      </div>
      <input
        id="remote-profile-name"
        class="settings-input setting-row-input"
        class:input-invalid={nameConflict}
        type="text"
        value={s.remoteProfiles[s.activeRemote].name}
        oninput={(e) => appSettings.renameRemoteProfile(s.activeRemote, e.currentTarget.value)}
      />
    </div>
    {#if nameConflict}
      <p class="settings-note input-error">远程名不允许重复，请输入其他名称</p>
    {/if}
    <div class="profile-actions">
      <button
        class="settings-action-button"
        onclick={() => appSettings.addRemoteProfile("")}
        type="button"
      >
        添加配置
      </button>
      <button
        class="settings-action-button"
        disabled={s.remoteProfiles.length <= 1}
        onclick={() => appSettings.removeRemoteProfile(s.activeRemote)}
        type="button"
      >
        删除当前配置
      </button>
    </div>
  </section>

  <section class="setting-card remote-group-card">
    <div class="remote-group-heading">
      <span class="remote-group-tag">WebDAV</span>
      <p>网盘 / 自建 WebDAV 服务的连接配置，独立于 S3</p>
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
        <div>
          <strong>WebDAV 服务地址</strong>
          <p>基地址（如 https://dav.example.com/remote.php/dav/files/user）</p>
        </div>
      </div>
      <input
        class="settings-input setting-row-input"
        type="text"
        value={remote.webdav.endpoint}
        placeholder="https://dav.example.com/dav"
        oninput={(e) => change("webdav.endpoint", e.currentTarget.value)}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="key" size={17} /></span>
        <div>
          <strong>用户名</strong>
          <p>WebDAV 登录用户名</p>
        </div>
      </div>
      <input
        class="settings-input setting-row-input"
        type="text"
        autocomplete="off"
        spellcheck="false"
        value={remote.webdav.accessKey}
        placeholder="user"
        oninput={(e) => change("webdav.accessKey", e.currentTarget.value)}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="lock" size={17} /></span>
        <div>
          <strong>密码</strong>
          <p>WebDAV 登录密码</p>
        </div>
      </div>
      <input
        class="settings-input setting-row-input"
        type="password"
        autocomplete="off"
        spellcheck="false"
        value={remote.webdav.secretKey}
        placeholder="••••••••"
        oninput={(e) => change("webdav.secretKey", e.currentTarget.value)}
      />
    </div>
  </section>

  <section class="setting-card remote-group-card">
    <div class="remote-group-heading">
      <span class="remote-group-tag">S3</span>
      <p>S3 兼容对象存储（AWS / MinIO / 各类云）的连接配置，独立于 WebDAV</p>
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
        <div>
          <strong>服务地址</strong>
          <p>兼容 AWS S3、MinIO 等 S3 API 服务</p>
        </div>
      </div>
      <input
        class="settings-input setting-row-input"
        type="text"
        value={remote.s3.endpoint}
        placeholder="https://s3.amazonaws.com"
        oninput={(e) => change("s3.endpoint", e.currentTarget.value)}
      />
    </div>
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
        value={remote.s3.region}
        placeholder="us-east-1"
        oninput={(e) => change("s3.region", e.currentTarget.value)}
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
        value={remote.s3.bucket}
        placeholder="my-bucket"
        oninput={(e) => change("s3.bucket", e.currentTarget.value)}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="key" size={17} /></span>
        <div>
          <strong>Access Key</strong>
          <p>远程存储的访问密钥 ID</p>
        </div>
      </div>
      <input
        class="settings-input setting-row-input"
        type="text"
        autocomplete="off"
        spellcheck="false"
        value={remote.s3.accessKey}
        placeholder="AKIA..."
        oninput={(e) => change("s3.accessKey", e.currentTarget.value)}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="lock" size={17} /></span>
        <div>
          <strong>Secret Key</strong>
          <p>与 Access Key 配对的私钥</p>
        </div>
      </div>
      <input
        class="settings-input setting-row-input"
        type="password"
        autocomplete="off"
        spellcheck="false"
        value={remote.s3.secretKey}
        placeholder="••••••••"
        oninput={(e) => change("s3.secretKey", e.currentTarget.value)}
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
        oninput={(e) => change("prefix", e.currentTarget.value)}
      />
    </div>
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="download" size={17} /></span>
        <div>
          <strong>本地镜像目录</strong>
          <p>“本地镜像”模式的落盘目录，自动随远程名命名</p>
        </div>
      </div>
      <code class="mirror-dir">{mirrorDir}</code>
    </div>
    <p class="settings-note">
      本地副本保存在 Storage/remote/{mirrorDir}/ 下，由远程名自动命名；因此远程名不允许重复。
    </p>
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
        oninput={(e) => change("backupTemplate", e.currentTarget.value)}
      />
    </div>
    <p class="settings-note">
      占位符：{"{name}"} 文件主名、{"{timestamp}"} 时间戳、{"{ext}"} 原扩展名
    </p>
  </section>

  <p class="auto-save-note">修改即时生效并自动保存</p>
</div>

<style>
  .remote-group-card .remote-group-heading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 0 10px;
    margin: -2px 0 12px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .remote-group-heading p {
    margin: 0;
    color: var(--text-muted);
    font-size: var(--settings-note-size, var(--font-size-tiny, 10px));
  }

  .remote-group-tag {
    flex: 0 0 auto;
    padding: 3px 9px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--hover-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    font-weight: 560;
  }

  .profile-actions {
    display: flex;
    gap: 8px;
    margin: 10px 0 2px;
  }
</style>
