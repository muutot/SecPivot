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
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  interface Props {
    entryUuid: string;
    attachment: AttachmentInfo;
    onclose: () => void;
    onsaved?: (name: string) => Promise<void> | void;
  }

  let { entryUuid, attachment, onclose, onsaved }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

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
    error = key === null ? t(lang, "attachment.sessionChanged") : "";
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
  description={`${formatBytes(attachment.size)}${preview?.truncated ? t(lang, "attachment.truncatedSuffix") : ""}`}
  size="medium"
  scrollable
  closeOnEscape={!importing}
  onclose={close}
>
  {#snippet children()}
    {#if loading}
      <p class="preview-note">{t(lang, "attachment.loading")}</p>
    {:else if error}
      <p class="preview-note error">{error}</p>
    {:else if preview?.kind === "image"}
      <img class="preview-image" src={preview.data} alt={attachment.name} />
    {:else if preview?.kind === "text"}
      <pre class="preview-text">{preview.data}</pre>
    {:else}
      <p class="preview-note">
        {t(lang, "attachment.binary")}
      </p>
    {/if}
    {#if externalError}
      <p class="preview-note error">{t(lang, "attachment.openFailed", { error: externalError })}</p>
    {/if}
    {#if tempRef}
      <p class="preview-note">
        {t(lang, "attachment.openedNote")}
      </p>
    {/if}
  {/snippet}
  {#snippet actions()}
    <Button onclick={close} disabled={importing}>{t(lang, "common.close")}</Button>
    {#if tempRef}
      <Button onclick={() => void importChanges()} disabled={importing}>
        {importing ? t(lang, "attachment.importing") : t(lang, "attachment.importChanges")}</Button
      >
      <Button onclick={discardTemp} disabled={importing}>{t(lang, "attachment.discard")}</Button>
    {/if}
    <Button
      variant={!tempRef ? "primary" : "plain"}
      onclick={() => void openExternal()}
      disabled={opening || importing}
    >
      {confirmExternal
        ? t(lang, "attachment.confirmExternal")
        : tempRef
          ? t(lang, "attachment.reopen")
          : t(lang, "attachment.openExternal")}</Button
    >
    <Button
      variant="primary"
      onclick={() => void saveToDisk()}
      disabled={opening || importing || savingToDisk}
    >
      {savingToDisk ? t(lang, "attachment.saving") : t(lang, "attachment.saveTo")}</Button
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
