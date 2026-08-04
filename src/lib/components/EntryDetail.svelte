<script lang="ts">
  import type { VaultEntry } from "$lib/types/vault";
  import type { HistoryVersion } from "$lib/types/vault";
  import { copyText } from "$lib/utils/clipboard";
  import { copySensitive } from "$lib/services/security";
  import { isTauriRuntime } from "$lib/services/settings";
  import { vault } from "$lib/services/vault";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import type { IconName } from "$lib/components/AppIcon.svelte";
  import { KEEPASS_ICONS, ENTRY_DEFAULT_ICON } from "$lib/utils/keepass-icons";
  import TotpWidget from "$lib/components/TotpWidget.svelte";

  interface Props {
    entry: VaultEntry;
    groupPath: string;
    inRecycleBin?: boolean;
    onfavorite: (entry: VaultEntry) => void;
    onedit: (entry: VaultEntry) => void;
    ondelete: (entry: VaultEntry) => void;
    onrestore?: (entry: VaultEntry) => void;
  }

  let {
    entry,
    groupPath,
    inRecycleBin = false,
    onfavorite,
    onedit,
    ondelete,
    onrestore,
  }: Props = $props();

  const iconName = $derived(
    ((entry.icon !== undefined ? (KEEPASS_ICONS[entry.icon] as string | undefined) : undefined) ??
      ENTRY_DEFAULT_ICON) as IconName,
  );

  /** Data URL of the database-stored custom icon (favicon), if any. */
  const customIconUrl = $derived(
    entry.customIcon ? $vault?.customIcons?.[entry.customIcon] : undefined,
  );

  let revealPassword = $state(false);
  let fetchedPassword = $state("");
  let passwordLoaded = $state(false);
  let passwordLoading = $state(false);
  let copied = $state("");
  let activeTab = $state<"fields" | "meta" | "attachments" | "history">("fields");
  let historyVersions = $state<HistoryVersion[]>([]);
  let historyLoading = $state(false);
  let historyLoadedUuid = $state<string | null>(null);

  let copiedTimer: ReturnType<typeof setTimeout> | undefined = $state();

  $effect(() => {
    entry.uuid;
    revealPassword = false;
    passwordLoaded = false;
    fetchedPassword = "";
    historyLoadedUuid = null;
    historyVersions = [];
  });

  async function loadHistory(force = false): Promise<void> {
    if (!force && historyLoadedUuid === entry.uuid) return;
    historyLoading = true;
    try {
      historyVersions = await vault.getEntryHistory(entry.uuid);
      historyLoadedUuid = entry.uuid;
    } finally {
      historyLoading = false;
    }
  }

  async function restoreVersion(version: HistoryVersion): Promise<void> {
    const when = version.modified ? new Date(version.modified).toLocaleString("zh-CN") : "未知时间";
    if (!window.confirm(`确定恢复到 ${when} 的版本吗？当前内容会保留为新的历史记录。`)) return;
    try {
      await vault.restoreEntryVersion(entry.uuid, version.index);
      historyLoadedUuid = null;
      await loadHistory(true);
      flash("restored");
    } catch {
      flash("error");
    }
  }

  /** Passwords are fetched on demand; never included in `VaultEntry` from the backend. */
  async function ensurePassword(): Promise<void> {
    if (passwordLoaded || passwordLoading) return;
    passwordLoading = true;
    try {
      fetchedPassword = await vault.getEntryPassword(entry.uuid);
      passwordLoaded = true;
    } finally {
      passwordLoading = false;
    }
  }

  async function copyPassword(): Promise<void> {
    try {
      await ensurePassword();
      await handleCopy(fetchedPassword, "password", true);
    } catch {
      flash("error");
    }
  }

  async function toggleReveal(): Promise<void> {
    try {
      await ensurePassword();
      revealPassword = !revealPassword;
    } catch {
      flash("error");
    }
  }

  function flash(kind: string): void {
    copied = kind;
    if (copiedTimer) clearTimeout(copiedTimer);
    copiedTimer = setTimeout(() => {
      copied = "";
      copiedTimer = undefined;
    }, 1200);
  }

  async function handleCopy(value: string, kind: string, sensitive = false): Promise<void> {
    try {
      if (sensitive) {
        await copySensitive(value);
      } else {
        await copyText(value);
      }
      flash(kind);
    } catch {
      flash("error");
    }
  }

  function openUrlExternal(): void {
    if (!entry.url) return;
    if (isTauriRuntime()) {
      void openUrl(entry.url);
    } else {
      window.open(entry.url, "_blank", "noopener,noreferrer");
    }
  }

  function formatTime(value?: string): string {
    if (!value) return "—";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "—";
    return date.toLocaleDateString("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit" });
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function toastMessage(): string {
    switch (copied) {
      case "error":
        return "操作失败";
      case "attachment":
        return "附件已保存";
      case "username":
        return "已复制用户名";
      case "password":
        return "已复制密码";
      case "restored":
        return "已恢复历史版本";
      case "url":
        return "已复制网址";
      default:
        return "已复制到剪贴板";
    }
  }

  async function saveAttachment(name: string): Promise<void> {
    try {
      let dest: string | null = null;
      if (isTauriRuntime()) {
        dest = await saveDialog({ defaultPath: name });
      } else {
        throw new Error("browser");
      }
      if (!dest) return;
      await vault.saveAttachment(entry.uuid, name, dest);
      flash("attachment");
    } catch {
      flash("error");
    }
  }
</script>

<div class="detail">
  <header class="detail-head">
    <div class="detail-title-row">
      <span class="entry-icon" style:--entry-color={entry.color}
        >{#if customIconUrl}
          <img class="entry-icon-img" src={customIconUrl} alt="" draggable="false" />
        {:else}
          <AppIcon name={iconName} size={20} />
        {/if}</span
      >
      <div class="detail-titles">
        <h3 class="detail-title">{entry.title || "未命名条目"}</h3>
        <p class="detail-path">{groupPath}</p>
      </div>
    </div>
    <div class="detail-actions">
      {#if inRecycleBin && onrestore}
        <button class="detail-btn restore" onclick={() => onrestore(entry)} title="恢复条目">
          <AppIcon name="undo" size={15} />
        </button>
      {/if}
      <button
        class="detail-btn"
        class:star-active={entry.favorite}
        onclick={() => onfavorite(entry)}
        title={entry.favorite ? "取消收藏" : "收藏条目"}
      >
        <AppIcon name="star" size={15} />
      </button>
      <button class="detail-btn" onclick={() => onedit(entry)} title="编辑条目">
        <AppIcon name="edit" size={15} />
      </button>
      <button class="detail-btn danger" onclick={() => ondelete(entry)} title="删除条目">
        <AppIcon name="trash" size={15} />
      </button>
    </div>
  </header>

  <div class="detail-tabs" role="tablist" aria-label="详情选项卡">
    <button
      type="button"
      role="tab"
      class="detail-tab"
      class:active={activeTab === "fields"}
      aria-selected={activeTab === "fields"}
      onclick={() => (activeTab = "fields")}
    >
      字段
    </button>
    <button
      type="button"
      role="tab"
      class="detail-tab"
      class:active={activeTab === "meta"}
      aria-selected={activeTab === "meta"}
      onclick={() => (activeTab = "meta")}
    >
      元属性
    </button>
    <button
      type="button"
      role="tab"
      class="detail-tab"
      class:active={activeTab === "attachments"}
      aria-selected={activeTab === "attachments"}
      onclick={() => (activeTab = "attachments")}
    >
      附件{#if entry.attachments?.length}
        ({entry.attachments.length}){/if}
    </button>
    <button
      type="button"
      role="tab"
      class="detail-tab"
      class:active={activeTab === "history"}
      aria-selected={activeTab === "history"}
      onclick={() => {
        activeTab = "history";
        void loadHistory();
      }}
    >
      历史
    </button>
  </div>

  <div class="detail-body" role="tabpanel">
    {#if activeTab === "fields"}
      <div class="field-block">
        <span class="field-label">用户名</span>
        <div class="field-value">
          <span class="field-text">{entry.username || "—"}</span>
          {#if entry.username}
            <button
              class="copy-btn"
              onclick={() => handleCopy(entry.username, "username")}
              title="复制用户名"
            >
              <AppIcon name="copy" size={13} />
            </button>
          {/if}
        </div>
      </div>

      <div class="field-block">
        <span class="field-label">密码</span>
        <div class="field-value">
          <span class="field-text mono">{revealPassword ? fetchedPassword : "••••••••••••"}</span>
          <button class="copy-btn" onclick={copyPassword} title="复制密码">
            <AppIcon name="copy" size={13} />
          </button>
          <button
            class="copy-btn"
            onclick={toggleReveal}
            title={revealPassword ? "隐藏密码" : "显示密码"}
          >
            <AppIcon name={revealPassword ? "eye-off" : "eye"} size={13} />
          </button>
        </div>
      </div>

      {#if entry.url}
        <div class="field-block">
          <span class="field-label">网址</span>
          <div class="field-value">
            <button class="url-link" onclick={openUrlExternal} title={entry.url}>
              <AppIcon name="globe" size={13} />
              <span class="field-text link">{entry.url}</span>
            </button>
          </div>
        </div>
      {/if}

      {#if entry.hasTotp}
        <div class="field-block">
          <span class="field-label">OTP 验证码</span>
          <div class="field-value">
            <TotpWidget entryUuid={entry.uuid} />
          </div>
        </div>
      {/if}

      {#if entry.notes}
        <div class="field-block">
          <span class="field-label">备注</span>
          <div class="field-notes">{entry.notes}</div>
        </div>
      {/if}

      {#if entry.customFields?.length}
        {#each entry.customFields as field}
          <div class="field-block">
            <span class="field-label">{field.name}</span>
            <div class="field-value">
              <span class="field-text" title={field.value}>{field.value || "—"}</span>
              {#if field.value}
                <button
                  class="copy-btn"
                  onclick={() => handleCopy(field.value, "custom")}
                  title="复制字段值"
                >
                  <AppIcon name="copy" size={13} />
                </button>
              {/if}
            </div>
          </div>
        {/each}
      {/if}
    {:else if activeTab === "meta"}
      <div class="field-block">
        <span class="field-label">所属分组</span>
        <div class="field-value">
          <span class="field-text">{groupPath || "—"}</span>
        </div>
      </div>

      <div class="field-block">
        <span class="field-label">创建时间</span>
        <div class="field-value">
          <span class="field-text">{formatTime(entry.created)}</span>
        </div>
      </div>

      <div class="field-block">
        <span class="field-label">修改时间</span>
        <div class="field-value">
          <span class="field-text">{formatTime(entry.modified)}</span>
        </div>
      </div>

      {#if entry.expires}
        <div class="field-block">
          <span class="field-label">过期时间</span>
          <div class="field-value">
            <span class="field-text" class:expired-text={entry.expired}>
              {formatTime(entry.expires)}{entry.expired ? " · 已过期" : ""}
            </span>
          </div>
        </div>
      {/if}

      {#if entry.tags}
        <div class="field-block">
          <span class="field-label">标签</span>
          <div class="field-value">
            <span class="field-text">{entry.tags}</span>
          </div>
        </div>
      {/if}

      <div class="field-block">
        <span class="field-label">收藏状态</span>
        <div class="field-value">
          <span class="field-text">{entry.favorite ? "已收藏" : "未收藏"}</span>
        </div>
      </div>

      <div class="field-block">
        <span class="field-label">UUID</span>
        <div class="field-value">
          <span class="field-text mono uuid-text" title={entry.uuid}>{entry.uuid}</span>
        </div>
      </div>
    {:else if activeTab === "attachments"}
      {#if entry.attachments?.length}
        <div class="attachment-list">
          {#each entry.attachments as attachment}
            <div class="attachment-item" title={attachment.name}>
              <AppIcon name="file" size={14} />
              <span class="attachment-name">{attachment.name}</span>
              <span class="attachment-size">{formatSize(attachment.size)}</span>
              <button
                class="copy-btn"
                onclick={() => saveAttachment(attachment.name)}
                title="保存附件"
              >
                <AppIcon name="download" size={13} />
              </button>
            </div>
          {/each}
        </div>
      {:else}
        <div class="tab-empty">
          <AppIcon name="file" size={18} />
          <p>该条目没有附件</p>
        </div>
      {/if}
    {:else if activeTab === "history"}
      {#if historyLoading}
        <div class="tab-empty">
          <AppIcon name="clock" size={18} />
          <p>正在加载历史版本…</p>
        </div>
      {:else if historyVersions.length === 0}
        <div class="tab-empty">
          <AppIcon name="clock" size={18} />
          <p>该条目没有历史版本</p>
        </div>
      {:else}
        <div class="history-list">
          {#each historyVersions as version (version.index)}
            <div
              class="history-item"
              title={`${version.username || ""}${version.url ? ` · ${version.url}` : ""}`}
            >
              <AppIcon name="clock" size={14} />
              <div class="history-item-main">
                <span class="history-time">
                  {version.modified
                    ? new Date(version.modified).toLocaleString("zh-CN")
                    : "未知时间"}
                </span>
                <span class="history-title">{version.title || "未命名条目"}</span>
              </div>
              <button class="copy-btn" onclick={() => restoreVersion(version)} title="恢复此版本">
                <AppIcon name="undo" size={13} />
              </button>
            </div>
          {/each}
        </div>
        <p class="history-hint">最多保留最近 10 个版本;恢复操作本身也会记录为新版本。</p>
      {/if}
    {/if}
  </div>

  {#if copied}
    <p class="copy-toast" class:error={copied === "error"} aria-live="polite">
      {toastMessage()}
    </p>
  {/if}
</div>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    border-left: 1px solid var(--border-subtle);
    background: var(--card-bg);
  }

  .detail-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 8px;
    padding: 12px 14px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .detail-title-row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .entry-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-icon-radius, 7px);
    color: var(--warning-color);
    background: var(--hover-bg);
  }

  .entry-icon-img {
    width: 20px;
    height: 20px;
    display: block;
    border-radius: 3px;
    object-fit: contain;
  }

  .entry-icon[style*="--entry-color"] {
    color: var(--entry-color);
    border-color: color-mix(in srgb, var(--entry-color) 45%, transparent);
    background: color-mix(in srgb, var(--entry-color) 12%, transparent);
  }

  .detail-titles {
    min-width: 0;
  }

  .detail-title {
    margin: 0;
    font-size: 13px;
    font-weight: 560;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail-path {
    margin: 2px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .detail-actions {
    display: flex;
    gap: 4px;
    flex: 0 0 auto;
  }

  .detail-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    cursor: pointer;
  }

  .detail-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .detail-btn.star-active {
    color: var(--warning-color);
    border-color: color-mix(in srgb, var(--warning-color) 40%, transparent);
  }

  .detail-btn.star-active:hover {
    color: var(--warning-color);
  }

  .detail-btn.danger:hover {
    color: var(--danger-color);
    border-color: color-mix(in srgb, var(--danger-color) 40%, transparent);
  }

  .detail-btn.restore {
    color: var(--success-color);
    border-color: color-mix(in srgb, var(--success-color) 40%, transparent);
  }

  .detail-btn.restore:hover {
    background: color-mix(in srgb, var(--success-color) 10%, var(--hover-bg));
  }

  .detail-tabs {
    display: flex;
    gap: 2px;
    padding: 8px 14px 0;
    border-bottom: 1px solid var(--border-subtle);
    flex: 0 0 auto;
  }

  .detail-tab {
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

  .detail-tab:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .detail-tab.active {
    color: var(--text-primary);
    border-bottom-color: var(--selection-color);
  }

  .detail-body {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 12px 14px 40px;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .field-block {
    margin-bottom: 14px;
  }

  .field-label {
    display: block;
    margin-bottom: 4px;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    letter-spacing: 0.04em;
  }

  .field-value {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 28px;
    padding: 0 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
  }

  .field-text {
    flex: 1;
    min-width: 0;
    color: var(--text-primary);
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .field-text.expired-text {
    color: var(--danger-color);
  }

  .field-text.mono {
    font-family: "Cascadia Code", "SFMono-Regular", Consolas, monospace;
    letter-spacing: 0.02em;
  }

  .field-text.link {
    color: var(--selection-color);
  }

  .url-link {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    flex: 1;
    padding: 0;
    border: 0;
    color: inherit;
    background: transparent;
    cursor: pointer;
  }

  .field-notes {
    padding: 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
    color: var(--text-secondary);
    font-size: 12px;
    line-height: 1.6;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .attachment-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .attachment-item {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--hover-bg);
  }

  .attachment-name {
    flex: 1;
    min-width: 0;
    font-size: 12px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .attachment-size {
    flex: 0 0 auto;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }

  .copy-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    flex: 0 0 auto;
    padding: 0;
    border: 0;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
    cursor: pointer;
  }

  .copy-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .uuid-text {
    font-size: var(--font-size-tiny, 10px);
    word-break: break-all;
  }

  .history-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .history-item {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 36px;
    padding: 0 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--hover-bg);
    color: var(--text-muted);
  }

  .history-item-main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }

  .history-time {
    font-size: var(--font-size-tiny, 10px);
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
  }

  .history-title {
    font-size: 12px;
    color: var(--text-primary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .history-hint {
    margin: 8px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .tab-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    padding: 40px 0;
    color: var(--text-faint);
  }

  .tab-empty p {
    margin: 0;
    font-size: var(--font-size-secondary, 11px);
  }

  .copy-toast {
    position: absolute;
    right: 14px;
    bottom: 12px;
    margin: 0;
    padding: 6px 10px;
    border: 1px solid color-mix(in srgb, var(--success-color) 40%, transparent);
    border-radius: var(--settings-feedback-radius, 7px);
    color: color-mix(in srgb, var(--success-color) 80%, white);
    background: color-mix(in srgb, var(--success-color) 12%, var(--surface-bg));
    font-size: var(--font-size-secondary, 11px);
  }

  .copy-toast.error {
    border-color: color-mix(in srgb, var(--danger-color) 40%, transparent);
    color: color-mix(in srgb, var(--danger-color) 80%, white);
    background: color-mix(in srgb, var(--danger-color) 12%, var(--surface-bg));
  }
</style>
