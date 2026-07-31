<script lang="ts">
  import type { VaultEntry } from "$lib/types/vault";
  import { copyText } from "$lib/utils/clipboard";
  import { copySensitive } from "$lib/services/security";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import TotpWidget from "$lib/components/TotpWidget.svelte";

  interface Props {
    entry: VaultEntry;
    groupPath: string;
    onedit: (entry: VaultEntry) => void;
    ondelete: (entry: VaultEntry) => void;
  }

  let { entry, groupPath, onedit, ondelete }: Props = $props();

  let revealPassword = $state(false);
  let copied = $state("");

  let copiedTimer: ReturnType<typeof setTimeout> | undefined = $state();

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

  function openUrl(): void {
    if (!entry.url) return;
    window.open(entry.url, "_blank", "noopener,noreferrer");
  }

  function formatTime(value?: string): string {
    if (!value) return "—";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return "—";
    return date.toLocaleDateString("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit" });
  }
</script>

<div class="detail">
  <header class="detail-head">
    <div class="detail-title-row">
      <span class="entry-icon"><AppIcon name="key" size={15} /></span>
      <div class="detail-titles">
        <h3 class="detail-title">{entry.title || "未命名条目"}</h3>
        <p class="detail-path">{groupPath}</p>
      </div>
    </div>
    <div class="detail-actions">
      <button class="detail-btn" onclick={() => onedit(entry)} title="编辑条目">
        <AppIcon name="edit" size={15} />
      </button>
      <button class="detail-btn danger" onclick={() => ondelete(entry)} title="删除条目">
        <AppIcon name="trash" size={15} />
      </button>
    </div>
  </header>

  <div class="detail-body">
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
        <span class="field-text mono">{revealPassword ? entry.password : "••••••••••••"}</span>
        <button
          class="copy-btn"
          onclick={() => handleCopy(entry.password, "password", true)}
          title="复制密码"
        >
          <AppIcon name="copy" size={13} />
        </button>
        <button
          class="copy-btn"
          onclick={() => (revealPassword = !revealPassword)}
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
          <button class="url-link" onclick={openUrl} title={entry.url}>
            <AppIcon name="globe" size={13} />
            <span class="field-text link">{entry.url}</span>
          </button>
        </div>
      </div>
    {/if}

    {#if entry.totp}
      <div class="field-block">
        <span class="field-label">TOTP 验证码</span>
        <div class="field-value">
          <TotpWidget seed={entry.totp} entryUuid={entry.uuid} />
        </div>
      </div>
    {/if}

    {#if entry.notes}
      <div class="field-block">
        <span class="field-label">备注</span>
        <div class="field-notes">{entry.notes}</div>
      </div>
    {/if}

    <div class="field-block meta">
      <span class="field-label">创建 / 修改</span>
      <div class="field-value meta-values">
        <span>{formatTime(entry.created)}</span>
        <span>{formatTime(entry.modified)}</span>
      </div>
    </div>
  </div>

  {#if copied}
    <p class="copy-toast" class:error={copied === "error"} aria-live="polite">
      {copied === "error" ? "复制失败" : "已复制到剪贴板"}
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

  .detail-btn.danger:hover {
    color: var(--danger-color);
    border-color: color-mix(in srgb, var(--danger-color) 40%, transparent);
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

  .meta-values {
    display: flex;
    justify-content: space-between;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
    font-variant-numeric: tabular-nums;
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
