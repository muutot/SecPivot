<script lang="ts">
  import type { VaultEntry, HistoryVersion, CustomField, AttachmentInfo } from "$lib/types/vault";
  import { formatLocalDate } from "$lib/utils/date";
  import { formatBytes } from "$lib/utils/format";
  import { vault } from "$lib/services/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    entry: VaultEntry;
    version: HistoryVersion;
    onclose: () => void;
  }

  let { entry, version, onclose }: Props = $props();

  let revealedFields = $state<Record<string, boolean>>({});
  let fetchedValues = $state<Record<string, string>>({});

  function differs(current: string | undefined, historical: string): boolean {
    return (current ?? "") !== historical;
  }

  const titleDiff = $derived(differs(entry.title, version.title));
  const usernameDiff = $derived(differs(entry.username, version.username));
  const urlDiff = $derived(differs(entry.url, version.url));
  const notesDiff = $derived(differs(entry.notes, version.notes));
  const expiresDiff = $derived(differs(entry.expires ?? "", version.expires ?? ""));

  type FieldChange = "unchanged" | "added" | "removed" | "modified";

  interface CustomFieldRow {
    name: string;
    change: FieldChange;
    value: string;
    protected: boolean;
    /** True when the value must be fetched on demand (a protected field that
     * only exists in the current snapshot, where protected values are absent). */
    fetchable: boolean;
  }

  /** Union of the version's and the current entry's custom fields, tagged with
   * how each field differs between the two states. */
  const customFieldRows = $derived.by<CustomFieldRow[]>(() => {
    const currentByName = new Map((entry.customFields ?? []).map((f) => [f.name, f]));
    const versionByName = new Map(version.customFields.map((f) => [f.name, f]));
    const rows: CustomFieldRow[] = [];
    for (const [name, vf] of versionByName) {
      const cf = currentByName.get(name);
      if (!cf) {
        rows.push({
          name,
          change: "removed",
          value: vf.value,
          protected: vf.protected ?? false,
          fetchable: false,
        });
      } else if (
        (cf.protected ?? false) !== (vf.protected ?? false) ||
        (!(vf.protected ?? false) && cf.value !== vf.value)
      ) {
        rows.push({
          name,
          change: "modified",
          value: vf.value,
          protected: vf.protected ?? false,
          fetchable: false,
        });
      } else {
        rows.push({
          name,
          change: "unchanged",
          value: vf.value,
          protected: vf.protected ?? false,
          fetchable: false,
        });
      }
    }
    for (const [name, cf] of currentByName) {
      if (!versionByName.has(name)) {
        rows.push({
          name,
          change: "added",
          value: cf.value,
          protected: cf.protected ?? false,
          fetchable: cf.protected ?? false,
        });
      }
    }
    rows.sort((a, b) => a.name.localeCompare(b.name));
    return rows.filter((r) => r.name !== "KPRPC JSON");
  });

  const customFieldChangedCount = $derived(
    customFieldRows.filter((r) => r.change !== "unchanged").length,
  );

  interface AttachmentRow {
    name: string;
    size: number;
    change: FieldChange;
  }

  /** Union of the version's and the current entry's attachments, tagged with
   * how each differs. Attachments are matched by name; a size change counts as
   * a modification. */
  const attachmentRows = $derived.by<AttachmentRow[]>(() => {
    const currentByName = new Map((entry.attachments ?? []).map((a) => [a.name, a]));
    const versionByName = new Map(version.attachments.map((a) => [a.name, a]));
    const rows: AttachmentRow[] = [];
    for (const [name, va] of versionByName) {
      const ca = currentByName.get(name);
      if (!ca) {
        rows.push({ name, size: va.size, change: "removed" });
      } else if (ca.size !== va.size) {
        rows.push({ name, size: va.size, change: "modified" });
      } else {
        rows.push({ name, size: va.size, change: "unchanged" });
      }
    }
    for (const [name, ca] of currentByName) {
      if (!versionByName.has(name)) {
        rows.push({ name, size: ca.size, change: "added" });
      }
    }
    rows.sort((a, b) => a.name.localeCompare(b.name));
    return rows;
  });

  const attachmentChangedCount = $derived(
    attachmentRows.filter((r) => r.change !== "unchanged").length,
  );
  const totalDiffs = $derived(
    (titleDiff ? 1 : 0) +
      (usernameDiff ? 1 : 0) +
      (urlDiff ? 1 : 0) +
      (notesDiff ? 1 : 0) +
      (expiresDiff ? 1 : 0) +
      customFieldChangedCount +
      attachmentChangedCount,
  );

  function badgeLabel(change: FieldChange): string | null {
    if (change === "added") return "新增";
    if (change === "removed") return "已删除";
    if (change === "modified") return "已修改";
    return null;
  }

  async function toggleReveal(row: CustomFieldRow): Promise<void> {
    if (!row.protected) return;
    if (revealedFields[row.name]) {
      revealedFields[row.name] = false;
      return;
    }
    if (row.fetchable) {
      const value = await vault.getCustomFieldValue(entry.uuid, row.name);
      if (value !== null) fetchedValues[row.name] = value;
    }
    revealedFields[row.name] = true;
  }
</script>

<div class="modal-backdrop" role="presentation">
  <div class="editor-modal" role="dialog" aria-modal="true" aria-label="历史版本">
    <div class="modal-head">
      <span class="modal-icon"><AppIcon name="clock" size={18} /></span>
      <div>
        <strong>历史版本</strong>
        <p>
          {version.modified ? formatLocalDate(version.modified) : "未知时间"}
          {#if totalDiffs > 0}· {totalDiffs} 处差异{/if}
        </p>
      </div>
    </div>

    <div class="field">
      <span class="field-label">标题</span>
      <div class="read-value" class:changed={titleDiff}>
        {#if titleDiff}<span class="diff-badge">已变更</span>{/if}
        <span class="read-text">{version.title || "未命名条目"}</span>
      </div>
    </div>

    <div class="form-grid">
      <div class="field">
        <span class="field-label">用户名</span>
        <div class="read-value" class:changed={usernameDiff}>
          {#if usernameDiff}<span class="diff-badge">已变更</span>{/if}
          <span class="read-text">{version.username || "—"}</span>
        </div>
      </div>
      <div class="field">
        <span class="field-label">过期时间</span>
        <div class="read-value" class:changed={expiresDiff}>
          {#if expiresDiff}<span class="diff-badge">已变更</span>{/if}
          <span class="read-text">{version.expires ? formatLocalDate(version.expires) : "无"}</span>
        </div>
      </div>
    </div>

    <div class="field">
      <span class="field-label">网址</span>
      <div class="read-value" class:changed={urlDiff}>
        {#if urlDiff}<span class="diff-badge">已变更</span>{/if}
        <span class="read-text link">{version.url || "—"}</span>
      </div>
    </div>

    <div class="field">
      <span class="field-label">备注</span>
      <div class="read-value read-area" class:changed={notesDiff}>
        {#if notesDiff}<span class="diff-badge">已变更</span>{/if}
        <span class="read-text read-pre">{version.notes || "—"}</span>
      </div>
    </div>

    <div class="field">
      <span class="field-label"
        >自定义字段{#if customFieldRows.length}
          ({customFieldRows.length}){/if}</span
      >
      {#if customFieldRows.length === 0}
        <div class="read-value"><span class="read-text faint">无</span></div>
      {:else}
        {#each customFieldRows as row (row.name)}
          <div class="custom-row">
            <span class="custom-name">
              {row.name}
              {#if row.change !== "unchanged"}
                <span
                  class="diff-badge"
                  class:added={row.change === "added"}
                  class:removed={row.change === "removed"}
                  class:modified={row.change === "modified"}>{badgeLabel(row.change)}</span
                >
              {/if}
            </span>
            <div
              class="read-value"
              class:changed={row.change !== "unchanged"}
              class:added={row.change === "added"}
              class:removed={row.change === "removed"}
              class:modified={row.change === "modified"}
            >
              {#if row.protected}
                <AppIcon name="lock" size={12} />
              {/if}
              <span class="read-text mono">
                {row.protected && !revealedFields[row.name]
                  ? "••••••••"
                  : (row.fetchable && fetchedValues[row.name] !== undefined
                      ? fetchedValues[row.name]
                      : row.value) || "—"}
              </span>
              {#if row.protected}
                <button
                  class="copy-btn"
                  onclick={() => toggleReveal(row)}
                  title={revealedFields[row.name] ? "隐藏" : "显示"}
                >
                  <AppIcon name={revealedFields[row.name] ? "eye-off" : "eye"} size={12} />
                </button>
              {/if}
            </div>
          </div>
        {/each}
      {/if}
    </div>

    <div class="field">
      <span class="field-label"
        >附件{#if attachmentRows.length}
          ({attachmentRows.length}){/if}</span
      >
      {#if attachmentRows.length === 0}
        <div class="read-value"><span class="read-text faint">无</span></div>
      {:else}
        {#each attachmentRows as row (row.name)}
          <div class="custom-row">
            <span class="custom-name">
              {row.name}
              {#if row.change !== "unchanged"}
                <span
                  class="diff-badge"
                  class:added={row.change === "added"}
                  class:removed={row.change === "removed"}
                  class:modified={row.change === "modified"}>{badgeLabel(row.change)}</span
                >
              {/if}
            </span>
            <div
              class="read-value"
              class:added={row.change === "added"}
              class:removed={row.change === "removed"}
              class:modified={row.change === "modified"}
            >
              <AppIcon name="file" size={12} />
              <span class="read-text mono">{formatBytes(row.size)}</span>
            </div>
          </div>
        {/each}
      {/if}
    </div>

    <div class="modal-actions">
      <button class="modal-button primary" onclick={onclose}>关闭</button>
    </div>
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

  .editor-modal {
    width: min(500px, calc(100% - 40px));
    max-height: calc(100% - 48px);
    padding: 18px;
    border: 1px solid var(--border-color);
    border-radius: 13px;
    background: var(--surface-bg);
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.4);
    overflow: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
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

  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .field {
    display: block;
    min-width: 0;
    margin-bottom: 12px;
  }

  .field-label {
    display: block;
    margin-bottom: 5px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .read-value {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 30px;
    padding: 0 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
  }

  .read-value.changed {
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 8%, var(--input-bg));
  }

  .read-value.added {
    border-color: color-mix(in srgb, var(--success-color) 55%, transparent);
    background: color-mix(in srgb, var(--success-color) 8%, var(--input-bg));
  }

  .read-value.removed {
    border-color: color-mix(in srgb, var(--danger-color) 55%, transparent);
    background: color-mix(in srgb, var(--danger-color) 8%, var(--input-bg));
  }

  .read-value.modified {
    border-color: color-mix(in srgb, var(--warning-color) 55%, transparent);
    background: color-mix(in srgb, var(--warning-color) 8%, var(--input-bg));
  }

  .read-value.read-area {
    min-height: 0;
    align-items: flex-start;
    padding: 8px;
  }

  .read-text {
    flex: 1;
    min-width: 0;
    color: var(--text-primary);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .read-text.read-pre {
    white-space: pre-wrap;
    word-break: break-word;
    line-height: 1.5;
  }

  .read-text.mono {
    font-family: "Cascadia Code", "SFMono-Regular", Consolas, monospace;
    letter-spacing: 0.02em;
  }

  .read-text.link {
    color: var(--selection-color);
  }

  .read-text.faint {
    color: var(--text-faint);
  }

  .diff-badge {
    display: inline-flex;
    flex: 0 0 auto;
    padding: 1px 6px;
    border-radius: 999px;
    color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 15%, transparent);
    font-size: 10px;
    font-weight: 520;
    vertical-align: middle;
  }

  .diff-badge.added {
    color: color-mix(in srgb, var(--success-color) 85%, white);
    background: color-mix(in srgb, var(--success-color) 15%, transparent);
  }

  .diff-badge.removed {
    color: color-mix(in srgb, var(--danger-color) 85%, white);
    background: color-mix(in srgb, var(--danger-color) 15%, transparent);
  }

  .diff-badge.modified {
    color: color-mix(in srgb, var(--warning-color) 85%, white);
    background: color-mix(in srgb, var(--warning-color) 15%, transparent);
  }

  .custom-row {
    margin-bottom: 6px;
  }

  .custom-name {
    display: block;
    margin-bottom: 4px;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    letter-spacing: 0.04em;
  }

  .copy-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    flex: 0 0 auto;
    padding: 0;
    border: 0;
    border-radius: var(--settings-control-radius, 6px);
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
  }

  .copy-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 16px;
  }

  .modal-button {
    height: 30px;
    padding: 0 14px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--card-bg);
    font-size: 12px;
    cursor: pointer;
  }

  .modal-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .modal-button.primary {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
  }
</style>
