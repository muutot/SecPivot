<script lang="ts">
  import type { MasterKeyOpenPrompt } from "$lib/services/master-key-open-prompt";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import MasterKeyChangeForm from "$lib/components/MasterKeyChangeForm.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import Feedback from "$lib/components/templates/form/Feedback.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  interface Props {
    prompt: MasterKeyOpenPrompt;
    ondecline: () => void;
    onchange: () => void;
    onback: () => void;
    onretry: () => void;
    onclose: () => void;
    onchanged: () => void;
    onbusychange: (busy: boolean) => void;
  }
  let { prompt, ondecline, onchange, onback, onretry, onclose, onchanged, onbusychange }: Props =
    $props();
  const lang = $derived($appSettings.general.language);
  const checking = $derived(prompt.offer === null);
  const required = $derived(prompt.offer?.required ?? true);
  const changing = $derived(prompt.phase === "change");
</script>

<ModalShell
  title={t(
    lang,
    checking
      ? "security.keyExpiryInspect"
      : required
        ? "security.keyExpiryRequired"
        : "security.keyExpiryRecommended",
  )}
  description={t(
    lang,
    checking
      ? "security.keyExpiryInspectDesc"
      : required
        ? "security.keyExpiryRequiredDesc"
        : "security.keyExpiryRecommendedDesc",
  )}
  size={changing ? "small" : "confirm"}
  prompt
  scrollable
  closeOnEscape={!prompt.busy && (!required || changing)}
  onclose={changing ? onback : ondecline}
>
  {#snippet children()}
    {#if prompt.offer}
      <p class="database-name">{prompt.offer.fileName}</p>
      {#if prompt.offer.changedAt === null}<p class="expiry-note">
          {t(lang, "security.keyExpiryUnknownDate")}
        </p>{/if}
      {#if prompt.offer.invalidRules.length > 0}<p class="expiry-note">
          {t(lang, "security.keyExpiryInvalidRules")}
        </p>{/if}
      {#if prompt.offer.readOnly}<p class="expiry-note">
          {t(lang, "security.keyExpiryReadOnly")}
        </p>{/if}
      {#if changing}
        <MasterKeyChangeForm
          card={false}
          expiryToken={prompt.offer.token}
          {onchanged}
          {onbusychange}
        />
      {/if}
    {/if}
    {#if prompt.error}<Feedback inline message={prompt.error} />{/if}
  {/snippet}
  {#snippet actions()}
    {#if changing}
      <Button disabled={prompt.busy} onclick={onback}>{t(lang, "security.keyExpiryBack")}</Button>
    {:else}
      {#if !required}<Button disabled={prompt.busy} onclick={ondecline}
          >{t(lang, "security.keyExpiryLater")}</Button
        >{/if}
      <Button disabled={prompt.busy} onclick={onclose}>{t(lang, "security.keyExpiryClose")}</Button>
      {#if prompt.phase === "error"}<Button variant="warning" onclick={onretry}
          >{t(lang, "security.keyExpiryRetry")}</Button
        >
      {:else if prompt.offer && (prompt.offer.required || prompt.offer.recommended)}
        <Button variant="warning" disabled={prompt.busy || prompt.offer.readOnly} onclick={onchange}
          >{t(lang, "security.keyExpiryChange")}</Button
        >
      {/if}
    {/if}
  {/snippet}
</ModalShell>

<style>
  .database-name {
    margin: 0 0 8px;
    color: var(--text-primary);
    font-size: var(--font-size-secondary, 11px);
    overflow-wrap: anywhere;
  }
  .expiry-note {
    margin: 0 0 8px;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    overflow-wrap: anywhere;
  }
</style>
