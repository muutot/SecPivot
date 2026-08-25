<script lang="ts">
  import { onDestroy } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { openPath } from "@tauri-apps/plugin-opener";
  import type { AttachmentInfo, AttachmentPreview, TempAttachmentRef } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import { replaceDisposable, settleDisposable } from "$lib/utils/disposable";
  import { awaitCurrentView, KeyedViewGuard, sessionResourceKey } from "$lib/utils/session-state";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import { formatBytes } from "$lib/utils/format";

  import Button from "$lib/components/templates/action/Button.svelte";
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
  let savingToDisk = $state(false);
  let externalError = $state("");
  const sessionId = vault.getActiveSessionId();
  let activeSessionId = $state(sessionId);
  const dialogView = new KeyedViewGuard();
  let activeKey: string | null = null;

  function replaceTempRef(replacement: TempAttachmentRef | null): void {
    tempRef = replaceDisposable(
      tempRef,
      replacement,
      (ref) => void vault.cleanupAttachmentTemp(ref.token),
    );
  }

  onDestroy(() => {
    dialogView.activate(null);
    replaceTempRef(null);
  });

  $effect(() => vault.activeId.subscribe((value) => (activeSessionId = value)));

  $effect(() => {
    const uuid = entryUuid;
    const name = attachment.name;
    const key =
      sessionId && activeSessionId === sessionId
        ? sessionResourceKey(sessionId, `${uuid}\0${name}`)
        : null;
    if (key === activeKey) return;
    activeKey = key;
    dialogView.activate(key);
    replaceTempRef(null);
    preview = null;
    loading = key !== null;
    error = key === null ? "数据库会话已切换" : "";
    confirmExternal = false;
    opening = false;
    importing = false;
    savingToDisk = false;
    externalError = "";
    const view = dialogView.capture();
    if (!sessionId || !view) return;
    void vault
      .callInSession(sessionId, () => vault.previewAttachment(uuid, name))
      .then((value) => {
        if (!dialogView.isCurrent(view)) return;
        preview = value;
      })
      .catch((e) => {
        if (!dialogView.isCurrent(view)) return;
        error = String(e);
      })
      .finally(() => {
        if (dialogView.isCurrent(view)) loading = false;
      });
  });

  async function saveToDisk(): Promise<void> {
    if (savingToDisk || !sessionId) return;
    const view = dialogView.capture();
    if (!view) return;
    const uuid = entryUuid;
    const name = attachment.name;
    savingToDisk = true;
    externalError = "";
    try {
      const picked = await awaitCurrentView(dialogView, view, () => save({ defaultPath: name }));
      if (!picked.current || !picked.value) return;
      await vault.callInSession(sessionId, () =>
        vault.saveAttachment(uuid, name, String(picked.value)),
      );
      if (!dialogView.isCurrent(view)) return;
      await onsaved?.(name);
    } catch (e) {
      if (dialogView.isCurrent(view)) externalError = String(e);
    } finally {
      if (dialogView.isCurrent(view)) savingToDisk = false;
    }
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
    const view = dialogView.capture();
    if (!view) {
      opening = false;
      return;
    }
    const uuid = entryUuid;
    const name = attachment.name;
    try {
      if (!sessionId) return;
      const ref = await vault.callInSession(sessionId, () => vault.openAttachmentTemp(uuid, name));
      if (!dialogView.isCurrent(view)) {
        await vault.cleanupAttachmentTemp(ref.token);
        return;
      }
      replaceTempRef(ref);
      await openPath(ref.path);
    } catch (e) {
      if (dialogView.isCurrent(view)) externalError = String(e);
    } finally {
      if (dialogView.isCurrent(view)) opening = false;
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
    const view = dialogView.capture();
    if (!view) return;
    const uuid = entryUuid;
    const name = attachment.name;
    importing = true;
    externalError = "";
    try {
      await vault.callInSession(sessionId, () => vault.importAttachmentFromTemp(uuid, name, token));
      tempRef = settleDisposable(tempRef, ownedRef, true);
      if (!dialogView.isCurrent(view)) return;
      await onsaved?.(name);
    } catch (e) {
      tempRef = settleDisposable(tempRef, ownedRef, false);
      if (dialogView.isCurrent(view) && tempRef === ownedRef) externalError = String(e);
    } finally {
      if (dialogView.isCurrent(view)) importing = false;
    }
  }

  function close(): void {
    dialogView.activate(null);
    replaceTempRef(null);
    onclose();
  }
</script>

<ModalShell
  title={attachment.name}
  description={`${formatBytes(attachment.size)}${preview?.truncated ? " · 预览已截断" : ""}`}
  size="medium"
  scrollable
  closeOnEscape={!importing}
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
    <Button onclick={close} disabled={importing}>关闭</Button>
    {#if tempRef}
      <Button onclick={() => void importChanges()} disabled={importing}>
        {importing ? "正在导入…" : "导入修改"}</Button
      >
      <Button onclick={discardTemp} disabled={importing}>丢弃修改</Button>
    {/if}
    <Button
      variant={!tempRef ? "primary" : "plain"}
      onclick={() => void openExternal()}
      disabled={opening || importing}
    >
      {confirmExternal ? "再次点击确认在外部打开" : tempRef ? "重新打开" : "外部打开…"}</Button
    >
    <Button
      variant="primary"
      onclick={() => void saveToDisk()}
      disabled={opening || importing || savingToDisk}
    >
      {savingToDisk ? "保存中…" : "保存到…"}</Button
    >
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
