<script lang="ts">
  import { onMount } from "svelte";
  import type { DatabaseSettings, DatabaseSettingsPatch } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let settings = $state<DatabaseSettings | null>(null);
  let loading = $state(true);
  let error = $state("");
  let saving = $state(false);

  let kdf = $state<"Aes" | "Argon2" | "Argon2id">("Aes");
  let cipher = $state<"Aes256" | "Twofish" | "ChaCha20">("Aes256");
  let compression = $state<"None" | "Gzip">("Gzip");
  let historyInput = $state("");
  let recycleEnabled = $state(true);

  onMount(() => {
    void vault
      .getDatabaseSettings()
      .then((value) => {
        if (!value) {
          error = "浏览器预览不支持数据库设置";
          return;
        }
        settings = value;
        kdf = value.kdf;
        cipher = value.cipher;
        compression = value.compression;
        historyInput = value.historyMaxItems === null ? "" : String(value.historyMaxItems);
        recycleEnabled = value.recycleBinEnabled;
      })
      .catch((e) => {
        error = String(e);
      })
      .finally(() => {
        loading = false;
      });
  });

  const dirty = $derived(
    settings !== null &&
      (kdf !== settings.kdf ||
        cipher !== settings.cipher ||
        compression !== settings.compression ||
        (historyInput === "" ? null : Number(historyInput)) !== settings.historyMaxItems ||
        recycleEnabled !== settings.recycleBinEnabled),
  );

  async function save(): Promise<void> {
    if (saving || !settings) return;
    saving = true;
    error = "";
    try {
      const patch: DatabaseSettingsPatch = {};
      if (kdf !== settings.kdf) patch.kdf = kdf;
      if (cipher !== settings.cipher) patch.cipher = cipher;
      if (compression !== settings.compression) patch.compression = compression;
      if ((historyInput === "" ? null : Number(historyInput)) !== settings.historyMaxItems) {
        patch.historyMaxItems = historyInput === "" ? null : Number(historyInput);
      }
      if (recycleEnabled !== settings.recycleBinEnabled) {
        patch.recycleBinEnabled = recycleEnabled;
      }
      await vault.updateDatabaseSettings(patch);
      onclose();
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }
</script>

<ModalShell
  title="数据库设置"
  description="存储算法与历史策略修改会重写数据库文件"
  size="medium"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    {#if loading}
      <p class="dialog-hint">正在读取数据库设置…</p>
    {:else if settings}
      <div class="setting-block">
        <span class="setting-label">密钥派生 (KDF)</span>
        <div class="choice-row" role="radiogroup" aria-label="密钥派生算法">
          {#each ["Aes", "Argon2", "Argon2id"] as const as value (value)}
            <button
              type="button"
              class="choice-option"
              class:active={kdf === value}
              onclick={() => (kdf = value)}
            >
              {value}
            </button>
          {/each}
        </div>
      </div>
      <div class="setting-block">
        <span class="setting-label">加密算法</span>
        <div class="choice-row" role="radiogroup" aria-label="加密算法">
          {#each ["Aes256", "Twofish", "ChaCha20"] as const as value (value)}
            <button
              type="button"
              class="choice-option"
              class:active={cipher === value}
              onclick={() => (cipher = value)}
            >
              {value}
            </button>
          {/each}
        </div>
      </div>
      <div class="setting-block">
        <span class="setting-label">压缩</span>
        <div class="choice-row" role="radiogroup" aria-label="压缩算法">
          {#each ["None", "Gzip"] as const as value (value)}
            <button
              type="button"
              class="choice-option"
              class:active={compression === value}
              onclick={() => (compression = value)}
            >
              {value}
            </button>
          {/each}
        </div>
      </div>
      <div class="setting-block">
        <span class="setting-label">历史版本上限（留空为默认）</span>
        <input
          class="text-input"
          type="number"
          min="0"
          bind:value={historyInput}
          placeholder="默认"
        />
      </div>
      <div class="setting-block setting-row">
        <span class="setting-label">启用回收站</span>
        <button
          type="button"
          class="toggle-switch"
          class:active={recycleEnabled}
          role="switch"
          aria-checked={recycleEnabled}
          aria-label="启用回收站"
          onclick={() => (recycleEnabled = !recycleEnabled)}
        ></button>
      </div>
    {:else}
      <p class="dialog-hint">{error || "无法读取数据库设置"}</p>
    {/if}
    {#if error && settings}<p class="dialog-error">{error}</p>{/if}
  {/snippet}
  {#snippet actions()}
    <button class="modal-button" onclick={onclose} disabled={saving}>取消</button>
    <button
      class="modal-button primary"
      onclick={() => void save()}
      disabled={saving || !settings || !dirty}
    >
      {saving ? "保存中…" : "保存"}
    </button>
  {/snippet}
</ModalShell>

<style>
  .setting-block {
    margin-bottom: 14px;
  }

  .setting-label {
    display: block;
    margin-bottom: 6px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .setting-row .setting-label {
    margin-bottom: 0;
  }

  .choice-row {
    display: flex;
    gap: 6px;
  }

  .choice-option {
    flex: 1;
    height: 30px;
    padding: 0 8px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .choice-option:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .choice-option.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .dialog-hint {
    margin: 0;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
  }

  .dialog-error {
    margin: 10px 0 0;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
  }
</style>
