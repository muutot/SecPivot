<script lang="ts">
  import { onDestroy } from "svelte";
  import { vault } from "$lib/services/vault";
  import { KeyedViewGuard, sessionResourceKey } from "$lib/utils/session-state";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  interface Props {
    name: string;
    description: string;
    onclose: () => void;
  }

  let { name, description, onclose }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

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
  title={t(lang, "dbmeta.title")}
  description={t(lang, "dbmeta.desc")}
  size="small"
  showClose={!saving}
  closeOnEscape={!saving}
  {onclose}
>
  {#snippet icon()}<AppIcon name="database" size={18} />{/snippet}
  {#snippet children()}
    <div class="field-row">
      <label for="db-name">{t(lang, "dbmeta.nameLabel")}</label>
      <TextField
        id="db-name"
        bind:value={dbName}
        placeholder={t(lang, "dbmeta.namePh")}
        maxlength={128}
        onkeydown={(e) => {
          if (e.key === "Enter") void save();
        }}
      />
    </div>

    <div class="field-row">
      <label for="db-description">{t(lang, "dbmeta.descLabel")}</label>
      <TextField
        id="db-description"
        multiline
        rows={4}
        bind:value={dbDescription}
        placeholder={t(lang, "dbmeta.descPh")}
        maxlength={1024}
      />
    </div>

    {#if error}<p class="error-msg">{error}</p>{/if}
  {/snippet}
  {#snippet actions()}
    <Button onclick={onclose} disabled={saving}>{t(lang, "common.cancel")}</Button>
    <Button variant="primary" onclick={() => void save()} disabled={saving}>
      {saving ? t(lang, "dbmeta.saving") : t(lang, "common.save")}
    </Button>
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

  .error-msg {
    margin: 0 0 10px;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
  }
</style>
