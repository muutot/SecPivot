<script lang="ts">
  import { onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import SettingToggleCard from "$lib/components/settings/SettingToggleCard.svelte";
  import Feedback from "$lib/components/templates/form/Feedback.svelte";
  import { t } from "$lib/i18n";
  interface Status {
    enabled: boolean;
    active: boolean;
    path: string | null;
    limitReached: boolean;
  }
  const enabled = $derived($appSettings.logging.enabled);
  const lang = $derived($appSettings.general.language);
  let status = $state<Status | null>(null);
  let pending = $state(false);
  let unavailable = $state(false);
  let generation = 0;
  async function refresh(requested: boolean, flushConfig = true): Promise<void> {
    const run = ++generation;
    if (flushConfig) {
      pending = true;
      unavailable = false;
      status = null;
    }
    try {
      if (flushConfig) await appSettings.flush();
      const result = await invoke<Status>("diagnostic_log_status");
      if (run !== generation || requested !== enabled) return;
      status = result;
      unavailable = requested && !result.active;
    } catch {
      if (run === generation) unavailable = requested;
    } finally {
      if (run === generation) pending = false;
    }
  }
  $effect(() => {
    if (!isTauriRuntime()) return;
    const requested = enabled;
    void refresh(requested);
    if (!requested) return;
    const timer = setInterval(() => void refresh(requested, false), 5000);
    return () => clearInterval(timer);
  });
  onDestroy(() => {
    generation++;
  });
</script>

<SettingToggleCard
  icon="file"
  label={t(lang, "settings.logging")}
  description={t(lang, "settings.loggingDesc")}
  checked={enabled}
  disabled={pending || !isTauriRuntime()}
  onchange={(checked) => appSettings.updateLogging(checked)}
/>
{#if enabled && status?.path}<p class="log-path">{status.path}</p>{/if}
{#if enabled && unavailable}<Feedback
    inline
    message={t(
      lang,
      status?.limitReached ? "settings.loggingLimit" : "settings.loggingUnavailable",
    )}
  />{/if}

<style>
  .log-path {
    margin: -2px 0 8px;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    overflow-wrap: anywhere;
  }
</style>
