<script lang="ts">
  import { onDestroy } from "svelte";
  import { get } from "svelte/store";
  import { open } from "@tauri-apps/plugin-dialog";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { rememberCredential } from "$lib/services/security";
  import { vault } from "$lib/services/vault";
  import type { SecuritySettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";
  import MasterPasswordFields from "$lib/components/MasterPasswordFields.svelte";
  import KdfWarningDialog from "$lib/components/KdfWarningDialog.svelte";
  import { isWeakKdfParameters } from "$lib/utils/kdf-parameters";
  import { SessionViewGuard, type SessionViewToken } from "$lib/utils/session-state";
  import { passwordRepeatMatches } from "$lib/utils/password-repeat";
  import Button from "$lib/components/templates/action/Button.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Feedback from "$lib/components/templates/form/Feedback.svelte";
  import SettingToggleCard from "$lib/components/settings/SettingToggleCard.svelte";
  import { t } from "$lib/i18n";

  interface Props {
    onclose: () => void;
    showHeader?: boolean;
  }

  let { onclose, showHeader = true }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const security = $derived(s.security);
  const lang = $derived(s.general.language);

  function change<K extends keyof SecuritySettings>(key: K, value: SecuritySettings[K]): void {
    appSettings.updateSecurity(key, value);
  }

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
  const unsubscribeVault = vault.subscribe(() => {
    masterView.activate(vault.getActiveSessionId());
    if (weakWarningView && !masterView.isCurrent(weakWarningView)) {
      showWeakWarning = false;
      weakMasterApproval = null;
      weakWarningView = null;
    }
  });
  onDestroy(() => {
    unsubscribeVault();
    masterView.activate(null);
  });

  async function pickKeyfile(): Promise<void> {
    if (!isTauriRuntime()) return;
    const picked = await open({
      multiple: false,
      directory: false,
    });
    if (typeof picked === "string" && picked.length > 0) {
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
        vault.changeMasterKey(newPassword, newKeyfile),
      );
      if (!masterView.isCurrent(view)) return;
      if (state.path && newPassword.length > 0 && !newKeyfile) {
        // A stored password alone can never open a keyfile-protected vault;
        // don't remember one when the change introduced (or kept) a keyfile.
        await rememberCredential(state.path, newPassword);
        if (!masterView.isCurrent(view)) return;
      }
      newPassword = "";
      confirmPassword = "";
      showPassword = false;
      newKeyfile = null;
      feedback = { ok: true, message: t(lang, "security.changed") };
    } catch (err) {
      if (!masterView.isCurrent(view)) return;
      feedback = { ok: false, message: String(err) };
    } finally {
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
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · {t(lang, "security.title")}</span>
      <h2>{t(lang, "security.title")}</h2>
      <p>{t(lang, "security.desc")}</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label={t(lang, "common.close")}>×</button>
  </header>
{/if}

<div class="settings-scroll">
  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="key" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>{t(lang, "security.masterKeyTries")}</strong>
          <p>{t(lang, "security.masterKeyTriesDesc")}</p>
        </div>
        <div class="master-key-tries-control">
          <TextField
            type="number"
            numeric
            size="control"
            min={1}
            max={2_147_483_647}
            step={1}
            value={String(security.masterKeyTries)}
            ariaLabel={t(lang, "security.masterKeyTries")}
            oninput={(event) => {
              const value = Number(event.currentTarget.value);
              if (Number.isInteger(value) && value >= 1 && value <= 2_147_483_647) {
                change("masterKeyTries", value);
              }
            }}
          />
        </div>
      </div>
    </div>
  </section>

  <SettingRangeCard
    icon="lock"
    label={t(lang, "security.autoLock")}
    description={t(lang, "security.autoLockDesc")}
    value={s.security.autoLockMinutes}
    valueLabel={s.security.autoLockMinutes > 0
      ? t(lang, "security.minutes", { count: s.security.autoLockMinutes })
      : t(lang, "settings.effectOff")}
    min={0}
    max={60}
    onchange={(value) => change("autoLockMinutes", value)}
  />

  <SettingRangeCard
    icon="clock"
    label={t(lang, "security.clipboardClear")}
    description={t(lang, "security.clipboardClearDesc")}
    value={s.security.clipboardClearSeconds}
    valueLabel={s.security.clipboardClearSeconds > 0
      ? t(lang, "security.seconds", { count: s.security.clipboardClearSeconds })
      : t(lang, "settings.effectOff")}
    min={0}
    max={120}
    step={5}
    onchange={(value) => change("clipboardClearSeconds", value)}
  />

  <SettingRangeCard
    icon="clock"
    label={t(lang, "security.defaultExpire")}
    description={t(lang, "security.defaultExpireDesc")}
    value={s.security.defaultExpireDays}
    valueLabel={s.security.defaultExpireDays > 0
      ? t(lang, "security.days", { count: s.security.defaultExpireDays })
      : t(lang, "settings.effectOff")}
    min={0}
    max={3650}
    step={1}
    onchange={(value) => change("defaultExpireDays", value)}
  />

  <SettingToggleCard
    icon="save"
    label={t(lang, "security.autoSaveAfterEdit")}
    description={t(lang, "security.autoSaveAfterEditDesc")}
    checked={security.autoSaveAfterEdit}
    onchange={(checked) => change("autoSaveAfterEdit", checked)}
  />

  <SettingToggleCard
    icon="save"
    label={t(lang, "security.autoSaveOnClose")}
    description={t(lang, "security.autoSaveOnCloseDesc")}
    checked={security.autoSaveOnClose}
    onchange={(checked) => change("autoSaveOnClose", checked)}
  />

  <SettingToggleCard
    icon="copy"
    label={t(lang, "security.clearOnLock")}
    description={t(lang, "security.clearOnLockDesc")}
    checked={security.clearOnLock}
    onchange={(checked) => change("clearOnLock", checked)}
  />

  <SettingToggleCard
    icon="eye-off"
    label={t(lang, "security.repeatPasswordOnlyWhenHidden")}
    description={t(lang, "security.repeatPasswordOnlyWhenHiddenDesc")}
    checked={security.repeatPasswordOnlyWhenHidden}
    onchange={(checked) => change("repeatPasswordOnlyWhenHidden", checked)}
  />

  <SettingToggleCard
    icon="key"
    label={t(lang, "security.keyTransformWeakWarning")}
    description={t(lang, "security.keyTransformWeakWarningDesc")}
    checked={security.keyTransformWeakWarning}
    onchange={(checked) => change("keyTransformWeakWarning", checked)}
  />

  <SettingToggleCard
    icon="shield"
    label={t(lang, "security.minimizeTray")}
    description={t(lang, "security.minimizeTrayDesc")}
    checked={security.minimizeToTray}
    onchange={(checked) => change("minimizeToTray", checked)}
  />

  <SettingToggleCard
    icon="lock"
    label={t(lang, "security.lockOnMinimizeTray")}
    description={t(lang, "security.lockOnMinimizeTrayDesc")}
    checked={security.lockOnMinimizeToTray}
    disabled={!security.minimizeToTray}
    onchange={(checked) => change("lockOnMinimizeToTray", checked)}
  />

  <SettingToggleCard
    icon="clock"
    label={t(lang, "security.showExpiredOnOpen")}
    description={t(lang, "security.showExpiredOnOpenDesc")}
    checked={security.showExpiredOnOpen}
    onchange={(checked) => change("showExpiredOnOpen", checked)}
  />

  <SettingRangeCard
    icon="clock"
    label={t(lang, "security.expirySoonDays")}
    description={t(lang, "security.expirySoonDaysDesc")}
    value={security.expirySoonDays}
    valueLabel={security.expirySoonDays > 0
      ? t(lang, "security.days", { count: security.expirySoonDays })
      : t(lang, "settings.effectOff")}
    min={0}
    max={365}
    step={1}
    onchange={(value) => change("expirySoonDays", value)}
  />

  <SettingToggleCard
    icon="minimize"
    label={t(lang, "security.minimizeAfterLocking")}
    description={t(lang, "security.minimizeAfterLockingDesc")}
    checked={security.minimizeAfterLocking}
    onchange={(checked) => change("minimizeAfterLocking", checked)}
  />

  <SettingToggleCard
    icon="eye-off"
    label={t(lang, "security.lockAfterAction")}
    description={t(lang, "security.lockAfterActionDesc")}
    checked={security.lockAfterAction}
    onchange={(checked) => change("lockAfterAction", checked)}
  />

  <SettingToggleCard
    icon="unlock"
    label={t(lang, "security.lockOnFocusLoss")}
    description={t(lang, "security.lockOnFocusLossDesc")}
    checked={security.lockOnFocusLoss}
    onchange={(checked) => change("lockOnFocusLoss", checked)}
  />

  <SettingToggleCard
    icon="key"
    label={t(lang, "security.rememberPassword")}
    description={t(lang, "security.rememberPasswordDesc")}
    checked={security.rememberPassword}
    ariaLabel={t(lang, "security.rememberPasswordAria")}
    onchange={(checked) => change("rememberPassword", checked)}
  />

  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="unlock" size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>{t(lang, "security.changeKey")}</strong>
          <p>{t(lang, "security.changeKeyDesc")}</p>
        </div>
      </div>
    </div>
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
      <button
        class="settings-submit-button"
        type="button"
        onclick={submitMasterKeyChange}
        disabled={busy}
      >
        {busy ? t(lang, "security.changing") : t(lang, "security.changeAndSave")}
      </button>
    </div>
    {#if feedback}
      <Feedback inline success={feedback.ok} message={feedback.message} />
    {/if}
  </section>

  <p class="auto-save-note">{t(lang, "settings.autoSaveNote")}</p>
</div>

{#if showWeakWarning}
  <KdfWarningDialog oncancel={() => (showWeakWarning = false)} oncontinue={continueWeakMasterKey} />
{/if}

<style>
  .master-key-tries-control {
    width: 96px;
    flex: 0 0 96px;
  }

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
  }

  .settings-submit-button {
    width: 100%;
    height: 32px;
    border: 1px solid var(--selection-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 18%, var(--card-bg));
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    font-weight: 560;
  }

  .settings-submit-button:hover {
    background: color-mix(in srgb, var(--selection-color) 26%, var(--card-bg));
  }

  .settings-submit-button:disabled {
    cursor: wait;
    opacity: 0.6;
  }
</style>
