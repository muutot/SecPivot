<script lang="ts">
  import { onDestroy } from "svelte";
  import type { VaultGroup, GroupAutoTypeConfig } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import { KeyedViewGuard, sessionResourceKey } from "$lib/utils/session-state";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import RadioGroup from "$lib/components/templates/form/RadioGroup.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  interface Props {
    group: VaultGroup;
    onclose: () => void;
  }

  let { group, onclose }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

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
  title={t(lang, "groupautotype.title")}
  description={group.name}
  size="small"
  closeOnEscape={!saving}
  {onclose}
>
  {#snippet children()}
    <div class="choice-row">
      <RadioGroup
        size="compact"
        bind:value={enableChoice}
        options={[
          { value: "inherit", label: t(lang, "groupautotype.inherit") },
          { value: "on", label: t(lang, "groupautotype.enable") },
          { value: "off", label: t(lang, "groupautotype.disable") },
        ]}
        ariaLabel={t(lang, "groupautotype.status")}
      />
    </div>
    <label class="field">
      <span>{t(lang, "editor.defaultSequence")}</span>
      <TextField mono bind:value={defaultSeq} placeholder={"{USERNAME}{TAB}{PASSWORD}{ENTER}"} />
    </label>
    {#if error}<p class="dialog-error">{error}</p>{/if}
  {/snippet}
  {#snippet actions()}
    <Button onclick={onclose} disabled={saving}>{t(lang, "common.cancel")}</Button>
    <Button variant="primary" onclick={() => void save()} disabled={saving}>
      {saving ? t(lang, "groupautotype.saving") : t(lang, "common.save")}
    </Button>
  {/snippet}
</ModalShell>

<style>
  .choice-row {
    margin-bottom: 14px;
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
