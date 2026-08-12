<script lang="ts">
  import type { SecurityReport, VaultEntry } from "$lib/types/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import { entropyLabel } from "$lib/utils/password";

  interface EntryRow {
    entry: VaultEntry;
    path: string;
  }

  interface Props {
    report: SecurityReport;
    entries: EntryRow[];
    onclose: () => void;
  }

  let { report, entries, onclose }: Props = $props();

  const byUuid = $derived(new Map(entries.map((row) => [row.entry.uuid, row])));

  const totals = $derived({
    entries: report.total,
    empty: report.empty.length,
    dupes: report.duplicates.length,
    weak: report.weak.length,
  });

  function titleOf(uuid: string): string {
    return byUuid.get(uuid)?.entry.title || "未命名条目";
  }

  function pathOf(uuid: string): string {
    return byUuid.get(uuid)?.path ?? "";
  }
</script>

<ModalShell
  title="安全报告"
  description="基于当前数据库的服务端分析，不发送任何数据"
  size="report"
  showClose
  closeOnEscape
  {onclose}
>
  {#snippet icon()}<AppIcon name="shield" size={18} />{/snippet}
  {#snippet children()}
    <div class="summary-row">
      <span class="summary-chip"><b>{totals.entries}</b>总条目</span>
      <span class="summary-chip" class:issue={totals.empty > 0}><b>{totals.empty}</b>空密码</span>
      <span class="summary-chip" class:issue={totals.dupes > 0}><b>{totals.dupes}</b>重复密码</span>
      <span class="summary-chip" class:issue={totals.weak > 0}><b>{totals.weak}</b>弱密码</span>
    </div>

    {#if totals.empty === 0 && totals.dupes === 0 && totals.weak === 0}
      <p class="all-clear">未发现安全问题</p>
    {/if}

    {#if report.empty.length > 0}
      <section class="report-section">
        <h2 class="section-title">空密码</h2>
        <ul class="issue-list">
          {#each report.empty as uuid (uuid)}
            <li class="issue-row">
              <AppIcon name="key" size={12} />
              <span class="issue-title">{titleOf(uuid)}</span>
              <span class="issue-path" title={pathOf(uuid)}>{pathOf(uuid)}</span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if report.duplicates.length > 0}
      <section class="report-section">
        <h2 class="section-title">重复密码</h2>
        <ul class="issue-list">
          {#each report.duplicates as dup (dup.uuids.join("-"))}
            <li class="issue-row">
              <AppIcon name="copy" size={12} />
              <span class="issue-title">{dup.uuids.map(titleOf).join("、")}</span>
              <span class="issue-count">{dup.count} 个条目</span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}

    {#if report.weak.length > 0}
      <section class="report-section">
        <h2 class="section-title">弱密码</h2>
        <ul class="issue-list">
          {#each report.weak as item (item.uuid)}
            <li class="issue-row">
              <AppIcon name="key" size={12} />
              <span class="issue-title">{titleOf(item.uuid)}</span>
              <span class="strength-label {entropyLabel(item.bits).className}"
                >{entropyLabel(item.bits).label}</span
              >
              <span class="issue-path" title={pathOf(item.uuid)}>{pathOf(item.uuid)}</span>
            </li>
          {/each}
        </ul>
      </section>
    {/if}
  {/snippet}
</ModalShell>

<style>
  .summary-row {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-bottom: 14px;
  }

  .summary-chip {
    display: inline-flex;
    align-items: baseline;
    gap: 5px;
    padding: 4px 10px;
    border: 1px solid var(--border-subtle);
    border-radius: 12px;
    color: var(--text-muted);
    background: var(--card-bg);
    font-size: var(--font-size-tiny, 10px);
  }

  .summary-chip b {
    color: var(--text-primary);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }

  .summary-chip.issue {
    border-color: color-mix(in srgb, var(--warning-color) 45%, transparent);
  }

  .summary-chip.issue b {
    color: var(--warning-color);
  }

  .all-clear {
    margin: 8px 0 0;
    color: var(--text-muted);
    font-size: 12px;
  }

  .report-section {
    margin-top: 12px;
  }

  .section-title {
    margin: 0 0 6px;
    color: var(--text-secondary);
    font-size: 11px;
    font-weight: 560;
  }

  .issue-list {
    max-height: 220px;
    margin: 0;
    padding: 0;
    overflow: auto;
    list-style: none;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .issue-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    padding: 6px 8px;
    border-bottom: 1px solid var(--border-subtle);
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
  }

  .issue-row:last-child {
    border-bottom: 0;
  }

  .issue-title {
    overflow: hidden;
    color: var(--text-primary);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .issue-path {
    overflow: hidden;
    margin-left: auto;
    color: var(--text-faint);
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 200px;
  }

  .issue-count {
    flex: 0 0 auto;
    color: var(--warning-color);
    font-variant-numeric: tabular-nums;
  }

  .strength-label {
    flex: 0 0 auto;
    padding: 1px 7px;
    border-radius: 9px;
    font-size: var(--font-size-tiny, 10px);
  }

  .strength-label.weak {
    color: var(--danger-color);
    background: color-mix(in srgb, var(--danger-color) 12%, transparent);
  }

  .strength-label.fair {
    color: var(--warning-color);
    background: color-mix(in srgb, var(--warning-color) 12%, transparent);
  }
</style>
