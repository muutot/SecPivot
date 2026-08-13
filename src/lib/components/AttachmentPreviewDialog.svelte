<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { openPath } from "@tauri-apps/plugin-opener";
  import type { AttachmentInfo, AttachmentPreview, TempAttachmentRef } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import { formatBytes } from "$lib/utils/format";

  interface Props {
    entryUuid: string;
    attachment: AttachmentInfo;
    onclose: () => void;
    onsaved?: (name: string) => Promise<void> | void;
  }

  let { entryUuid, attachment, onclose, onsaved }: Props = $props();

  let preview = $state<AttachmentPreview | null>(null);
  let loading = $state(true);
  let error = $state("");
  let tempRef = $state<TempAttachmentRef | null>(null);
  let confirmExternal = $state(false);
  let opening = $state(false);
  let externalError = $state("");
  const sessionId = vault.getActiveSessionId();

  $effect(() => {
    if (!sessionId) return;
    void vault
      .callInSession(sessionId, () => vault.previewAttachment(entryUuid, attachment.name))
      .then((value) => {
        if (vault.getActiveSessionId() !== sessionId) return;
        preview = value;
        loading = false;
      })
      .catch((e) => {
        if (vault.getActiveSessionId() !== sessionId) return;
        error = String(e);
        loading = false;
      });
  });

  async function saveToDisk(): Promise<void> {
    const dest = await save({ defaultPath: attachment.name });
    if (!dest || !sessionId) return;
    await vault.callInSession(sessionId, () =>
      vault.saveAttachment(entryUuid, attachment.name, dest),
    );
    if (vault.getActiveSessionId() !== sessionId) return;
    await onsaved?.(attachment.name);
  }

  async function openExternal(): Promise<void> {
    // Explicit two-step confirmation before anything is written to disk and
    // handed to an external viewer.
    if (!confirmExternal) {
      confirmExternal = true;
      return;
    }
    confirmExternal = false;
    opening = true;
    externalError = "";
    try {
      if (!sessionId) return;
      const ref = await vault.callInSession(sessionId, () =>
        vault.openAttachmentTemp(entryUuid, attachment.name),
      );
      if (vault.getActiveSessionId() !== sessionId) {
        await vault.cleanupAttachmentTemp(ref.token);
        return;
      }
      tempRef = ref;
      await openPath(ref.path);
    } catch (e) {
      externalError = String(e);
    } finally {
      opening = false;
    }
  }

  async function discardTemp(): Promise<void> {
    if (tempRef) {
      await vault.cleanupAttachmentTemp(tempRef.token);
      tempRef = null;
    }
  }

  async function importChanges(): Promise<void> {
    if (!tempRef) return;
    const token = tempRef.token;
    tempRef = null;
    if (!sessionId) return;
    await vault.callInSession(sessionId, () =>
      vault.importAttachmentFromTemp(entryUuid, attachment.name, token),
    );
    if (vault.getActiveSessionId() !== sessionId) return;
    await onsaved?.(attachment.name);
  }

  function close(): void {
    if (tempRef) void vault.cleanupAttachmentTemp(tempRef.token);
    onclose();
  }
</script>

<ModalShell
  title={attachment.name}
  description={`${formatBytes(attachment.size)}${preview?.truncated ? " · 预览已截断" : ""}`}
  size="medium"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    {#if loading}
      <p class="preview-note">正在加载预览…</p>
    {:else if error}
      <p class="preview-note error">{error}</p>
    {:else if preview?.kind === "image"}
      <img class="preview-image" src={preview.data} alt={attachment.name} />
    {:else if preview?.kind === "text"}
      <pre class="preview-text">{preview.data}</pre>
    {:else}
      <p class="preview-note">
        该附件为二进制文件，无法在内存中预览；可保存到本地后用外部程序打开。
      </p>
    {/if}
    {#if externalError}
      <p class="preview-note error">外部打开失败：{externalError}</p>
    {/if}
    {#if tempRef}
      <p class="preview-note">
        已在外部程序打开；临时文件位于系统临时目录，关闭对话框或锁定后自动清理。
      </p>
    {/if}
  {/snippet}
  {#snippet actions()}
    <button class="modal-button" onclick={close}>关闭</button>
    {#if tempRef}
      <button class="modal-button" onclick={() => void importChanges()}>导入修改</button>
      <button class="modal-button" onclick={() => void discardTemp()}>丢弃修改</button>
    {/if}
    <button
      class="modal-button"
      class:primary={!tempRef}
      onclick={() => void openExternal()}
      disabled={opening}
    >
      {confirmExternal ? "再次点击确认在外部打开" : tempRef ? "重新打开" : "外部打开…"}
    </button>
    <button class="modal-button primary" onclick={() => void saveToDisk()}>保存到…</button>
  {/snippet}
</ModalShell>

<style>
  .preview-image {
    display: block;
    max-width: 100%;
    max-height: 420px;
    margin: 0 auto;
    border-radius: var(--settings-control-radius, 6px);
  }

  .preview-text {
    max-height: 420px;
    overflow: auto;
    margin: 0;
    padding: 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-family: var(--font-mono, "Cascadia Mono", Consolas, monospace);
    font-size: var(--font-size-secondary, 11px);
    white-space: pre-wrap;
    word-break: break-word;
  }

  .preview-note {
    margin: 8px 0;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
  }

  .preview-note.error {
    color: var(--danger-color);
  }
</style>
