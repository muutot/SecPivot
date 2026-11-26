<script lang="ts">
  import { onDestroy } from "svelte";
  import { get } from "svelte/store";
  import { vault } from "$lib/services/vault";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { KdfOpenPromptController, type KdfOpenPrompt } from "$lib/services/kdf-open-prompt";
  import KdfUpgradeDialog from "$lib/components/KdfUpgradeDialog.svelte";
  import MasterKeyExpiryDialog from "$lib/components/MasterKeyExpiryDialog.svelte";
  import {
    MasterKeyOpenPromptController,
    type MasterKeyOpenPrompt,
  } from "$lib/services/master-key-open-prompt";
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
  const masterController = new MasterKeyOpenPromptController({
    enabled: isTauriRuntime,
    inspect: (id) => vault.callInSession(id, () => vault.getMasterKeyExpiryOffer()),
    close: async (owner) => {
      if (vault.getSessionEpoch(owner.sessionId) !== owner.epoch)
        throw new Error(t(get(appSettings).general.language, "security.keyExpiryChanged"));
      await vault.closeTab(owner.sessionId);
    },
    error: String,
  });
  let masterPrompt = $state<MasterKeyOpenPrompt | null>(null);
  const unsubscribeMaster = masterController.state.subscribe((value) => (masterPrompt = value));
  const unsubscribeReady = masterController.ready.subscribe((owner) => controller.activate(owner));
  let prompt = $state<KdfOpenPrompt | null>(null);
  const unsubscribePrompt = controller.state.subscribe((value) => (prompt = value));
  const unsubscribeVault = vault.subscribe((value) => {
    const id = vault.getActiveSessionId();
    const epoch = id ? vault.getSessionEpoch(id) : undefined;
    masterController.activate(value && id && epoch ? { sessionId: id, epoch } : null);
  });
  const unsubscribeTabs = vault.tabs.subscribe((tabs) => {
    const ids = tabs.map((tab) => tab.sessionId);
    controller.retain(ids);
    masterController.retain(ids);
  });
  onDestroy(() => {
    unsubscribeVault();
    unsubscribeTabs();
    masterController.dispose();
    unsubscribeMaster();
    unsubscribeReady();
    controller.dispose();
    unsubscribePrompt();
  });
</script>

{#if masterPrompt}
  {#key masterPrompt.owner.epoch}
    <MasterKeyExpiryDialog
      prompt={masterPrompt}
      ondecline={() => masterController.decline()}
      onchange={() => masterController.change()}
      onback={() => masterController.back()}
      onretry={() => masterController.retry()}
      onclose={() => void masterController.close()}
      onchanged={masterController.changed.bind(masterController, masterPrompt.owner)}
      onbusychange={masterController.setBusy.bind(masterController, masterPrompt.owner)}
    />
  {/key}
{/if}

{#if prompt}
  <KdfUpgradeDialog
    offer={prompt.offer}
    busy={prompt.busy}
    error={prompt.error}
    ondecline={() => controller.dismiss()}
    onupgrade={() => void controller.upgrade()}
  />
{/if}
