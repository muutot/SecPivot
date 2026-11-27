<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  import type { BitwardenUriMatch, BitwardenUriSettings } from "$lib/types/vault";
  import { parseBitwardenUriSettings } from "$lib/utils/bitwarden-uri-settings";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import Select from "$lib/components/Select.svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Feedback from "$lib/components/templates/form/Feedback.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  interface Props {
    onconfirm: (settings: BitwardenUriSettings) => void;
    onclose: () => void;
  }
  let { onconfirm, onclose }: Props = $props();
  let defaultMatch = $state<BitwardenUriMatch>(0);
  let domains = $state("");
  let error = $state(false);
  const lang = $derived($appSettings.general.language);
  const options = $derived([
    { value: 0 as BitwardenUriMatch, label: t(lang, "io.bitwardenDomain") },
    { value: 1 as BitwardenUriMatch, label: t(lang, "editor.websiteHost") },
    { value: 2 as BitwardenUriMatch, label: t(lang, "editor.websitePrefix") },
    { value: 3 as BitwardenUriMatch, label: t(lang, "editor.websiteExact") },
    { value: 4 as BitwardenUriMatch, label: t(lang, "editor.websiteRegex") },
    { value: 5 as BitwardenUriMatch, label: t(lang, "editor.websiteNever") },
  ]);
  function confirm(): void {
    try {
      const settings = parseBitwardenUriSettings(defaultMatch, domains);
      error = false;
      onconfirm(settings);
    } catch {
      error = true;
    }
  }
</script>

<ModalShell
  title={t(lang, "io.bitwardenSettingsTitle")}
  description={t(lang, "io.bitwardenSettingsHint")}
  size="medium"
  scrollable
  showClose
  closeOnEscape
  {onclose}
>
  {#snippet icon()}<AppIcon name="globe" size={20} />{/snippet}
  <div class="source-settings">
    <label
      ><span>{t(lang, "io.bitwardenDefaultMatch")}</span><Select
        value={defaultMatch}
        {options}
        ariaLabel={t(lang, "io.bitwardenDefaultMatch")}
        onchange={(value) => {
          defaultMatch = value;
          error = false;
        }}
      /></label
    >
    <p>{t(lang, "io.bitwardenExplicitHint")}</p>
    <label
      ><span>{t(lang, "io.bitwardenEquivalentDomains")}</span><TextField
        multiline
        rows={4}
        bind:value={domains}
        ariaLabel={t(lang, "io.bitwardenEquivalentDomains")}
        placeholder="example.com, example.net"
        spellcheck={false}
        maxlength={32768}
        oninput={() => (error = false)}
      /></label
    >
    <p>{t(lang, "io.bitwardenDomainsHint")}</p>
    <p>{t(lang, "io.bitwardenDomainsMissing")}</p>
    {#if error}<Feedback inline message={t(lang, "io.bitwardenSettingsInvalid")} />{/if}
  </div>
  {#snippet actions()}<Button onclick={onclose}>{t(lang, "common.cancel")}</Button><Button
      variant="primary"
      onclick={confirm}>{t(lang, "io.bitwardenConfirmImport")}</Button
    >{/snippet}
</ModalShell>

<style>
  .source-settings {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  span,
  p {
    font-size: var(--font-size-secondary, 11px);
    color: var(--text-secondary);
    overflow-wrap: anywhere;
  }
  p {
    margin: 0;
    color: var(--text-muted);
    line-height: 1.5;
  }
</style>
