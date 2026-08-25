<script lang="ts">
  import { onDestroy } from "svelte";
  import type { VaultGroup, GroupAutoTypeConfig } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import { KeyedViewGuard, sessionResourceKey } from "$lib/utils/session-state";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";

  interface Props {
    group: VaultGroup;
    onclose: () => void;
  }

  let { group, onclose }: Props = $props();

  type EnableChoice = "inherit" | "on" | "off";
  // The dialog is mounted per open, so the group config is captured once.
  // svelte-ignore state_referenced_locally
  let enableChoice = $state<EnableChoice>(
    group.autoType?.enabled === undefined ? "inherit" : group.autoType.enabled ? "on" : "off",
  );
  // svelte-ignore state_referenced_locally
  let defaultSeq = $state(group.autoType?.defaultSequence ?? "");
  let saving = $state(false);
  let error = $state("");
  const sessionId = vault.getActiveSessionId();
  const dialogView = new KeyedViewGuard();
  let activeKey: string | null = null;

  $effect(() => {
    const key = sessionId ? sessionResourceKey(sessionId, group.uuid) : null;
    if (key === activeKey) return;
    activeKey = key;
    dialogView.activate(key);
    enableChoice =
      group.autoType?.enabled === undefined ? "inherit" : group.autoType.enabled ? "on" : "off";
    defaultSeq = group.autoType?.defaultSequence ?? "";
    saving = false;
    error = "";
  });

  onDestroy(() => dialogView.activate(null));

  async function save(): Promise<void> {
    if (saving || !sessionId) return;
    const view = dialogView.capture();
    if (!view) return;
    saving = true;
    error = "";
    try {
      const input: GroupAutoTypeConfig = {};
      if (enableChoice !== "inherit") input.enabled = enableChoice === "on";
      input.defaultSequence = defaultSeq.trim();
      await vault.callInSession(sessionId, () => vault.updateGroupAutoType(group.uuid, input));
      if (!dialogView.isCurrent(view)) return;
      onclose();
    } catch (e) {
      if (dialogView.isCurrent(view)) error = String(e);
    } finally {
      if (dialogView.isCurrent(view)) saving = false;
    }
  }
</script>

<ModalShell
  title="自动填充设置"
  description={group.name}
  size="small"
  closeOnEscape={!saving}
  {onclose}
>
  {#snippet children()}
    <div class="choice-row" role="radiogroup" aria-label="自动填充启用状态">
      <button
        type="button"
        class="choice-option"
        class:active={enableChoice === "inherit"}
        onclick={() => (enableChoice = "inherit")}
      >
        继承
      </button>
      <button
        type="button"
        class="choice-option"
        class:active={enableChoice === "on"}
        onclick={() => (enableChoice = "on")}
      >
        启用
      </button>
      <button
        type="button"
        class="choice-option"
        class:active={enableChoice === "off"}
        onclick={() => (enableChoice = "off")}
      >
        禁用
      </button>
    </div>
    <label class="field">
      <span>默认序列</span>
      <TextField mono bind:value={defaultSeq} placeholder={"{USERNAME}{TAB}{PASSWORD}{ENTER}"} />
    </label>
    {#if error}<p class="dialog-error">{error}</p>{/if}
  {/snippet}
  {#snippet actions()}
    <Button onclick={onclose} disabled={saving}>取消</Button>
    <Button variant="primary" onclick={() => void save()} disabled={saving}>
      {saving ? "保存中…" : "保存"}
    </Button>
  {/snippet}
</ModalShell>

<style>
  .choice-row {
    display: flex;
    gap: 6px;
    margin-bottom: 14px;
  }

  .choice-option {
    flex: 1;
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .choice-option:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .choice-option.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .field > span {
    display: block;
    margin-bottom: 5px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .dialog-error {
    margin: 10px 0 0;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
  }
</style>
