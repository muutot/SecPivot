<script lang="ts">
  import ModalShell from "$lib/components/ModalShell.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  interface Props {
    oncancel: () => void;
    oncontinue: () => void;
  }

  let { oncancel, oncontinue }: Props = $props();
  const lang = $derived($appSettings.general.language);
</script>

<ModalShell
  title={t(lang, "kdf.weakTitle")}
  description={t(lang, "kdf.weakDescription")}
  size="confirm"
  prompt
  scrollable
  closeOnEscape
  onclose={oncancel}
>
  {#snippet children()}
    <p class="threshold-note">{t(lang, "kdf.weakThreshold")}</p>
  {/snippet}
  {#snippet actions()}
    <Button onclick={oncancel}>{t(lang, "common.cancel")}</Button>
    <Button variant="warning" onclick={oncontinue}>{t(lang, "kdf.keepWeak")}</Button>
  {/snippet}
</ModalShell>

<style>
  .threshold-note {
    margin: 0;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
  }
</style>
