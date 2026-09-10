<script lang="ts">
  import SettingsDialog from "$lib/components/SettingsDialog.svelte";
  import { goto } from "$app/navigation";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  function handleClose(): void {
    void goto("/");
  }
</script>

<svelte:head>
  <title>{t(lang, "page.settingsTitle")}</title>
</svelte:head>

<div class="settings-shell">
  <SettingsDialog onclose={handleClose} />
</div>

<style>
  .settings-shell {
    width: 100%;
    height: 100%;
  }

  :global(html),
  :global(body) {
    overflow: hidden;
    background: var(--bg-settings, #1b1b1b);
  }
</style>
