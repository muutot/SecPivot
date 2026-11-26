<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import Feedback from "$lib/components/templates/form/Feedback.svelte";
  import { t } from "$lib/i18n";

  interface Props {
    setting: "masterKeyExpiryRec" | "masterKeyExpiryForce";
  }
  let { setting }: Props = $props();
  const lang = $derived($appSettings.general.language);
  const required = $derived(setting === "masterKeyExpiryForce");
  const current = $derived($appSettings.security[setting]);
  const initial = untrack(() => current);
  let saved = $state(initial);
  let value = $state(initial);
  let busy = $state(false);
  let error = $state("");
  let generation = 0;
  $effect(() => {
    if (current !== saved) {
      saved = current;
      value = current;
      error = "";
      busy = false;
      generation++;
    }
  });
  onDestroy(() => {
    generation++;
  });

  async function applyRule(): Promise<void> {
    if (busy || !isTauriRuntime()) return;
    const run = ++generation;
    const rule = value;
    const key = setting;
    busy = true;
    error = "";
    try {
      const valid = await invoke<boolean>("validate_master_key_expiry_rule", { rule });
      if (run !== generation || key !== setting) return;
      if (!valid) {
        error = t(lang, "security.keyExpiryInvalid");
        return;
      }
      appSettings.updateSecurity(key, rule);
    } catch (failure) {
      if (run === generation) error = String(failure);
    } finally {
      if (run === generation) busy = false;
    }
  }
</script>

<section class="setting-card">
  <div class="setting-heading">
    <span class="setting-icon"><AppIcon name="key" size={17} /></span>
    <div class="heading-inline">
      <div>
        <strong>{t(lang, required ? "security.keyExpiryForce" : "security.keyExpiryRec")}</strong>
        <p>{t(lang, required ? "security.keyExpiryForceDesc" : "security.keyExpiryRecDesc")}</p>
      </div>
    </div>
  </div>
  <TextField
    size="control"
    bind:value
    disabled={busy}
    ariaLabel={t(lang, required ? "security.keyExpiryForce" : "security.keyExpiryRec")}
    placeholder="P90D / 2027-01-01T00:00:00Z"
    oninput={() => (error = "")}
  />
  <div class="rule-actions">
    <span class="settings-note">{t(lang, "security.keyExpiryFormat")}</span>
    <Button
      variant="action"
      disabled={busy || value === current || !isTauriRuntime()}
      {busy}
      onclick={() => void applyRule()}>{t(lang, "common.apply")}</Button
    >
  </div>
  {#if error}<Feedback inline message={error} />{/if}
</section>

<style>
  .rule-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 8px;
  }
  .settings-note {
    min-width: 0;
    overflow-wrap: anywhere;
  }
</style>
