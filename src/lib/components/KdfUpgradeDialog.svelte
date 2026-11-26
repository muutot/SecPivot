<script lang="ts">
  import type { KdfUpgradeOffer } from "$lib/types/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import Feedback from "$lib/components/templates/form/Feedback.svelte";
  import { appSettings } from "$lib/services/settings";
  import { memoryMiB } from "$lib/utils/kdf-parameters";
  import { t } from "$lib/i18n";

  interface Props {
    offer: KdfUpgradeOffer;
    busy: boolean;
    error: string;
    ondecline: () => void;
    onupgrade: () => void;
  }
  let { offer, busy, error, ondecline, onupgrade }: Props = $props();
  const lang = $derived($appSettings.general.language);
</script>

{#snippet costs(parameters: KdfUpgradeOffer["parameters"])}
  {#if parameters.algorithm === "Aes"}
    {t(lang, "kdf.aesCost", { rounds: BigInt(parameters.rounds).toLocaleString(lang) })}
  {:else}
    {t(lang, "kdf.argonCost", {
      algorithm: parameters.algorithm,
      iterations: parameters.iterations,
      memory: memoryMiB(parameters.memoryBytes),
      lanes: parameters.parallelism,
      version: parameters.version === 16 ? "1.0" : "1.3",
    })}
  {/if}
{/snippet}

<ModalShell
  title={t(lang, "kdf.weakTitle")}
  description={t(lang, "kdf.openUpgradeDesc")}
  size="confirm"
  prompt
  scrollable
  closeOnEscape={!busy}
  onclose={ondecline}
>
  {#snippet children()}
    <p class="database-name">{offer.fileName}</p>
    <div class="cost-summary">
      <p><strong>{t(lang, "kdf.currentCost")}</strong> {@render costs(offer.parameters)}</p>
      {#if offer.recommended}
        <p><strong>{t(lang, "kdf.recommendedCost")}</strong> {@render costs(offer.recommended)}</p>
      {/if}
      {#if offer.readOnly}<p>{t(lang, "kdf.upgradeReadOnly")}</p>{/if}
    </div>
    {#if error}<Feedback inline message={error} />{/if}
  {/snippet}
  {#snippet actions()}
    <Button disabled={busy} onclick={ondecline}>{t(lang, "kdf.upgradeLater")}</Button>
    <Button variant="warning" disabled={busy || offer.readOnly} {busy} onclick={onupgrade}>
      {t(lang, "kdf.applyRecommended")}
    </Button>
  {/snippet}
</ModalShell>

<style>
  .database-name {
    margin: 0 0 8px;
    color: var(--text-primary);
    font-size: var(--font-size-secondary, 11px);
    overflow-wrap: anywhere;
  }
  .cost-summary {
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    overflow-wrap: anywhere;
  }
  .cost-summary p {
    margin: 0 0 6px;
  }
</style>
