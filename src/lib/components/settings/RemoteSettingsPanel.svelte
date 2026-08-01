<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import type { RemoteSettings } from "$lib/types/settings";
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
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="cloud" size={17} /></span>
      <div>
        <strong>连接</strong>
        <p>兼容 AWS S3、MinIO 等 S3 API 服务</p>
      </div>
    </div>
    <label class="field-label" for="remote-endpoint">服务地址</label>
    <input
      id="remote-endpoint"
      class="settings-input"
      type="text"
      value={remote.endpoint}
      placeholder="https://s3.amazonaws.com"
      oninput={(e) => change("endpoint", e.currentTarget.value)}
    />
    <div class="field-row">
      <div>
        <label class="field-label" for="remote-region">区域</label>
        <input
          id="remote-region"
          class="settings-input"
          type="text"
          value={remote.region}
          placeholder="us-east-1"
          oninput={(e) => change("region", e.currentTarget.value)}
        />
      </div>
      <div>
        <label class="field-label" for="remote-bucket">存储桶</label>
        <input
          id="remote-bucket"
          class="settings-input"
          type="text"
          value={remote.bucket}
          placeholder="my-bucket"
          oninput={(e) => change("bucket", e.currentTarget.value)}
        />
      </div>
    </div>
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="key" size={17} /></span>
      <div>
        <strong>访问凭据</strong>
        <p>明文保存在 config.json，仅用于访问远程存储，与主密码无关</p>
      </div>
    </div>
    <label class="field-label" for="remote-access-key">Access Key</label>
    <input
      id="remote-access-key"
      class="settings-input"
      type="text"
      autocomplete="off"
      spellcheck="false"
      value={remote.accessKey}
      placeholder="AKIA..."
      oninput={(e) => change("accessKey", e.currentTarget.value)}
    />
    <label class="field-label" for="remote-secret-key">Secret Key</label>
    <input
      id="remote-secret-key"
      class="settings-input"
      type="password"
      autocomplete="off"
      spellcheck="false"
      value={remote.secretKey}
      placeholder="••••••••"
      oninput={(e) => change("secretKey", e.currentTarget.value)}
    />
    <p class="field-note">
      风险提示：凭据以明文写入
      config.json（属次要凭据）。若泄露仅影响远程存储读写，不会暴露任何数据库内容。
    </p>
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="folder" size={17} /></span>
      <div>
        <strong>文件浏览与本地镜像</strong>
        <p>远程文件列表前缀与“保存到本地”模式的落盘目录</p>
      </div>
    </div>
    <label class="field-label" for="remote-prefix">对象前缀(可选)</label>
    <input
      id="remote-prefix"
      class="settings-input"
      type="text"
      value={remote.prefix}
      placeholder="vaults/"
      oninput={(e) => change("prefix", e.currentTarget.value)}
    />
    <label class="field-label" for="remote-local-dir">本地镜像目录</label>
    <input
      id="remote-local-dir"
      class="settings-input"
      type="text"
      value={remote.localDir}
      placeholder="remote"
      oninput={(e) => change("localDir", e.currentTarget.value)}
    />
    <p class="field-note">
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

  <p class="auto-save-note">修改即时生效并自动保存</p>
</div>

<style>
  .field-label {
    display: block;
    margin: 10px 0 5px;
    color: var(--text-muted);
    font-size: var(--settings-note-size, var(--font-size-tiny, 10px));
  }

  .field-row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .field-note {
    margin: 8px 0 0;
    color: var(--text-faint);
    font-size: var(--settings-note-size, var(--font-size-tiny, 10px));
    line-height: 1.5;
  }
</style>
