<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type {
    DatabaseSettings,
    DatabaseSettingsPatch,
    WritableDatabaseCipher,
  } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import { KeyedViewGuard, sessionResourceKey } from "$lib/utils/session-state";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import Toggle from "$lib/components/templates/form/Toggle.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  let settings = $state<DatabaseSettings | null>(null);
  let loading = $state(true);
  let error = $state("");
  let saving = $state(false);

  let kdf = $state<"Aes" | "Argon2" | "Argon2id">("Aes");
  let cipher = $state<WritableDatabaseCipher | null>(null);
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
      error = t(lang, "dbsettings.noVault");
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
          error = t(lang, "dbsettings.browserUnsupported");
          return;
        }
        settings = value;
        kdf = value.kdf;
        cipher = value.cipher === "Twofish" ? null : value.cipher;
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
        (settings.cipher === "Twofish" ? cipher !== null : cipher !== settings.cipher) ||
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
      if (cipher !== null && cipher !== settings.cipher) patch.cipher = cipher;
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
  title={t(lang, "dbsettings.title")}
  description={t(lang, "dbsettings.desc")}
  size="medium"
  scrollable
  closeOnEscape={!saving}
  {onclose}
>
  {#snippet children()}
    {#if loading}
      <p class="dialog-hint">{t(lang, "dbsettings.loading")}</p>
    {:else if settings}
      <div class="setting-block">
        <span class="setting-label">{t(lang, "database.kdf")}</span>
        <div class="choice-row" role="radiogroup" aria-label={t(lang, "dbsettings.kdfAria")}>
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
        <span class="setting-label">{t(lang, "database.cipher")}</span>
        {#if settings.cipher === "Twofish"}
          <span class="setting-label">
            {t(lang, "dbsettings.twofishNote")}
          </span>
        {/if}
        <div class="choice-row" role="radiogroup" aria-label={t(lang, "dbsettings.cipherAria")}>
          {#if settings.cipher === "Twofish"}
            <button
              type="button"
              class="choice-option"
              class:active={cipher === null}
              onclick={() => (cipher = null)}
            >
              {t(lang, "dbsettings.keepTwofish")}
            </button>
          {/if}
          {#each ["Aes256", "ChaCha20"] as const as value (value)}
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
        <span class="setting-label">{t(lang, "database.compression")}</span>
        <div
          class="choice-row"
          role="radiogroup"
          aria-label={t(lang, "dbsettings.compressionAria")}
        >
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
        <span class="setting-label">{t(lang, "dbsettings.historyMax")}</span>
        <TextField
          numeric
          type="number"
          bind:value={historyInput}
          placeholder={t(lang, "dbsettings.default")}
        />
      </div>
      <div class="setting-block">
        <span class="setting-label">{t(lang, "dbsettings.historySize")}</span>
        <TextField
          numeric
          type="number"
          bind:value={historySizeInput}
          placeholder={t(lang, "dbsettings.default")}
        />
      </div>
      <div class="setting-block">
        <span class="setting-label">{t(lang, "dbsettings.templateGroup")}</span>
        <TextField
          mono
          bind:value={templateGroupInput}
          placeholder={t(lang, "dbsettings.templateGroupPh")}
        />
      </div>
      <div class="setting-block setting-row">
        <span class="setting-label">{t(lang, "dbsettings.recycleBin")}</span>
        <Toggle bind:checked={recycleEnabled} ariaLabel={t(lang, "dbsettings.recycleBin")} />
      </div>
    {:else}
      <p class="dialog-hint">{error || t(lang, "dbsettings.unreadable")}</p>
    {/if}
    {#if error && settings}<p class="dialog-error">{error}</p>{/if}
  {/snippet}
  {#snippet actions()}
    <Button onclick={onclose} disabled={saving}>{t(lang, "common.cancel")}</Button>
    <Button variant="primary" onclick={() => void save()} disabled={saving || !settings || !dirty}>
      {saving ? t(lang, "dbsettings.saving") : t(lang, "common.save")}
    </Button>
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
