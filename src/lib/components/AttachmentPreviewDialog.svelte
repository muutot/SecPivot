<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import type { AttachmentInfo, AttachmentPreview } from "$lib/types/vault";
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

  $effect(() => {
    void vault
      .previewAttachment(entryUuid, attachment.name)
      .then((value) => {
        preview = value;
        loading = false;
      })
      .catch((e) => {
        error = String(e);
        loading = false;
      });
  });

  async function saveToDisk(): Promise<void> {
    const dest = await save({ defaultPath: attachment.name });
    if (!dest) return;
    await vault.saveAttachment(entryUuid, attachment.name, dest);
    await onsaved?.(attachment.name);
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
  {/snippet}
  {#snippet actions()}
    <button class="modal-button" onclick={onclose}>关闭</button>
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
