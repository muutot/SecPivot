<script lang="ts">
  import { vault } from "$lib/services/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";

  interface Props {
    name: string;
    description: string;
    onclose: () => void;
  }

  let { name, description, onclose }: Props = $props();

  // The dialog is mounted per open (dbMetaOpen), so the meta props never
  // change during an instance's lifetime; capturing them once is intentional.
  // svelte-ignore state_referenced_locally
  let dbName = $state(name);
  // svelte-ignore state_referenced_locally
  let dbDescription = $state(description);
  let saving = $state(false);
  let error = $state("");

  async function save(): Promise<void> {
    if (saving) return;
    saving = true;
    error = "";
    try {
      await vault.updateDbMeta(dbName, dbDescription);
      onclose();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Enter") void save();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<ModalShell
  title="数据库属性"
  description="库名称与描述写入 KDBX 元数据"
  size="small"
  showClose
  closeOnEscape
  {onclose}
>
  {#snippet icon()}<AppIcon name="database" size={18} />{/snippet}
  {#snippet children()}
    <div class="field-row">
      <label for="db-name">库名称</label>
      <input id="db-name" bind:value={dbName} placeholder="留空则清除" maxlength="128" />
    </div>

    <div class="field-row">
      <label for="db-description">描述</label>
      <textarea
        id="db-description"
        bind:value={dbDescription}
        placeholder="留空则清除"
        rows="4"
        maxlength="1024"></textarea>
    </div>

    {#if error}<p class="error-msg">{error}</p>{/if}
  {/snippet}
  {#snippet actions()}
    <button class="modal-button" onclick={onclose} disabled={saving}>取消</button>
    <button class="modal-button primary" onclick={() => void save()} disabled={saving}
      >{saving ? "保存中…" : "保存"}</button
    >
  {/snippet}
</ModalShell>

<style>
  .field-row {
    margin-bottom: 12px;
  }

  .field-row label {
    display: block;
    margin-bottom: 4px;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
  }

  .field-row input,
  .field-row textarea {
    width: 100%;
    box-sizing: border-box;
    padding: 6px 8px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--bg-input, var(--bg-secondary));
    font-size: var(--font-size-primary, 13px);
    resize: vertical;
  }

  .field-row input:focus,
  .field-row textarea:focus {
    outline: none;
    border-color: var(--selection-color);
  }

  .error-msg {
    margin: 0 0 10px;
    color: var(--warning-color, var(--danger-color, #e5484d));
    font-size: var(--font-size-secondary, 11px);
  }
</style>
