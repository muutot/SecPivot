<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime, isMobile } from "$lib/services/settings";
  import SettingToggleCard from "$lib/components/settings/SettingToggleCard.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import Feedback from "$lib/components/templates/form/Feedback.svelte";
  import { t } from "$lib/i18n";
  const current = $derived($appSettings.localBackup);
  const lang = $derived($appSettings.general.language);
  const initial = untrack(() => current);
  let savedCount = $state(initial.count);
  let savedTemplate = $state(initial.template);
  let count = $state(String(initial.count));
  let template = $state(initial.template);
  let busy = $state(false);
  let error = $state("");
  let generation = 0;
  $effect(() => {
    if (current.count !== savedCount || current.template !== savedTemplate) {
      savedCount = current.count;
      savedTemplate = current.template;
      count = String(current.count);
      template = current.template;
      error = "";
      busy = false;
      generation++;
    }
  });
  onDestroy(() => {
    generation++;
  });
  async function apply(): Promise<void> {
    if (busy || !isTauriRuntime() || isMobile()) return;
    const amount = Number(count);
    if (!Number.isInteger(amount) || amount < 1 || amount > 50) {
      error = t(lang, "database.backupCountInvalid");
      return;
    }
    const run = ++generation;
    busy = true;
    error = "";
    try {
      const valid = await invoke<string>("validate_local_backup_template", { template });
      if (run !== generation) return;
      savedCount = amount;
      savedTemplate = valid;
      count = String(amount);
      template = valid;
      appSettings.updateLocalBackup({ count: amount, template: valid });
      await appSettings.flush();
    } catch (failure) {
      if (run === generation) error = String(failure);
    } finally {
      if (run === generation) busy = false;
    }
  }
</script>

<SettingToggleCard
  icon="save"
  label={t(lang, "database.localBackup")}
  description={t(lang, "database.localBackupDesc")}
  checked={current.enabled}
  disabled={busy || isMobile() || !isTauriRuntime()}
  onchange={(enabled) => appSettings.updateLocalBackup({ enabled })}
/>
<section class="setting-card">
  <p class="settings-note backup-location">{t(lang, "database.backupLocationDesc")}</p>
  <div class="backup-count">
    <label class="settings-label" for="local-backup-count">{t(lang, "database.backupCount")}</label>
    <div class="backup-number">
      <TextField
        id="local-backup-count"
        type="number"
        size="control"
        numeric
        min={1}
        max={50}
        step={1}
        bind:value={count}
        disabled={busy || isMobile()}
        oninput={() => (error = "")}
      />
    </div>
  </div>
  <label class="settings-label backup-template-label" for="local-backup-template"
    >{t(lang, "database.backupTemplate")}</label
  >
  <TextField
    id="local-backup-template"
    size="control"
    mono
    bind:value={template}
    disabled={busy || isMobile()}
    oninput={() => (error = "")}
  />
  <div class="backup-actions">
    <span class="settings-note">{t(lang, "database.backupTemplateDesc")}</span>
    <Button
      variant="action"
      {busy}
      disabled={busy ||
        isMobile() ||
        !isTauriRuntime() ||
        (!error && Number(count) === current.count && template === current.template)}
      onclick={() => void apply()}>{t(lang, "common.apply")}</Button
    >
  </div>
  {#if error}<Feedback inline message={error} />{/if}
</section>

<style>
  .backup-location {
    margin: 0 0 8px;
  }
  .backup-count,
  .backup-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .backup-number {
    width: 96px;
    flex-shrink: 0;
  }
  .backup-template-label {
    display: block;
    margin: 8px 0 6px;
  }
  .backup-actions {
    margin-top: 8px;
  }
  .settings-note {
    min-width: 0;
    overflow-wrap: anywhere;
  }
</style>
