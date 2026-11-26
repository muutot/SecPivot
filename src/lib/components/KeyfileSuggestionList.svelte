<script lang="ts">
  import { onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings, isTauriRuntime, isMobile } from "$lib/services/settings";
  import { KeyfileSearchController, type KeyfileSuggestions } from "$lib/services/keyfile-search";
  import Select from "$lib/components/Select.svelte";
  import { t } from "$lib/i18n";
  interface Props {
    scope: string;
    busy?: boolean;
    onselect: (path: string) => void;
  }
  let { scope, busy = false, onselect }: Props = $props();
  const lang = $derived($appSettings.general.language);
  const controller = new KeyfileSearchController(async () => {
    await appSettings.flush();
    return invoke<KeyfileSuggestions>("search_key_files");
  });
  const state = controller.state;
  $effect(() => {
    controller.activate(
      isTauriRuntime() && !isMobile() && $appSettings.integration.searchKeyFiles
        ? { scope, removable: $appSettings.integration.searchKeyFilesOnRemovableMedia }
        : null,
    );
  });
  onDestroy(() => controller.dispose());
  function choose(path: string): void {
    if (!busy && controller.allows(path)) onselect(path);
  }
</script>

{#if $state.result?.supported && $state.result.paths.length > 0}
  <div class="suggestions">
    <Select
      value=""
      options={[
        { value: "", label: t(lang, "vault.keyfileSuggestions") },
        ...$state.result.paths.map((path) => ({ value: path, label: path })),
      ]}
      ariaLabel={t(lang, "vault.keyfileSuggestions")}
      disabled={busy}
      onchange={choose}
    />
  </div>
{/if}
{#if $state.busy || $state.failed || $state.result?.incomplete}
  <p class="search-note" role="status">
    {t(lang, $state.busy ? "vault.keyfileSearching" : "vault.keyfileSearchIncomplete")}
  </p>
{/if}

<style>
  .suggestions {
    display: flex;
    flex-direction: column;
    min-width: 0;
    margin-top: 6px;
  }
  .search-note {
    margin: 6px 0 0;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
    overflow-wrap: anywhere;
  }
</style>
