<script lang="ts">
  import { onDestroy } from "svelte";
  import { get } from "svelte/store";
  import { vault } from "$lib/services/vault";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { KdfOpenPromptController, type KdfOpenPrompt } from "$lib/services/kdf-open-prompt";
  import KdfUpgradeDialog from "$lib/components/KdfUpgradeDialog.svelte";
  import { t } from "$lib/i18n";

  const controller = new KdfOpenPromptController({
    enabled: () => isTauriRuntime() && get(appSettings).security.keyTransformWeakWarning,
    inspect: (id) => vault.callInSession(id, () => vault.getKdfUpgradeOffer()),
    upgrade: (id, offer) => vault.callInSession(id, () => vault.upgradeWeakKdf(offer)),
    error: (error) =>
      String(error).includes("KDF_UPGRADE_CHANGED:")
        ? t(get(appSettings).general.language, "kdf.upgradeChanged")
        : String(error),
  });
  let prompt = $state<KdfOpenPrompt | null>(null);
  const unsubscribePrompt = controller.state.subscribe((value) => (prompt = value));
  const unsubscribeVault = vault.subscribe((value) => {
    const id = vault.getActiveSessionId();
    const epoch = id ? vault.getSessionEpoch(id) : undefined;
    controller.activate(value && id && epoch ? { sessionId: id, epoch } : null);
  });
  const unsubscribeTabs = vault.tabs.subscribe((tabs) =>
    controller.retain(tabs.map((tab) => tab.sessionId)),
  );
  onDestroy(() => {
    unsubscribeVault();
    unsubscribeTabs();
    controller.dispose();
    unsubscribePrompt();
  });
</script>

{#if prompt}
  <KdfUpgradeDialog
    offer={prompt.offer}
    busy={prompt.busy}
    error={prompt.error}
    ondecline={() => controller.dismiss()}
    onupgrade={() => void controller.upgrade()}
  />
{/if}
