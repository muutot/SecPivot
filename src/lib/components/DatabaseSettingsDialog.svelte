<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { DatabaseSettings, DatabaseSettingsPatch } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import { KeyedViewGuard, sessionResourceKey } from "$lib/utils/session-state";
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
  let historySizeInput = $state("");
  let templateGroupInput = $state("");
  let recycleEnabled = $state(true);
  const sessionId = vault.getActiveSessionId();
  const dialogView = new KeyedViewGuard();
  dialogView.activate(sessionId ? sessionResourceKey(sessionId, "database-settings") : null);

  onDestroy(() => dialogView.activate(null));

  onMount(() => {
    if (!sessionId) {
      error = "数据库未打开";
      loading = false;
      return;
    }
    const view = dialogView.capture();
    if (!view) return;
    void vault
      .callInSession(sessionId, () => vault.getDatabaseSettings())
      .then((value) => {
        if (!dialogView.isCurrent(view)) return;
        if (!value) {
          error = "浏览器预览不支持数据库设置";
          return;
        }
        settings = value;
        kdf = value.kdf;
        cipher = value.cipher;
        compression = value.compression;
        historyInput = value.historyMaxItems === null ? "" : String(value.historyMaxItems);
        historySizeInput = value.historyMaxSize === null ? "" : String(value.historyMaxSize);
        templateGroupInput = value.entryTemplatesGroup ?? "";
        recycleEnabled = value.recycleBinEnabled;
      })
      .catch((e) => {
        if (!dialogView.isCurrent(view)) return;
        error = String(e);
      })
      .finally(() => {
        if (dialogView.isCurrent(view)) loading = false;
      });
  });

  const dirty = $derived(
    settings !== null &&
      (kdf !== settings.kdf ||
        cipher !== settings.cipher ||
        compression !== settings.compression ||
        (historyInput === "" ? null : Number(historyInput)) !== settings.historyMaxItems ||
        (historySizeInput === "" ? null : Number(historySizeInput)) !== settings.historyMaxSize ||
        (templateGroupInput.trim() || null) !== settings.entryTemplatesGroup ||
        recycleEnabled !== settings.recycleBinEnabled),
  );

  async function save(): Promise<void> {
    if (saving || !settings) return;
    const view = dialogView.capture();
    if (!view) return;
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
      if ((historySizeInput === "" ? null : Number(historySizeInput)) !== settings.historyMaxSize) {
        patch.historyMaxSize = historySizeInput === "" ? null : Number(historySizeInput);
      }
      if ((templateGroupInput.trim() || null) !== settings.entryTemplatesGroup) {
        patch.entryTemplatesGroup = templateGroupInput.trim() || null;
      }
      if (recycleEnabled !== settings.recycleBinEnabled) {
        patch.recycleBinEnabled = recycleEnabled;
      }
      if (!sessionId) return;
      await vault.callInSession(sessionId, () => vault.updateDatabaseSettings(patch));
      if (!dialogView.isCurrent(view)) return;
      onclose();
    } catch (e) {
      if (dialogView.isCurrent(view)) error = String(e);
    } finally {
      if (dialogView.isCurrent(view)) saving = false;
    }
  }
</script>

<ModalShell
  title="数据库设置"
  description="存储算法与历史策略修改会重写数据库文件"
  size="medium"
  scrollable
  closeOnEscape={!saving}
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
      <div class="setting-block">
        <span class="setting-label">历史总大小上限（字节，留空为默认）</span>
        <input
          class="text-input"
          type="number"
          min="0"
          bind:value={historySizeInput}
          placeholder="默认"
        />
      </div>
      <div class="setting-block">
        <span class="setting-label">模板分组 UUID（留空清除）</span>
        <input
          class="text-input mono"
          type="text"
          bind:value={templateGroupInput}
          placeholder="分组 UUID"
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
