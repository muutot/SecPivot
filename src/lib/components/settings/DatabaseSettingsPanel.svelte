<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import type { Cipher, Compression, Kdf, PasswordGeneratorSettings } from "$lib/types/settings";
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

  const profiles = $derived(s.database.generatorProfiles);
  let editingIndex = $state<number | null>(null);
  let draft = $state<PasswordGeneratorSettings | null>(null);

  function startAdd(): void {
    draft = {
      name: "",
      length: 20,
      includeUpper: true,
      includeLower: true,
      includeDigits: true,
      includeSymbols: false,
      excludeSimilar: false,
      excludeAmbiguous: false,
    };
    editingIndex = -1;
  }

  function startEdit(index: number): void {
    draft = { ...profiles[index] };
    editingIndex = index;
  }

  function saveDraft(): void {
    if (!draft || !draft.name?.trim()) return;
    const next = profiles.map((p) => ({ ...p }));
    if (editingIndex === -1) {
      next.push({ ...draft, name: draft.name.trim() });
    } else if (editingIndex !== null && next[editingIndex]) {
      next[editingIndex] = { ...draft, name: draft.name.trim() };
    }
    appSettings.updateDatabase("generatorProfiles", next);
    draft = null;
    editingIndex = null;
  }

  function removeProfile(index: number): void {
    appSettings.updateDatabase(
      "generatorProfiles",
      profiles.filter((_, i) => i !== index),
    );
    if (editingIndex === index) {
      draft = null;
      editingIndex = null;
    }
  }

  function setDefault(profile: PasswordGeneratorSettings): void {
    appSettings.updateDatabase("generator", { ...profile });
  }

  function updateDraft(key: keyof PasswordGeneratorSettings, value: string | number): void {
    if (draft) draft = { ...draft, [key]: value };
  }

  function toggleDraft(key: keyof PasswordGeneratorSettings): void {
    if (draft && typeof draft[key] === "boolean") {
      draft = { ...draft, [key]: !draft[key] };
    }
  }

  const isDefault = (profile: PasswordGeneratorSettings): boolean =>
    Boolean(profile.name) && s.database.generator.name === profile.name;
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

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="sliders" size={17} /></span>
      <div>
        <strong>密码配置档</strong>
        <p>命名规则可复用于新条目；「设为默认」作为新建条目的生成规则</p>
      </div>
    </div>
    {#each profiles as profile, index}
      <div class="profile-row">
        <span class="profile-name">
          {profile.name}
          {#if isDefault(profile)}<span class="profile-default">默认</span>{/if}
        </span>
        <span class="profile-length">{profile.length} 位</span>
        <button type="button" class="profile-action" onclick={() => setDefault(profile)}>
          设为默认
        </button>
        <button type="button" class="profile-action" onclick={() => startEdit(index)}>编辑</button>
        <button
          type="button"
          class="profile-action destructive"
          onclick={() => removeProfile(index)}
        >
          删除
        </button>
      </div>
    {/each}
    {#if profiles.length === 0}
      <p class="profile-empty">尚无自定义配置档</p>
    {/if}
    <button type="button" class="profile-add" onclick={startAdd}>
      <AppIcon name="plus" size={12} />新建配置档
    </button>
  </section>

  {#if draft}
    <section class="setting-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="key" size={17} /></span>
        <div>
          <strong>{editingIndex === -1 ? "新建配置档" : "编辑配置档"}</strong>
          <p>命名规则；留空的必含/排除/自定义字符集与 pattern 视为未启用</p>
        </div>
      </div>
      <div class="profile-form">
        <input
          class="profile-name-input"
          type="text"
          value={draft.name ?? ""}
          placeholder="配置名称"
          oninput={(e) => updateDraft("name", e.currentTarget.value)}
        />
        <input
          class="profile-length-input"
          type="number"
          min="8"
          max="128"
          value={draft.length}
          oninput={(e) => updateDraft("length", Number(e.currentTarget.value))}
        />
        <div class="charset-grid">
          <button
            class="charset-chip"
            class:active={draft.includeUpper}
            onclick={() => toggleDraft("includeUpper")}>A–Z</button
          >
          <button
            class="charset-chip"
            class:active={draft.includeLower}
            onclick={() => toggleDraft("includeLower")}>a–z</button
          >
          <button
            class="charset-chip"
            class:active={draft.includeDigits}
            onclick={() => toggleDraft("includeDigits")}>0–9</button
          >
          <button
            class="charset-chip"
            class:active={draft.includeSymbols}
            onclick={() => toggleDraft("includeSymbols")}>!@#</button
          >
          <button
            class="charset-chip"
            class:active={draft.excludeSimilar}
            onclick={() => toggleDraft("excludeSimilar")}>排除相似</button
          >
          <button
            class="charset-chip"
            class:active={draft.excludeAmbiguous}
            onclick={() => toggleDraft("excludeAmbiguous")}>排除易混</button
          >
        </div>
        <input
          class="profile-text"
          type="text"
          value={draft.customCharset ?? ""}
          placeholder="自定义字符集（整体替换类别）"
          oninput={(e) => updateDraft("customCharset", e.currentTarget.value)}
        />
        <input
          class="profile-text"
          type="text"
          value={draft.excludeChars ?? ""}
          placeholder="排除字符"
          oninput={(e) => updateDraft("excludeChars", e.currentTarget.value)}
        />
        <input
          class="profile-text"
          type="text"
          value={draft.requiredChars ?? ""}
          placeholder="必含字符"
          oninput={(e) => updateDraft("requiredChars", e.currentTarget.value)}
        />
        <input
          class="profile-text mono"
          type="text"
          value={draft.pattern ?? ""}
          placeholder="pattern（u/l/d/s/a，其他为字面量）"
          oninput={(e) => updateDraft("pattern", e.currentTarget.value)}
        />
        <div class="profile-actions">
          <button
            type="button"
            class="profile-action"
            onclick={() => ((draft = null), (editingIndex = null))}
          >
            取消
          </button>
          <button type="button" class="profile-action primary" onclick={saveDraft}>
            保存配置档
          </button>
        </div>
      </div>
    </section>
  {/if}

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

  .profile-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .profile-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--text-primary);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .profile-default {
    margin-left: 6px;
    padding: 1px 6px;
    border-radius: 999px;
    color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 14%, transparent);
    font-size: 9px;
  }

  .profile-length {
    flex: 0 0 auto;
    color: var(--text-faint);
    font-size: var(--settings-note-size, var(--font-size-tiny, 10px));
    font-variant-numeric: tabular-nums;
  }

  .profile-action {
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: transparent;
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    cursor: pointer;
  }

  .profile-action:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .profile-action.destructive {
    color: var(--danger-color);
  }

  .profile-action.primary {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 16%, var(--card-bg));
  }

  .profile-empty {
    margin: 10px 0 0;
    color: var(--text-faint);
    font-size: var(--settings-note-size, var(--font-size-tiny, 10px));
  }

  .profile-add {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    margin-top: 10px;
    height: 28px;
    padding: 0 12px;
    border: 1px dashed var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: transparent;
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    cursor: pointer;
  }

  .profile-add:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .profile-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 10px;
  }

  .profile-name-input,
  .profile-length-input,
  .profile-text {
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
  }

  .profile-length-input {
    width: 90px;
    flex: 0 0 auto;
  }

  .profile-text.mono {
    font-family: var(--font-mono);
  }

  .profile-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }
</style>
