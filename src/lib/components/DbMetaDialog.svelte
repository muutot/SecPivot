<script lang="ts">
  import { onDestroy } from "svelte";
  import { vault } from "$lib/services/vault";
  import { KeyedViewGuard, sessionResourceKey } from "$lib/utils/session-state";
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
  const sessionId = vault.getActiveSessionId();
  const dialogView = new KeyedViewGuard();
  dialogView.activate(sessionId ? sessionResourceKey(sessionId, "database-meta") : null);

  onDestroy(() => dialogView.activate(null));

  async function save(): Promise<void> {
    if (saving || !sessionId) return;
    const view = dialogView.capture();
    if (!view) return;
    saving = true;
    error = "";
    try {
      await vault.callInSession(sessionId, () => vault.updateDbMeta(dbName, dbDescription));
      if (!dialogView.isCurrent(view)) return;
      onclose();
    } catch (e) {
      if (dialogView.isCurrent(view)) error = e instanceof Error ? e.message : String(e);
    } finally {
      if (dialogView.isCurrent(view)) saving = false;
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    // Enter-to-save applies only to the name input (bound above); the
    // description textarea must keep Enter for newlines.
    if (event.key !== "Enter") return;
    const target = event.target as HTMLElement | null;
    if (target?.id === "db-name") void save();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<ModalShell
  title="数据库属性"
  description="库名称与描述写入 KDBX 元数据"
  size="small"
  showClose={!saving}
  closeOnEscape={!saving}
  {onclose}
>
  {#snippet icon()}<AppIcon name="database" size={18} />{/snippet}
  {#snippet children()}
    <div class="field-row">
      <label for="db-name">库名称</label>
      <input
        id="db-name"
        class="text-input"
        bind:value={dbName}
        placeholder="留空则清除"
        maxlength="128"
        onkeydown={(e) => {
          if (e.key === "Enter") void save();
        }}
      />
    </div>

    <div class="field-row">
      <label for="db-description">描述</label>
      <textarea
        id="db-description"
        class="text-input textarea"
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
    box-sizing: border-box;
    resize: vertical;
  }

  .field-row input:focus,
  .field-row textarea:focus {
    outline: none;
  }

  .error-msg {
    margin: 0 0 10px;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
  }
</style>
