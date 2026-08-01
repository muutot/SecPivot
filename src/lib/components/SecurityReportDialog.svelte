<script lang="ts">
  import type { SecurityReport, VaultEntry } from "$lib/types/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
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

<div class="modal-backdrop" role="presentation">
  <div class="report-modal" role="dialog" aria-modal="true" aria-label="安全报告">
    <div class="modal-head">
      <span class="modal-icon"><AppIcon name="shield" size={18} /></span>
      <div>
        <strong>安全报告</strong>
        <p>基于当前数据库的服务端分析，不发送任何数据</p>
      </div>
      <button class="close-button" onclick={onclose} title="关闭" aria-label="关闭"
        ><AppIcon name="x" size={14} /></button
      >
    </div>

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
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    background: color-mix(in srgb, #000 45%, transparent);
  }

  .report-modal {
    display: flex;
    flex-direction: column;
    width: min(520px, calc(100% - 40px));
    max-height: min(560px, calc(100% - 80px));
    padding: 18px;
    border: 1px solid var(--border-color);
    border-radius: 13px;
    background: var(--surface-bg);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
  }

  .modal-head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }

  .modal-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-icon-radius, 7px);
    color: var(--selection-color);
    background: var(--hover-bg);
  }

  .modal-head strong {
    display: block;
    font-size: 13px;
    font-weight: 560;
  }

  .modal-head p {
    margin: 2px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .modal-head div {
    flex: 1;
    min-width: 0;
  }

  .close-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    flex: 0 0 auto;
    padding: 0;
    border: 0;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
    cursor: pointer;
  }

  .close-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

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
