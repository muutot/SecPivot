<script lang="ts">
  import { onDestroy } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { openPath } from "@tauri-apps/plugin-opener";
  import type { AttachmentInfo, AttachmentPreview, TempAttachmentRef } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import { replaceDisposable, settleDisposable } from "$lib/utils/disposable";
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
  let importing = $state(false);
  let externalError = $state("");
  const sessionId = vault.getActiveSessionId();
  let disposed = false;

  function replaceTempRef(replacement: TempAttachmentRef | null): void {
    tempRef = replaceDisposable(
      tempRef,
      replacement,
      (ref) => void vault.cleanupAttachmentTemp(ref.token),
    );
  }

  onDestroy(() => {
    disposed = true;
    replaceTempRef(null);
  });

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
    if (opening || importing) return;
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
      if (disposed) {
        await vault.cleanupAttachmentTemp(ref.token);
        return;
      }
      replaceTempRef(ref);
      await openPath(ref.path);
    } catch (e) {
      if (!disposed) externalError = String(e);
    } finally {
      if (!disposed) opening = false;
    }
  }

  function discardTemp(): void {
    replaceTempRef(null);
  }

  async function importChanges(): Promise<void> {
    if (!tempRef || importing) return;
    const ownedRef = tempRef;
    const token = ownedRef.token;
    if (!sessionId) return;
    importing = true;
    externalError = "";
    try {
      await vault.callInSession(sessionId, () =>
        vault.importAttachmentFromTemp(entryUuid, attachment.name, token),
      );
      tempRef = settleDisposable(tempRef, ownedRef, true);
      if (vault.getActiveSessionId() !== sessionId || disposed) return;
      await onsaved?.(attachment.name);
    } catch (e) {
      tempRef = settleDisposable(tempRef, ownedRef, false);
      if (!disposed && tempRef === ownedRef) externalError = String(e);
    } finally {
      if (!disposed) importing = false;
    }
  }

  function close(): void {
    replaceTempRef(null);
    onclose();
  }
</script>

<ModalShell
  title={attachment.name}
  description={`${formatBytes(attachment.size)}${preview?.truncated ? " · 预览已截断" : ""}`}
  size="medium"
  scrollable
  closeOnEscape
  onclose={close}
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
      <button class="modal-button" onclick={() => void importChanges()} disabled={importing}>
        {importing ? "正在导入…" : "导入修改"}
      </button>
      <button class="modal-button" onclick={discardTemp} disabled={importing}>丢弃修改</button>
    {/if}
    <button
      class="modal-button"
      class:primary={!tempRef}
      onclick={() => void openExternal()}
      disabled={opening || importing}
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
