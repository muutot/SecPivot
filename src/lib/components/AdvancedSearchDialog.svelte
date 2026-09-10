<script lang="ts">
  import { get } from "svelte/store";
  import type { AdvancedSearchQuery, SearchFieldScope } from "$lib/utils/entry-search";
  import type { SavedSearch } from "$lib/types/settings";
  import { appSettings } from "$lib/services/settings";
  import { t, type I18nKey } from "$lib/i18n";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";

  interface Props {
    initialQuery?: AdvancedSearchQuery | null;
    onapply: (query: AdvancedSearchQuery) => void;
    onclear: () => void;
    onclose: () => void;
  }

  let { initialQuery = null, onapply, onclear, onclose }: Props = $props();

  // The dialog is mounted per open, so the query is captured once.
  // svelte-ignore state_referenced_locally
  const captured = initialQuery;
  let text = $state(captured?.text ?? "");
  let field = $state<SearchFieldScope>(captured?.field ?? "all");
  let regex = $state(captured?.regex ?? false);
  let exclude = $state(captured?.exclude ?? false);
  let onlyExpired = $state(captured?.onlyExpired ?? false);
  let onlyFavorites = $state(captured?.onlyFavorites ?? false);
  let tags = $state(captured?.tags ?? "");
  let requireQualityCheck = $state(captured?.requireQualityCheck ?? false);

  let settings = $state(get(appSettings));
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      settings = value;
    });
    return unsubscribe;
  });
  const lang = $derived(settings.general.language);
  const savedSearches = $derived(settings.general.savedSearches);
  let saveName = $state("");

  function loadSearch(search: SavedSearch): void {
    text = search.query.text;
    field = search.query.field;
    regex = search.query.regex;
    exclude = search.query.exclude;
    onlyExpired = search.query.onlyExpired ?? false;
    onlyFavorites = search.query.onlyFavorites ?? false;
    tags = search.query.tags ?? "";
    requireQualityCheck = search.query.requireQualityCheck ?? false;
  }

  function saveSearch(): void {
    const name = saveName.trim();
    if (!name) return;
    const query: AdvancedSearchQuery = {
      text,
      field,
      regex,
      exclude,
      onlyExpired,
      onlyFavorites,
      tags,
      requireQualityCheck,
    };
    appSettings.updateGeneral("savedSearches", [...savedSearches, { name, query }]);
    saveName = "";
  }

  function deleteSearch(name: string): void {
    appSettings.updateGeneral(
      "savedSearches",
      savedSearches.filter((search) => search.name !== name),
    );
  }

  const FIELD_OPTIONS: { value: SearchFieldScope; labelKey: I18nKey }[] = [
    { value: "all", labelKey: "search.fieldAll" },
    { value: "title", labelKey: "search.fieldTitle" },
    { value: "username", labelKey: "search.fieldUsername" },
    { value: "url", labelKey: "search.fieldUrl" },
    { value: "notes", labelKey: "search.fieldNotes" },
    { value: "tags", labelKey: "search.fieldTags" },
    { value: "custom", labelKey: "search.fieldCustom" },
  ];

  function apply(): void {
    onapply({
      text,
      field,
      regex,
      exclude,
      onlyExpired,
      onlyFavorites,
      tags,
      requireQualityCheck,
    });
    onclose();
  }
</script>

<ModalShell
  title={t(lang, "search.title")}
  description={t(lang, "search.desc")}
  size="medium"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    <div class="block">
      <span class="label">{t(lang, "search.scope")}</span>
      <div class="chips" role="radiogroup" aria-label={t(lang, "search.scope")}>
        {#each FIELD_OPTIONS as option (option.value)}
          <button
            type="button"
            class="chip"
            class:active={field === option.value}
            onclick={() => (field = option.value)}
          >
            {t(lang, option.labelKey)}
          </button>
        {/each}
      </div>
    </div>
    <div class="block">
      <span class="label">{t(lang, "search.keyword")}</span>
      <TextField mono bind:value={text} placeholder={t(lang, "search.keywordPh")} />
      <div class="toggles">
        <button type="button" class="toggle" class:active={regex} onclick={() => (regex = !regex)}>
          {t(lang, "search.regex")}
        </button>
        <button
          type="button"
          class="toggle"
          class:active={exclude}
          onclick={() => (exclude = !exclude)}
        >
          {t(lang, "search.exclude")}
        </button>
      </div>
    </div>
    <div class="block">
      <span class="label">{t(lang, "search.tagsLabel")}</span>
      <TextField bind:value={tags} placeholder="work dev" />
    </div>
    <div class="block">
      <div class="toggles">
        <button
          type="button"
          class="toggle"
          class:active={onlyExpired}
          onclick={() => (onlyExpired = !onlyExpired)}
        >
          {t(lang, "search.onlyExpired")}
        </button>
        <button
          type="button"
          class="toggle"
          class:active={onlyFavorites}
          onclick={() => (onlyFavorites = !onlyFavorites)}
        >
          {t(lang, "search.onlyFavorites")}
        </button>
        <button
          type="button"
          class="toggle"
          class:active={requireQualityCheck}
          onclick={() => (requireQualityCheck = !requireQualityCheck)}
        >
          {t(lang, "search.qualityCheck")}
        </button>
      </div>
    </div>
    <div class="block">
      <span class="label">{t(lang, "search.savedTitle")}</span>
      {#each savedSearches as search (search.name)}
        <div class="saved-row">
          <span class="saved-name">{search.name}</span>
          <button type="button" class="saved-action" onclick={() => loadSearch(search)}>
            {t(lang, "search.load")}
          </button>
          <button
            type="button"
            class="saved-action destructive"
            onclick={() => deleteSearch(search.name)}
          >
            {t(lang, "search.delete")}
          </button>
        </div>
      {/each}
      {#if savedSearches.length === 0}
        <p class="saved-empty">{t(lang, "search.noSaved")}</p>
      {/if}
      <div class="save-row">
        <div class="save-field">
          <TextField
            bind:value={saveName}
            placeholder={t(lang, "search.namePh")}
            onkeydown={(event) => {
              if (event.key === "Enter") saveSearch();
            }}
          />
        </div>
        <button type="button" class="saved-action primary" onclick={saveSearch}>
          {t(lang, "search.saveCurrent")}
        </button>
      </div>
    </div>
  {/snippet}
  {#snippet actions()}
    <Button
      onclick={() => {
        onclear();
        onclose();
      }}
    >
      {t(lang, "search.clear")}
    </Button>
    <Button onclick={onclose}>{t(lang, "common.cancel")}</Button>
    <Button variant="primary" onclick={apply}>{t(lang, "search.apply")}</Button>
  {/snippet}
</ModalShell>

<style>
  .block {
    margin-bottom: 12px;
  }

  .label {
    display: block;
    margin-bottom: 6px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .chips,
  .toggles {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chips {
    margin-bottom: 12px;
  }

  .chip,
  .toggle {
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .chip:hover,
  .toggle:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .chip.active,
  .toggle.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .saved-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  .saved-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--text-primary);
    font-size: var(--font-size-secondary, 11px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .saved-action {
    height: 26px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: transparent;
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .saved-action:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .saved-action.destructive {
    color: var(--danger-color);
  }

  .saved-action.primary {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 16%, var(--card-bg));
  }

  .saved-empty {
    margin: 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .save-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 8px;
  }

  .save-field {
    flex: 1;
    min-width: 0;
  }

  .save-row .saved-action {
    flex: 0 0 auto;
    white-space: nowrap;
  }
</style>
