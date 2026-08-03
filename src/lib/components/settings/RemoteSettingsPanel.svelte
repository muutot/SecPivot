<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import type { RemoteSettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Select from "$lib/components/Select.svelte";

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

  function change<K extends keyof RemoteSettings>(key: K, value: RemoteSettings[K]): void {
    appSettings.updateRemote(key, value);
  }

  function sliderPercentage(value: number, min: number, max: number): number {
    return Math.round(((value - min) / (max - min)) * 100);
  }
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · 远程</span>
      <h2>远程</h2>
      <p>S3 兼容对象存储配置，用于远程打开与创建数据库。</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label="关闭">×</button>
  </header>
{/if}

<div class="settings-scroll">
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
        className="profile-picker"
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
          <p>当前配置的显示名称</p>
        </div>
      </div>
      <input
        id="remote-profile-name"
        class="settings-input setting-row-input"
        type="text"
        value={s.remoteProfiles[s.activeRemote].name}
        oninput={(e) => appSettings.renameRemoteProfile(s.activeRemote, e.currentTarget.value)}
      />
    </div>
    <div class="profile-actions">
      <button
        class="profile-action-button"
        onclick={() => appSettings.addRemoteProfile("")}
        type="button"
      >
        添加配置
      </button>
      <button
        class="profile-action-button"
        disabled={s.remoteProfiles.length <= 1}
        onclick={() => appSettings.removeRemoteProfile(s.activeRemote)}
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
          <strong>服务地址</strong>
          <p>兼容 AWS S3、MinIO 等 S3 API 服务</p>
        </div>
      </div>
      <input
        id="remote-endpoint"
        class="settings-input setting-row-input"
        type="text"
        value={remote.endpoint}
        placeholder="https://s3.amazonaws.com"
        oninput={(e) => change("endpoint", e.currentTarget.value)}
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
        id="remote-region"
        class="settings-input setting-row-input"
        type="text"
        value={remote.region}
        placeholder="us-east-1"
        oninput={(e) => change("region", e.currentTarget.value)}
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
        id="remote-bucket"
        class="settings-input setting-row-input"
        type="text"
        value={remote.bucket}
        placeholder="my-bucket"
        oninput={(e) => change("bucket", e.currentTarget.value)}
      />
    </div>
  </section>

  <section class="setting-card">
    <div class="setting-row">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="key" size={17} /></span>
        <div>
          <strong>Access Key</strong>
          <p>远程存储的访问密钥 ID</p>
        </div>
      </div>
      <input
        id="remote-access-key"
        class="settings-input setting-row-input"
        type="text"
        autocomplete="off"
        spellcheck="false"
        value={remote.accessKey}
        placeholder="AKIA..."
        oninput={(e) => change("accessKey", e.currentTarget.value)}
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
        id="remote-secret-key"
        class="settings-input setting-row-input"
        type="password"
        autocomplete="off"
        spellcheck="false"
        value={remote.secretKey}
        placeholder="••••••••"
        oninput={(e) => change("secretKey", e.currentTarget.value)}
      />
    </div>
    <p class="settings-note warn">
      风险提示：凭据以 DPAPI 加密后写入
      config.json（属次要凭据）。若泄露仅影响远程存储读写，不会暴露任何数据库内容。
    </p>
  </section>

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
          <p>“保存到本地”模式的落盘目录</p>
        </div>
      </div>
      <input
        id="remote-local-dir"
        class="settings-input setting-row-input"
        type="text"
        value={remote.localDir}
        placeholder="remote"
        oninput={(e) => change("localDir", e.currentTarget.value)}
      />
    </div>
    <p class="settings-note">
      本地副本保存在 Storage/remote/{remote.localDir || "remote"}/ 下，仅允许字母、数字、- 与 _。
    </p>
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="clock" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>本地备份数量</strong>
          <p>每次保存时保留的带时间戳备份份数，0 表示不保留</p>
        </div>
        <span class="value-label">{remote.backupCount} 份</span>
      </div>
    </div>
    <input
      type="range"
      class="transparency-slider"
      min="0"
      max="10"
      step="1"
      value={remote.backupCount}
      style:--slider-pct={sliderPercentage(remote.backupCount, 0, 10)}
      oninput={(e) => change("backupCount", Number(e.currentTarget.value))}
    />
  </section>

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
  :global(.profile-picker) {
    flex: 0 0 200px;
    width: 200px;
  }

  .profile-actions {
    display: flex;
    gap: 8px;
    margin: 10px 0 2px;
  }

  .profile-action-button {
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--hover-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    flex: 0 0 auto;
    cursor: pointer;
  }

  .profile-action-button:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }
</style>
