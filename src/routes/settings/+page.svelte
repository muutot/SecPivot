<script lang="ts">
  import SettingsDialog from "$lib/components/SettingsDialog.svelte";
  import { goto } from "$app/navigation";

  function handleClose(): void {
    void goto("/");
  }
</script>

<svelte:head>
  <title>KeyVault · 设置</title>
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
