<script lang="ts">
  import { vault } from "$lib/services/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";

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
    if (event.key === "Escape") onclose();
    else if (event.key === "Enter") void save();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="modal-backdrop" role="presentation">
  <div class="meta-modal" role="dialog" aria-modal="true" aria-label="数据库属性">
    <div class="modal-head">
      <span class="modal-icon"><AppIcon name="database" size={18} /></span>
      <div>
        <strong>数据库属性</strong>
        <p>库名称与描述写入 KDBX 元数据</p>
      </div>
      <button class="close-button" onclick={onclose} title="关闭" aria-label="关闭"
        ><AppIcon name="x" size={14} /></button
      >
    </div>

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

    <div class="modal-actions">
      <button class="btn-secondary" onclick={onclose} disabled={saving}>取消</button>
      <button class="btn-primary" onclick={() => void save()} disabled={saving}
        >{saving ? "保存中…" : "保存"}</button
      >
    </div>
  </div>
</div>

<style>
  .meta-modal {
    width: 360px;
    max-width: calc(100vw - 48px);
    max-height: calc(100vh - 64px);
    overflow: auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 10px);
    background: var(--bg-primary);
    box-shadow: 0 12px 40px color-mix(in srgb, #000 35%, transparent);
    padding: 16px;
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
    width: 30px;
    height: 30px;
    border-radius: var(--settings-control-radius, 8px);
    color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 14%, transparent);
  }

  .modal-head strong {
    display: block;
    color: var(--text-primary);
    font-size: var(--font-size-primary, 13px);
  }

  .modal-head p {
    margin: 2px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .close-button {
    margin-left: auto;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
    cursor: pointer;
  }

  .close-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

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

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 4px;
  }

  .btn-secondary,
  .btn-primary {
    padding: 6px 14px;
    border-radius: var(--settings-control-radius, 6px);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .btn-secondary {
    border: 1px solid var(--border-color);
    color: var(--text-secondary);
    background: transparent;
  }

  .btn-secondary:hover {
    background: var(--hover-bg);
  }

  .btn-primary {
    border: 1px solid var(--selection-color);
    color: var(--on-selection-color, #fff);
    background: var(--selection-color);
  }

  .btn-primary:hover {
    filter: brightness(1.05);
  }

  .btn-primary:disabled,
  .btn-secondary:disabled {
    opacity: 0.55;
    cursor: default;
  }
</style>
