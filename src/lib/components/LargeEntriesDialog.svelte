<script lang="ts">
  /** KeePass "Find → Large Entries": every entry outside the recycle bin,
   *  listed largest first (the official command sorts by size descending),
   *  with an optional minimum-size filter. Sizes already ship in the vault
   *  snapshot (`VaultEntry.size`, KeePass `GetSize()` semantics), so this is
   *  view-only — no extra IPC. The parent passes the entries plus a group-path
   *  lookup; the dialog never calls the vault itself. */
  import type { VaultEntry } from "$lib/types/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";

  import Button from "$lib/components/templates/action/Button.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  import { formatKeePassSize } from "$lib/utils/format";

  interface Props {
    entries: VaultEntry[];
    /** Group uuid → "Group / Subgroup" path for the location column. */
    pathOf: (groupUuid: string) => string;
    onclose: () => void;
    onselect?: (uuid: string) => void;
  }

  let { entries, pathOf, onclose, onselect }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  /** Minimum size in KB as text; empty/non-numeric means 0 (show all). */
  let minKbText = $state("0");

  const largeEntries = $derived.by(() => {
    const min = Number(minKbText);
    const minBytes = Number.isFinite(min) && min > 0 ? min * 1024 : 0;
    return entries
      .filter((e) => (e.size ?? 0) >= minBytes)
      .sort((a, b) => (b.size ?? 0) - (a.size ?? 0));
  });

  /** Sum of the filtered entries' sizes, for the count line. */
  const totalBytes = $derived(largeEntries.reduce((sum, e) => sum + (e.size ?? 0), 0));
</script>

<ModalShell
  title={t(lang, "large.title")}
  description={t(lang, "large.desc")}
  size="large"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    <div class="filter-row">
      <label class="filter-label" for="large-min-kb">{t(lang, "large.minSize")}</label>
      <input
        id="large-min-kb"
        class="filter-input"
        type="number"
        min="0"
        step="1"
        bind:value={minKbText}
      />
      <span class="filter-unit">KB</span>
    </div>
    {#if entries.length === 0}
      <p class="note">{t(lang, "large.empty")}</p>
    {:else if largeEntries.length === 0}
      <p class="note">{t(lang, "large.filteredEmpty")}</p>
    {:else}
      <p class="note">
        {t(lang, "large.count", { count: largeEntries.length })}
        · {t(lang, "large.total", { size: formatKeePassSize(totalBytes) })}
      </p>
      <ul class="list">
        {#each largeEntries as entry (entry.uuid)}
          <li class="row">
            <button
              type="button"
              class="main"
              onclick={() => onselect?.(entry.uuid)}
              title={t(lang, "hibp.locateEntry")}
            >
              <span class="size">{formatKeePassSize(entry.size ?? 0)}</span>
              <span class="meta">
                <span class="title">{entry.title}</span>
                <span class="sub">{pathOf(entry.groupUuid)}</span>
              </span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  {/snippet}
  {#snippet actions()}
    <Button variant="primary" onclick={onclose}>{t(lang, "common.close")}</Button>
  {/snippet}
</ModalShell>

<style>
  .filter-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 10px;
  }

  .filter-label {
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .filter-input {
    width: 90px;
    height: 26px;
    padding: 0 8px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
  }

  .filter-input:focus {
    outline: none;
    border-color: var(--selection-color);
  }

  .filter-unit {
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .note {
    margin: 8px 0;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
  }

  .list {
    margin: 0;
    padding: 0;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    list-style: none;
  }

  .row {
    border-bottom: 1px solid var(--border-subtle);
  }

  .row:last-child {
    border-bottom: none;
  }

  .main {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 6px 10px;
    border: none;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .main:hover {
    background: var(--hover-bg);
  }

  .size {
    flex: none;
    min-width: 64px;
    color: var(--text-secondary);
    font-family: var(--font-mono);
    font-size: var(--font-size-tiny, 10px);
    text-align: right;
  }

  .meta {
    display: flex;
    flex: 1;
    min-width: 0;
    flex-direction: column;
    gap: 2px;
  }

  .title {
    overflow: hidden;
    color: var(--text-primary);
    font-size: var(--font-size-secondary, 11px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    overflow: hidden;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
