<script lang="ts">
  import type { ChangeTimelineEvent, HistoryDiff } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import { formatLocalDate } from "$lib/utils/date";

  import Button from "$lib/components/templates/action/Button.svelte";
  interface Props {
    onclose: () => void;
    onselect?: (uuid: string) => void;
  }

  let { onclose, onselect }: Props = $props();

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
    string
  > = {
    title: "标题",
    username: "用户名",
    password: "密码",
    url: "网址",
    notes: "笔记",
    expires: "过期",
    hasTotp: "TOTP",
    icon: "图标",
    color: "颜色",
    tags: "标签",
    favorite: "收藏",
    qualityCheck: "质量检查",
  };

  function diffChips(event: ChangeTimelineEvent): string[] {
    const chips: string[] = [];
    for (const [key, label] of Object.entries(FIELD_LABELS)) {
      if (event.diff[key as keyof HistoryDiff] === true) chips.push(label);
    }
    for (const item of event.diff.customFields) {
      chips.push(
        `字段 ${item.name}${item.change === "added" ? " 新增" : item.change === "removed" ? " 移除" : " 修改"}`,
      );
    }
    for (const item of event.diff.attachments) {
      chips.push(
        `附件 ${item.name}${item.change === "added" ? " 新增" : item.change === "removed" ? " 移除" : " 修改"}`,
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
      const day = formatLocalDate(event.time);
      const last = groups[groups.length - 1];
      if (last && last.day === day) last.items.push(event);
      else groups.push({ day, items: [event] });
    }
    return groups;
  });
</script>

<ModalShell
  title="变更时间线"
  description="全库修改历史（按时间倒序，回收站条目不参与）"
  size="large"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    {#if loading}
      <p class="note">正在加载…</p>
    {:else if error}
      <p class="note error">{error}</p>
    {:else if events.length === 0}
      <p class="note">暂无变更记录。</p>
    {:else}
      <p class="note">共 {events.length} 条变更：</p>
      {#each dayGroups as group (group.day)}
        <div class="day-label">{group.day}</div>
        <ul class="list">
          {#each group.items as event (event.uuid + event.time)}
            <li class="row">
              <button
                type="button"
                class="main"
                onclick={() => onselect?.(event.uuid)}
                title="定位条目"
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
    <Button variant="primary" onclick={onclose}>关闭</Button>
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
