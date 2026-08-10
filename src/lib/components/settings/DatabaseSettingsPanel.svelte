<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import type { Cipher, Compression, Kdf } from "$lib/types/settings";
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

  const gen = $derived(s.database.generator);

  function setKdf(kdf: Kdf): void {
    appSettings.updateDatabase("kdf", kdf);
  }

  function toggleGenerator(key: keyof typeof gen): void {
    appSettings.updateDatabase("generator", { ...gen, [key]: !gen[key] });
  }
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · 数据库</span>
      <h2>数据库</h2>
      <p>新建数据库的加密默认值与密码生成规则。</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label="关闭">×</button>
  </header>
{/if}

<div class="settings-scroll">
  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="shield" size={17} /></span>
      <div>
        <strong>密钥派生函数</strong>
        <p>新建数据库默认 KDF，影响抗暴力破解强度</p>
      </div>
    </div>
    <div class="kdf-segmented" role="group" aria-label="KDF">
      {#each ["Argon2id", "Argon2", "Aes"] as const as kdf}
        <button
          class="kdf-segment"
          class:active={s.database.kdf === kdf}
          onclick={() => setKdf(kdf)}
        >
          {kdf}
        </button>
      {/each}
    </div>
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="database" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>加密算法</strong>
          <p>数据库对称加密标准</p>
        </div>
        <Select
          value={s.database.cipher}
          options={[
            { value: "Aes256", label: "AES-256" },
            { value: "ChaCha20", label: "ChaCha20" },
          ]}
          onchange={(v) => appSettings.updateDatabase("cipher", v as Cipher)}
        />
      </div>
    </div>
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="file" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>压缩</strong>
          <p>数据库内容压缩方式</p>
        </div>
        <Select
          value={s.database.compression}
          options={[
            { value: "None", label: "不压缩" },
            { value: "Gzip", label: "Gzip" },
          ]}
          onchange={(v) => appSettings.updateDatabase("compression", v as Compression)}
        />
      </div>
    </div>
  </section>

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="save" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>文件后缀</strong>
          <p>另存为的默认文件后缀；备份文件无后缀时也使用它</p>
        </div>
        <input
          class="extension-input"
          type="text"
          value={s.database.fileExtension}
          placeholder="kdbx"
          oninput={(e) => appSettings.updateDatabase("fileExtension", e.currentTarget.value)}
        />
      </div>
    </div>
  </section>

  <SettingRangeCard
    icon="key"
    label="默认密码长度"
    description="生成器默认生成的密码长度"
    value={s.database.generator.length}
    valueLabel={`${s.database.generator.length} 位`}
    min={8}
    max={64}
    onchange={(value) => appSettings.updateDatabase("generator", { ...gen, length: value })}
  />

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="sliders" size={17} /></span>
      <div>
        <strong>字符集</strong>
        <p>密码生成器启用的字符类别</p>
      </div>
    </div>
    <div class="charset-grid">
      <button
        class="charset-chip"
        class:active={gen.includeUpper}
        onclick={() => toggleGenerator("includeUpper")}
      >
        A–Z
      </button>
      <button
        class="charset-chip"
        class:active={gen.includeLower}
        onclick={() => toggleGenerator("includeLower")}
      >
        a–z
      </button>
      <button
        class="charset-chip"
        class:active={gen.includeDigits}
        onclick={() => toggleGenerator("includeDigits")}
      >
        0–9
      </button>
      <button
        class="charset-chip"
        class:active={gen.includeSymbols}
        onclick={() => toggleGenerator("includeSymbols")}
      >
        !@#
      </button>
      <button
        class="charset-chip"
        class:active={gen.excludeSimilar}
        onclick={() => toggleGenerator("excludeSimilar")}
      >
        排除相似
      </button>
      <button
        class="charset-chip"
        class:active={gen.excludeAmbiguous}
        onclick={() => toggleGenerator("excludeAmbiguous")}
      >
        排除易混
      </button>
    </div>
  </section>

  <p class="auto-save-note">修改即时生效并自动保存</p>
</div>

<style>
  .kdf-segmented {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
    margin-top: 10px;
  }

  .kdf-segment {
    height: 30px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--input-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    font-variant-numeric: tabular-nums;
    cursor: pointer;
  }

  .kdf-segment.active {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--input-bg));
  }

  .charset-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
    margin-top: 10px;
  }

  .charset-chip {
    height: 30px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    font-variant-numeric: tabular-nums;
    cursor: pointer;
  }

  .extension-input {
    width: 110px;
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
    color: var(--text-primary);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .extension-input:focus {
    outline: none;
    border-color: var(--selection-color);
  }

  .charset-chip.active {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--input-bg));
  }
</style>
