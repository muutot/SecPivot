<script lang="ts">
  import { onDestroy } from "svelte";
  import { get } from "svelte/store";
  import { open } from "@tauri-apps/plugin-dialog";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { rememberCredential } from "$lib/services/security";
  import { vault } from "$lib/services/vault";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import MasterPasswordFields from "$lib/components/MasterPasswordFields.svelte";
  import KdfWarningDialog from "$lib/components/KdfWarningDialog.svelte";
  import { isWeakKdfParameters } from "$lib/utils/kdf-parameters";
  import { SessionViewGuard, type SessionViewToken } from "$lib/utils/session-state";
  import { passwordRepeatMatches } from "$lib/utils/password-repeat";
  import Button from "$lib/components/templates/action/Button.svelte";
  import Feedback from "$lib/components/templates/form/Feedback.svelte";
  import { t } from "$lib/i18n";
  interface Props {
    card?: boolean;
    expiryToken?: string;
    onchanged?: () => void;
    onbusychange?: (busy: boolean) => void;
  }
  let { card = true, expiryToken, onchanged, onbusychange }: Props = $props();
  const lang = $derived($appSettings.general.language);
  let newPassword = $state("");
  let confirmPassword = $state("");
  let showPassword = $state(false);
  let newKeyfile = $state<string | null>(null);
  let busy = $state(false);
  let feedback = $state<{ ok: boolean; message: string } | null>(null);
  let showWeakWarning = $state(false);
  let weakWarningSignature = $state("");
  let weakMasterApproval = $state<string | null>(null);
  let weakWarningView = $state<SessionViewToken | null>(null);
  const masterView = new SessionViewGuard();
  let masterEpoch: symbol | undefined;
  const unsubscribeVault = vault.subscribe(() => {
    const previous = masterView.capture();
    const id = vault.getActiveSessionId();
    const epoch = id ? vault.getSessionEpoch(id) : undefined;
    if (epoch !== masterEpoch) {
      masterView.activate(null);
      masterEpoch = epoch;
    }
    masterView.activate(id);
    if (previous && !masterView.isCurrent(previous)) {
      newPassword = "";
      confirmPassword = "";
      showPassword = false;
      newKeyfile = null;
      feedback = null;
    }
    if (weakWarningView && !masterView.isCurrent(weakWarningView)) {
      showWeakWarning = false;
      weakMasterApproval = null;
      weakWarningView = null;
    }
  });
  onDestroy(() => {
    unsubscribeVault();
    masterView.activate(null);
    newPassword = "";
    confirmPassword = "";
    newKeyfile = null;
  });

  async function pickKeyfile(): Promise<void> {
    if (!isTauriRuntime()) return;
    const view = masterView.capture();
    if (!view) return;
    const picked = await open({
      multiple: false,
      directory: false,
    });
    if (masterView.isCurrent(view) && typeof picked === "string" && picked.length > 0) {
      newKeyfile = picked;
    }
  }

  async function submitMasterKeyChange(): Promise<void> {
    if (busy || showWeakWarning) return;
    feedback = null;
    if (
      !passwordRepeatMatches(
        newPassword,
        confirmPassword,
        showPassword,
        get(appSettings).security.repeatPasswordOnlyWhenHidden,
      )
    ) {
      feedback = { ok: false, message: t(lang, "security.mismatch") };
      return;
    }
    if (newPassword.length === 0 && !newKeyfile) {
      feedback = { ok: false, message: t(lang, "security.needKey") };
      return;
    }
    busy = true;
    const sessionId = vault.getActiveSessionId();
    if (!sessionId) {
      busy = false;
      feedback = { ok: false, message: t(lang, "security.noVault") };
      return;
    }
    const view = masterView.capture();
    if (!view) {
      busy = false;
      return;
    }
    let submittedPassword = newPassword;
    const submittedKeyfile = newKeyfile;
    const submittedToken = expiryToken;
    try {
      if (get(appSettings).security.keyTransformWeakWarning) {
        const database = await vault.callInSession(sessionId, () => vault.getDatabaseSettings());
        if (!masterView.isCurrent(view)) return;
        if (database && isWeakKdfParameters(database.kdfParameters)) {
          const signature = JSON.stringify({ sessionId, parameters: database.kdfParameters });
          if (weakMasterApproval !== signature) {
            weakWarningSignature = signature;
            weakWarningView = view;
            showWeakWarning = true;
            return;
          }
        }
      }
      weakMasterApproval = null;
      const state = await vault.callInSession(sessionId, () =>
        vault.changeMasterKey(submittedPassword, submittedKeyfile, submittedToken),
      );
      if (!masterView.isCurrent(view)) return;
      if (state.path && submittedPassword.length > 0 && !submittedKeyfile) {
        // A stored password alone can never open a keyfile-protected vault;
        // don't remember one when the change introduced (or kept) a keyfile.
        await rememberCredential(state.path, submittedPassword);
        if (!masterView.isCurrent(view)) return;
      }
      newPassword = "";
      confirmPassword = "";
      showPassword = false;
      newKeyfile = null;
      feedback = { ok: true, message: t(lang, "security.changed") };
      onchanged?.();
    } catch (err) {
      if (!masterView.isCurrent(view)) return;
      feedback = {
        ok: false,
        message: String(err).includes("MASTER_KEY_EXPIRY_CHANGED:")
          ? t(lang, "security.keyExpiryChanged")
          : String(err),
      };
    } finally {
      submittedPassword = "";
      busy = false;
    }
  }

  function continueWeakMasterKey(): void {
    if (!weakWarningView || !masterView.isCurrent(weakWarningView)) {
      showWeakWarning = false;
      return;
    }
    weakMasterApproval = weakWarningSignature;
    showWeakWarning = false;
    void submitMasterKeyChange();
  }

  $effect(() => {
    onbusychange?.(busy || showWeakWarning);
  });
</script>

<section class:setting-card={card}>
  {#if card}
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="unlock" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>{t(lang, "security.changeKey")}</strong>
          <p>{t(lang, "security.changeKeyDesc")}</p>
        </div>
      </div>
    </div>
  {/if}
  <MasterPasswordFields
    size="control"
    repeat
    {busy}
    bind:password={newPassword}
    bind:confirm={confirmPassword}
    bind:showPassword
    passwordLabel={t(lang, "security.newPasswordLabel")}
    passwordPlaceholder={t(lang, "security.newPasswordPh")}
    confirmLabel={t(lang, "security.confirmPasswordLabel")}
    confirmPlaceholder={t(lang, "security.confirmPasswordPh")}
  />
  <div class="setting-row mk-keyfile-row">
    <span class="mk-keyfile-name" class:mk-empty={!newKeyfile}>
      {newKeyfile ?? t(lang, "security.noKeyfile")}
    </span>
    <Button variant="field" onclick={pickKeyfile} disabled={busy || !isTauriRuntime()}>
      {t(lang, "vault.pickKeyfileTitle")}
    </Button>
    {#if newKeyfile}
      <Button variant="field" onclick={() => (newKeyfile = null)} disabled={busy}
        >{t(lang, "common.clear")}</Button
      >
    {/if}
  </div>
  <div class="mk-submit-row">
    <Button
      variant={expiryToken ? "warning" : "primary"}
      onclick={submitMasterKeyChange}
      disabled={busy}
      {busy}
    >
      {busy ? t(lang, "security.changing") : t(lang, "security.changeAndSave")}
    </Button>
  </div>
  {#if feedback}
    <Feedback inline success={feedback.ok} message={feedback.message} />
  {/if}
</section>
{#if showWeakWarning}
  <KdfWarningDialog oncancel={() => (showWeakWarning = false)} oncontinue={continueWeakMasterKey} />
{/if}

<style>
  .mk-keyfile-row {
    margin-top: 12px;
  }

  .mk-keyfile-name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
  }

  .mk-keyfile-name.mk-empty {
    color: var(--text-faint);
  }

  .mk-submit-row {
    margin-top: 14px;
    display: flex;
    flex-direction: column;
  }
</style>
