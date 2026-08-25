<script lang="ts">
  import type {
    VaultEntry,
    HistoryVersion,
    HistoryItemChange,
    CustomField,
    AttachmentInfo,
  } from "$lib/types/vault";
  import { formatLocalDate } from "$lib/utils/date";
  import { formatBytes } from "$lib/utils/format";
  import { vault } from "$lib/services/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";

  import Button from "$lib/components/templates/action/Button.svelte";
  interface Props {
    entry: VaultEntry;
    version: HistoryVersion;
    onclose: () => void;
  }

  let { entry, version, onclose }: Props = $props();

  let revealedFields = $state<Record<string, boolean>>({});
  let fetchedValues = $state<Record<string, string>>({});
  /** Per-field toggle: true shows the current-entry value instead of the
   * historical version value. */
  let showCurrent = $state<Record<string, boolean>>({});
  let activeTab = $state<"fields" | "meta" | "custom" | "data" | "attachments">("fields");

  function toggleValue(key: string): void {
    showCurrent[key] = !showCurrent[key];
  }

  // Change flags come from the backend diff, which sees the password and
  // protected custom-field values that never reach the renderer.
  const diff = $derived(version.diff);

  const titleDiff = $derived(diff.title);
  const usernameDiff = $derived(diff.username);
  const urlDiff = $derived(diff.url);
  const notesDiff = $derived(diff.notes);
  const expiresDiff = $derived(diff.expires);
  const tagsDiff = $derived(diff.tags);
  const hasTotpDiff = $derived(diff.hasTotp);
  const iconDiff = $derived(diff.icon);
  const colorDiff = $derived(diff.color);
  const qualityCheckDiff = $derived(diff.qualityCheck);
  const favoriteDiff = $derived(diff.favorite);
  const passwordDiff = $derived(diff.password);

  const fieldsGroupDiff = $derived(
    titleDiff || usernameDiff || expiresDiff || urlDiff || notesDiff || tagsDiff || passwordDiff,
  );
  const metaGroupDiff = $derived(
    hasTotpDiff || iconDiff || favoriteDiff || qualityCheckDiff || colorDiff,
  );

  type FieldChange = "unchanged" | "added" | "removed" | "modified";

  /** Change flag for one named item from the backend diff lists. */
  function changeOf(changes: HistoryItemChange[], name: string): FieldChange {
    const found = changes.find((c) => c.name === name);
    return (found?.change as FieldChange) ?? "unchanged";
  }

  interface CustomFieldRow {
    name: string;
    change: FieldChange;
    value: string;
    protected: boolean;
    /** True when the value must be fetched on demand (a protected field that
     * only exists in the current snapshot, where protected values are absent). */
    fetchable: boolean;
    /** Current-entry counterpart value for the value toggle, when available. */
    currentValue?: string;
    currentProtected?: boolean;
  }

  /** Union of the version's and the current entry's custom fields, tagged with
   * the backend's per-field change flags. */
  const customFieldRows = $derived.by<CustomFieldRow[]>(() => {
    const currentByName = new Map((entry.customFields ?? []).map((f) => [f.name, f]));
    const versionByName = new Map(version.customFields.map((f) => [f.name, f]));
    const rows: CustomFieldRow[] = [];
    for (const [name, vf] of versionByName) {
      const cf = currentByName.get(name);
      rows.push({
        name,
        change: changeOf(diff.customFields, name),
        value: vf.value,
        protected: vf.protected ?? false,
        fetchable: false,
        currentValue: cf?.value,
        currentProtected: cf?.protected ?? false,
      });
    }
    for (const [name, cf] of currentByName) {
      if (!versionByName.has(name)) {
        rows.push({
          name,
          change: changeOf(diff.customFields, name),
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
    /** Current-entry counterpart of `size` for the value toggle. */
    currentSize?: number;
    change: FieldChange;
  }

  /** Union of the version's and the current entry's attachments, tagged with
   * the backend's per-item change flags. */
  const attachmentRows = $derived.by<AttachmentRow[]>(() => {
    const currentByName = new Map((entry.attachments ?? []).map((a) => [a.name, a]));
    const versionByName = new Map(version.attachments.map((a) => [a.name, a]));
    const rows: AttachmentRow[] = [];
    for (const [name, va] of versionByName) {
      const ca = currentByName.get(name);
      rows.push({
        name,
        size: va.size,
        currentSize: ca?.size,
        change: changeOf(diff.attachments, name),
      });
    }
    for (const [name, ca] of currentByName) {
      if (!versionByName.has(name)) {
        rows.push({ name, size: ca.size, change: changeOf(diff.attachments, name) });
      }
    }
    rows.sort((a, b) => a.name.localeCompare(b.name));
    return rows;
  });

  const attachmentChangedCount = $derived(
    attachmentRows.filter((r) => r.change !== "unchanged").length,
  );

  interface CustomDataRow {
    key: string;
    change: FieldChange;
    label: string;
    /** Current-entry counterpart of `label` for the value toggle. */
    currentLabel?: string;
  }

  /** Union of the version's and the current entry's `CustomData` items, tagged
   * with the backend's per-key change flags. */
  const customDataRows = $derived.by<CustomDataRow[]>(() => {
    const label = (item: { value?: string; binary?: string; modified?: string }) =>
      item.binary !== undefined ? `二进制 ${item.binary.length} 字节` : (item.value ?? "—");
    const currentByName = new Map((entry.customData ?? []).map((item) => [item.key, label(item)]));
    const versionByName = new Map(
      (version.customData ?? []).map((item) => [item.key, label(item)]),
    );
    const rows: CustomDataRow[] = [];
    for (const [key, value] of versionByName) {
      rows.push({
        key,
        change: changeOf(diff.customData, key),
        label: value,
        currentLabel: currentByName.get(key),
      });
    }
    for (const [key, value] of currentByName) {
      if (!versionByName.has(key)) {
        rows.push({ key, change: changeOf(diff.customData, key), label: value });
      }
    }
    rows.sort((a, b) => a.key.localeCompare(b.key));
    return rows;
  });

  const customDataChangedCount = $derived(
    customDataRows.filter((r) => r.change !== "unchanged").length,
  );

  const totalDiffs = $derived(
    (titleDiff ? 1 : 0) +
      (usernameDiff ? 1 : 0) +
      (passwordDiff ? 1 : 0) +
      (urlDiff ? 1 : 0) +
      (notesDiff ? 1 : 0) +
      (expiresDiff ? 1 : 0) +
      (tagsDiff ? 1 : 0) +
      (hasTotpDiff ? 1 : 0) +
      (iconDiff ? 1 : 0) +
      (colorDiff ? 1 : 0) +
      (qualityCheckDiff ? 1 : 0) +
      (favoriteDiff ? 1 : 0) +
      customFieldChangedCount +
      customDataChangedCount +
      attachmentChangedCount,
  );

  function badgeLabel(change: FieldChange): string | null {
    if (change === "added") return "新增";
    if (change === "removed") return "已删除";
    if (change === "modified") return "已修改";
    return null;
  }

  function fmtDate(value: string | null | undefined): string {
    return value ? formatLocalDate(value) : "无";
  }

  function fieldTipValue(value: string | undefined, isProtected: boolean | undefined): string {
    if (isProtected) return "（受保护）";
    return value || "—";
  }

  function iconLabel(e: { icon?: number; customIcon?: string }): string {
    return e.icon !== undefined ? `图标#${e.icon}` : e.customIcon ? "自定义图标" : "默认图标";
  }

  async function toggleReveal(row: CustomFieldRow): Promise<void> {
    // Only the current entry's protected values are fetchable on demand;
    // historical snapshots never carry their plaintext (server-side policy).
    if (!row.protected || !row.fetchable) return;
    if (revealedFields[row.name]) {
      revealedFields[row.name] = false;
      return;
    }
    if (row.fetchable) {
      const sessionId = vault.getActiveSessionId();
      if (!sessionId) return;
      const value = await vault.callInSession(sessionId, () =>
        vault.getCustomFieldValue(entry.uuid, row.name),
      );
      if (vault.getActiveSessionId() !== sessionId) return;
      if (value !== null) fetchedValues[row.name] = value;
    }
    revealedFields[row.name] = true;
  }
</script>

<ModalShell
  title="历史版本"
  description={`${version.modified ? formatLocalDate(version.modified) : "未知时间"}${totalDiffs > 0 ? ` · ${totalDiffs} 处差异` : ""}`}
  size="large"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet icon()}<AppIcon name="clock" size={18} />{/snippet}
  {#snippet children()}
    <div class="editor-tabs" role="tablist" aria-label="历史版本字段分组">
      <button
        type="button"
        role="tab"
        class="editor-tab"
        class:active={activeTab === "fields"}
        aria-selected={activeTab === "fields"}
        onclick={() => (activeTab = "fields")}
      >
        字段{#if fieldsGroupDiff}<span class="tab-dot" title="包含变更项"></span>{/if}
      </button>
      <button
        type="button"
        role="tab"
        class="editor-tab"
        class:active={activeTab === "meta"}
        aria-selected={activeTab === "meta"}
        onclick={() => (activeTab = "meta")}
      >
        元属性{#if metaGroupDiff}<span class="tab-dot" title="包含变更项"></span>{/if}
      </button>
      <button
        type="button"
        role="tab"
        class="editor-tab"
        class:active={activeTab === "custom"}
        aria-selected={activeTab === "custom"}
        onclick={() => (activeTab = "custom")}
      >
        自定义字段{#if customFieldRows.length}({customFieldRows.length}){/if}{#if customFieldChangedCount > 0}<span
            class="tab-dot"
            title="包含变更项"
          ></span>{/if}
      </button>
      <button
        type="button"
        role="tab"
        class="editor-tab"
        class:active={activeTab === "data"}
        aria-selected={activeTab === "data"}
        onclick={() => (activeTab = "data")}
      >
        自定义数据{#if customDataRows.length}({customDataRows.length}){/if}{#if customDataChangedCount > 0}<span
            class="tab-dot"
            title="包含变更项"
          ></span>{/if}
      </button>
      <button
        type="button"
        role="tab"
        class="editor-tab"
        class:active={activeTab === "attachments"}
        aria-selected={activeTab === "attachments"}
        onclick={() => (activeTab = "attachments")}
      >
        附件{#if attachmentRows.length}({attachmentRows.length}){/if}{#if attachmentChangedCount > 0}<span
            class="tab-dot"
            title="包含变更项"
          ></span>{/if}
      </button>
    </div>

    {#if activeTab === "fields"}
      <div role="tabpanel">
        <div class="field">
          <span class="field-label">标题</span>
          <div class="read-value" class:changed={titleDiff}>
            {#if titleDiff}<span class="diff-badge">已变更</span>{/if}
            <span class="read-text"
              >{(showCurrent.title ? entry.title : version.title) || "未命名条目"}</span
            >
            {#if titleDiff}{@render swapBtn("title")}{/if}
          </div>
        </div>

        <div class="field">
          <span class="field-label">密码</span>
          <div class="read-value" class:changed={passwordDiff}>
            {#if passwordDiff}<span class="diff-badge">已变更</span>{/if}
            <AppIcon name="lock" size={12} />
            <span class="read-text mono">••••••••</span>
          </div>
        </div>

        <div class="form-grid">
          <div class="field">
            <span class="field-label">用户名</span>
            <div class="read-value" class:changed={usernameDiff}>
              {#if usernameDiff}<span class="diff-badge">已变更</span>{/if}
              <span class="read-text"
                >{(showCurrent.username ? entry.username : version.username) || "—"}</span
              >
              {#if usernameDiff}{@render swapBtn("username")}{/if}
            </div>
          </div>
          <div class="field">
            <span class="field-label">过期时间</span>
            <div class="read-value" class:changed={expiresDiff}>
              {#if expiresDiff}<span class="diff-badge">已变更</span>{/if}
              <span class="read-text"
                >{fmtDate(showCurrent.expires ? entry.expires : version.expires)}</span
              >
              {#if expiresDiff}{@render swapBtn("expires")}{/if}
            </div>
          </div>
        </div>

        <div class="field">
          <span class="field-label">网址</span>
          <div class="read-value" class:changed={urlDiff}>
            {#if urlDiff}<span class="diff-badge">已变更</span>{/if}
            <span class="read-text link">{(showCurrent.url ? entry.url : version.url) || "—"}</span>
            {#if urlDiff}{@render swapBtn("url")}{/if}
          </div>
        </div>

        <div class="field">
          <span class="field-label">备注</span>
          <div class="read-value read-area" class:changed={notesDiff}>
            {#if notesDiff}<span class="diff-badge">已变更</span>{/if}
            <span class="read-text read-pre"
              >{(showCurrent.notes ? entry.notes : version.notes) || "—"}</span
            >
            {#if notesDiff}{@render swapBtn("notes")}{/if}
          </div>
        </div>

        <div class="field">
          <span class="field-label">标签</span>
          <div class="read-value" class:changed={tagsDiff}>
            {#if tagsDiff}<span class="diff-badge">已变更</span>{/if}
            <span class="read-text">{(showCurrent.tags ? entry.tags : version.tags) || "—"}</span>
            {#if tagsDiff}{@render swapBtn("tags")}{/if}
          </div>
        </div>
      </div>
    {:else if activeTab === "meta"}
      <div class="meta-grid" role="tabpanel">
        <div class="read-value" class:changed={hasTotpDiff}>
          {#if hasTotpDiff}<span class="diff-badge">已变更</span>{/if}
          <AppIcon name="key" size={12} />
          <span class="read-text"
            >{(showCurrent.totp ? entry.hasTotp : version.hasTotp) ? "含 TOTP" : "无 TOTP"}</span
          >
          {#if hasTotpDiff}{@render swapBtn("totp")}{/if}
        </div>
        <div class="read-value" class:changed={iconDiff}>
          {#if iconDiff}<span class="diff-badge">已变更</span>{/if}
          <AppIcon name="grid" size={12} />
          <span class="read-text">{showCurrent.icon ? iconLabel(entry) : iconLabel(version)}</span>
          {#if iconDiff}{@render swapBtn("icon")}{/if}
        </div>
        <div class="read-value" class:changed={favoriteDiff}>
          {#if favoriteDiff}<span class="diff-badge">已变更</span>{/if}
          <AppIcon name="star" size={12} filled={version.favorite} />
          <span class="read-text"
            >{(showCurrent.favorite ? entry.favorite : version.favorite)
              ? "已收藏"
              : "未收藏"}</span
          >
          {#if favoriteDiff}{@render swapBtn("favorite")}{/if}
        </div>
        <div class="read-value" class:changed={qualityCheckDiff}>
          {#if qualityCheckDiff}<span class="diff-badge">已变更</span>{/if}
          <AppIcon name="shield" size={12} />
          <span class="read-text"
            >{(showCurrent.qualityCheck ? entry.qualityCheck : version.qualityCheck)
              ? "密码质量检查开启"
              : "密码质量检查关闭"}</span
          >
          {#if qualityCheckDiff}{@render swapBtn("qualityCheck")}{/if}
        </div>
        {#if colorDiff || version.color}
          <div class="read-value" class:changed={colorDiff}>
            {#if colorDiff}<span class="diff-badge">已变更</span>{/if}
            <span class="color-swatch" style:--swatch={version.color ?? "transparent"}></span>
            <span class="read-text"
              >{(showCurrent.color ? entry.color : version.color) ?? "无背景色"}</span
            >
            {#if colorDiff}{@render swapBtn("color")}{/if}
          </div>
        {/if}
      </div>
    {:else if activeTab === "custom"}
      <div role="tabpanel">
        {#if customFieldRows.length === 0}
          <div class="read-value"><span class="read-text faint">无</span></div>
        {:else}
          {#each customFieldRows as row (row.name)}
            <div class="custom-row">
              <span class="custom-name">
                {row.name}
                {#if row.change === "modified"}
                  <span class="diff-badge modified">已修改</span>
                {:else if row.change !== "unchanged"}
                  <span
                    class="diff-badge"
                    class:added={row.change === "added"}
                    class:removed={row.change === "removed"}>{badgeLabel(row.change)}</span
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
                  {#if showCurrent[`cf:${row.name}`]}
                    {fieldTipValue(row.currentValue, row.currentProtected)}
                  {:else if row.protected && !revealedFields[row.name]}
                    ••••••••
                  {:else}
                    {(row.fetchable && fetchedValues[row.name] !== undefined
                      ? fetchedValues[row.name]
                      : row.value) || "—"}
                  {/if}
                </span>
                {#if row.protected && row.fetchable}
                  <button
                    class="copy-btn"
                    onclick={() => toggleReveal(row)}
                    title={revealedFields[row.name] ? "隐藏" : "显示"}
                  >
                    <AppIcon name={revealedFields[row.name] ? "eye-off" : "eye"} size={12} />
                  </button>
                {/if}
                {#if row.change === "modified"}{@render swapBtn(`cf:${row.name}`)}{/if}
              </div>
            </div>
          {/each}
        {/if}
      </div>
    {:else if activeTab === "data"}
      <div role="tabpanel">
        {#if customDataRows.length === 0}
          <div class="read-value"><span class="read-text faint">无</span></div>
        {:else}
          {#each customDataRows as row (row.key)}
            <div class="custom-row">
              <span class="custom-name">
                {row.key}
                {#if row.change === "modified"}
                  <span class="diff-badge modified">已修改</span>
                {:else if row.change !== "unchanged"}
                  <span
                    class="diff-badge"
                    class:added={row.change === "added"}
                    class:removed={row.change === "removed"}>{badgeLabel(row.change)}</span
                  >
                {/if}
              </span>
              <div
                class="read-value"
                class:added={row.change === "added"}
                class:removed={row.change === "removed"}
                class:modified={row.change === "modified"}
              >
                <span class="read-text mono">
                  {showCurrent[`cd:${row.key}`] ? (row.currentLabel ?? "—") : row.label}
                </span>
                {#if row.change === "modified"}{@render swapBtn(`cd:${row.key}`)}{/if}
              </div>
            </div>
          {/each}
        {/if}
      </div>
    {:else}
      <div role="tabpanel">
        {#if attachmentRows.length === 0}
          <div class="read-value"><span class="read-text faint">无</span></div>
        {:else}
          {#each attachmentRows as row (row.name)}
            <div class="custom-row">
              <span class="custom-name">
                {row.name}
                {#if row.change === "modified"}
                  <span class="diff-badge modified">已修改</span>
                {:else if row.change !== "unchanged"}
                  <span
                    class="diff-badge"
                    class:added={row.change === "added"}
                    class:removed={row.change === "removed"}>{badgeLabel(row.change)}</span
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
                <span class="read-text mono">
                  {formatBytes(
                    showCurrent[`at:${row.name}`] ? (row.currentSize ?? row.size) : row.size,
                  )}
                </span>
                {#if row.change === "modified"}{@render swapBtn(`at:${row.name}`)}{/if}
              </div>
            </div>
          {/each}
        {/if}
      </div>
    {/if}
  {/snippet}
  {#snippet actions()}
    <Button variant="primary" onclick={onclose}>关闭</Button>
  {/snippet}
</ModalShell>

{#snippet swapBtn(key: string)}
  <button
    class="swap-btn"
    class:active={showCurrent[key]}
    onclick={() => toggleValue(key)}
    title={showCurrent[key]
      ? "当前显示：现在条目的值。点击查看该历史版本的值"
      : "当前显示：该历史版本的值。点击查看现在条目的值"}
  >
    {showCurrent[key] ? "当前值" : "历史值"}
  </button>
{/snippet}

<style>
  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  .meta-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 6px;
  }

  .color-swatch {
    width: 12px;
    height: 12px;
    flex: 0 0 auto;
    border: 1px solid var(--border-subtle);
    border-radius: 3px;
    background: var(--swatch, transparent);
  }

  .editor-tabs {
    display: flex;
    gap: 2px;
    margin: -12px 0 12px;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .editor-tab {
    display: inline-flex;
    align-items: flex-start;
    gap: 3px;
    padding: 5px 12px;
    border: 0;
    border-bottom: 2px solid transparent;
    border-radius: var(--settings-control-radius, 6px) var(--settings-control-radius, 6px) 0 0;
    background: transparent;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
    transition:
      color 80ms ease,
      border-color 80ms ease;
  }

  .editor-tab:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .editor-tab.active {
    color: var(--text-primary);
    border-bottom-color: var(--selection-color);
  }

  .tab-dot {
    width: 5px;
    height: 5px;
    flex: 0 0 auto;
    margin-top: 1px;
    border-radius: 999px;
    background: var(--selection-color);
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
    font-family: var(--font-mono);
    letter-spacing: 0.02em;
  }

  .read-text.link {
    color: var(--link-color);
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

  .swap-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    height: 18px;
    flex: 0 0 auto;
    padding: 0 7px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: transparent;
    font-size: 10px;
    font-weight: 520;
    cursor: pointer;
  }

  .swap-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .swap-btn.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 45%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .diff-badge.added {
    color: color-mix(in srgb, var(--success-color) 85%, white);
    background: color-mix(in srgb, var(--success-color) 15%, transparent);
  }

  .diff-badge.removed {
    color: color-mix(in srgb, var(--danger-color) 85%, white);
    background: color-mix(in srgb, var(--danger-color) 15%, transparent);
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
</style>
