<script lang="ts">
  import type { ChangeTimelineEvent, HistoryDiff } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import { formatLocalDate } from "$lib/utils/date";
  import { appSettings } from "$lib/services/settings";
  import { t, type I18nKey } from "$lib/i18n";

  import Button from "$lib/components/templates/action/Button.svelte";
  interface Props {
    onclose: () => void;
    onselect?: (uuid: string) => void;
  }

  let { onclose, onselect }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);
  const dateLocale = $derived(lang === "en" ? "en-US" : "zh-CN");

  let events = $state<ChangeTimelineEvent[]>([]);
  let loading = $state(true);
  let error = $state("");
  const sessionId = vault.getActiveSessionId();

  async function refresh(): Promise<void> {
    try {
      if (!sessionId) return;
      const value = await vault.callInSession(sessionId, () => vault.changeTimeline());
      if (vault.getActiveSessionId() !== sessionId) return;
      events = value;
      loading = false;
    } catch (e) {
      if (vault.getActiveSessionId() !== sessionId) return;
      error = String(e);
      loading = false;
    }
  }

  $effect(() => {
    void refresh();
  });

  const FIELD_LABELS: Record<
    keyof Omit<HistoryDiff, "customFields" | "customData" | "attachments">,
    I18nKey
  > = {
    title: "editor.title",
    username: "tcato.username",
    password: "tcato.password",
    url: "detail.url",
    notes: "detail.notes",
    expires: "history.expired",
    hasTotp: "timeline.totp",
    icon: "editor.iconSection",
    color: "history.color",
    tags: "detail.tags",
    favorite: "history.favorite",
    qualityCheck: "editor.qualityCheck",
  };

  function diffChips(event: ChangeTimelineEvent): string[] {
    const chips: string[] = [];
    for (const [key, labelKey] of Object.entries(FIELD_LABELS)) {
      if (event.diff[key as keyof HistoryDiff] === true) chips.push(t(lang, labelKey));
    }
    for (const item of event.diff.customFields) {
      chips.push(
        t(lang, "timeline.fieldChange", {
          name: item.name,
          change:
            item.change === "added"
              ? t(lang, "timeline.added")
              : item.change === "removed"
                ? t(lang, "timeline.removed")
                : t(lang, "timeline.modified"),
        }),
      );
    }
    for (const item of event.diff.attachments) {
      chips.push(
        t(lang, "timeline.attachmentChange", {
          name: item.name,
          change:
            item.change === "added"
              ? t(lang, "timeline.added")
              : item.change === "removed"
                ? t(lang, "timeline.removed")
                : t(lang, "timeline.modified"),
        }),
      );
    }
    return chips;
  }

  function timeOfDay(iso: string): string {
    const d = new Date(iso);
    if (Number.isNaN(d.getTime())) return iso;
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}:${String(d.getSeconds()).padStart(2, "0")}`;
  }

  interface DayGroup {
    day: string;
    items: ChangeTimelineEvent[];
  }

  const dayGroups = $derived.by<DayGroup[]>(() => {
    const groups: DayGroup[] = [];
    for (const event of events) {
      const day = formatLocalDate(event.time, dateLocale);
      const last = groups[groups.length - 1];
      if (last && last.day === day) last.items.push(event);
      else groups.push({ day, items: [event] });
    }
    return groups;
  });
</script>

<ModalShell
  title={t(lang, "timeline.title")}
  description={t(lang, "timeline.desc")}
  size="large"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    {#if loading}
      <p class="note">{t(lang, "timeline.loading")}</p>
    {:else if error}
      <p class="note error">{error}</p>
    {:else if events.length === 0}
      <p class="note">{t(lang, "timeline.empty")}</p>
    {:else}
      <p class="note">{t(lang, "timeline.count", { count: events.length })}</p>
      {#each dayGroups as group (group.day)}
        <div class="day-label">{group.day}</div>
        <ul class="list">
          {#each group.items as event (event.uuid + event.time)}
            <li class="row">
              <button
                type="button"
                class="main"
                onclick={() => onselect?.(event.uuid)}
                title={t(lang, "hibp.locateEntry")}
              >
                <span class="when">{timeOfDay(event.time)}</span>
                <span class="body">
                  <span class="title">{event.title}</span>
                  <span class="sub">{event.username}</span>
                </span>
                <span class="chips">
                  {#each diffChips(event) as chip (chip)}
                    <span class="chip">{chip}</span>
                  {/each}
                </span>
              </button>
            </li>
          {/each}
        </ul>
      {/each}
    {/if}
  {/snippet}
  {#snippet actions()}
    <Button variant="primary" onclick={onclose}>{t(lang, "common.close")}</Button>
  {/snippet}
</ModalShell>

<style>
  .note {
    margin: 8px 0;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
  }

  .note.error {
    color: var(--danger-color);
  }

  .day-label {
    margin: 10px 0 4px;
    color: var(--text-muted);
    font-size: var(--font-size-tiny, 10px);
    text-transform: uppercase;
    letter-spacing: 0.05em;
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
    padding: 5px 10px;
    border: none;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .main:hover {
    background: var(--hover-bg);
  }

  .when {
    flex: none;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }

  .body {
    display: flex;
    flex: 0 1 auto;
    min-width: 0;
    flex-direction: column;
    gap: 1px;
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

  .chips {
    display: flex;
    flex: 1 1 auto;
    gap: 4px;
    justify-content: flex-end;
    min-width: 0;
    overflow: hidden;
    flex-wrap: wrap;
  }

  .chip {
    flex: none;
    max-width: 160px;
    overflow: hidden;
    padding: 1px 6px;
    border: 1px solid var(--border-subtle);
    border-radius: 999px;
    color: var(--text-secondary);
    background: var(--input-bg);
    font-size: var(--font-size-tiny, 10px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
