<script lang="ts">
  import type { VaultGroup } from "$lib/types/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  interface Props {
    group: VaultGroup;
    onclose: () => void;
    onsaved: (meta: {
      notes?: string;
      tags?: string;
      enableSearching?: boolean;
    }) => Promise<boolean>;
  }

  let { group, onclose, onsaved }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  // The dialog is mounted per open, so capturing the initial group is intended.
  // svelte-ignore state_referenced_locally
  let notes = $state(group.notes ?? "");
  // svelte-ignore state_referenced_locally
  let tags = $state(group.tags ?? "");
  // svelte-ignore state_referenced_locally
  let enableSearching = $state(group.enableSearching);
  let saving = $state(false);

  async function submit(): Promise<void> {
    if (saving) return;
    saving = true;
    try {
      const current = await onsaved({ notes, tags, enableSearching });
      if (current) onclose();
    } finally {
      saving = false;
    }
  }
</script>

<ModalShell
  title={t(lang, "groupmeta.title")}
  description={group.name}
  size="small"
  closeOnEscape={!saving}
  {onclose}
>
  {#snippet children()}
    <div class="block">
      <span class="label">{t(lang, "groupmeta.notes")}</span>
      <TextField multiline rows={3} bind:value={notes} placeholder={t(lang, "groupmeta.notesPh")} />
    </div>
    <div class="block">
      <span class="label">{t(lang, "groupmeta.tags")}</span>
      <TextField bind:value={tags} placeholder="work, dev" />
    </div>
    <div class="block">
      <span class="label">{t(lang, "groupmeta.search")}</span>
      <button
        type="button"
        class="toggle"
        class:active={enableSearching}
        onclick={() => (enableSearching = !enableSearching)}
        aria-pressed={enableSearching}
      >
        {enableSearching ? t(lang, "groupmeta.searchOn") : t(lang, "groupmeta.searchOff")}
      </button>
      <p class="hint">{t(lang, "groupmeta.hint")}</p>
    </div>
  {/snippet}
  {#snippet actions()}
    <Button onclick={onclose} disabled={saving}>{t(lang, "common.cancel")}</Button>
    <Button variant="primary" onclick={() => void submit()} disabled={saving}>
      {saving ? t(lang, "groupautotype.saving") : t(lang, "common.save")}
    </Button>
  {/snippet}
</ModalShell>

<style>
  .block {
    margin-bottom: 12px;
  }

  .label {
    display: block;
    margin-bottom: 6px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }

  .toggle {
    height: 28px;
    padding: 0 12px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .toggle:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .toggle.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .hint {
    margin: 6px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }
</style>
