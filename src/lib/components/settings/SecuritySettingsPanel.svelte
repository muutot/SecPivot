<script lang="ts">
  import { get } from "svelte/store";
  import { open } from "@tauri-apps/plugin-dialog";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { rememberCredential } from "$lib/services/security";
  import { vault } from "$lib/services/vault";
  import type { SecuritySettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
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
  let newKeyfile = $state<string | null>(null);
  let busy = $state(false);
  let feedback = $state<{ ok: boolean; message: string } | null>(null);

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
    feedback = null;
    if (newPassword.length > 0 && newPassword !== confirmPassword) {
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
    try {
      const state = await vault.callInSession(sessionId, () =>
        vault.changeMasterKey(newPassword, newKeyfile),
      );
      if (vault.getActiveSessionId() !== sessionId) return;
      if (state.path && newPassword.length > 0) {
        await rememberCredential(state.path, newPassword);
      }
      newPassword = "";
      confirmPassword = "";
      newKeyfile = null;
      feedback = { ok: true, message: t(lang, "security.changed") };
    } catch (err) {
      if (vault.getActiveSessionId() !== sessionId) return;
      feedback = { ok: false, message: String(err) };
    } finally {
      busy = false;
    }
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

  <SettingToggleCard
    icon="copy"
    label={t(lang, "security.clearOnLock")}
    description={t(lang, "security.clearOnLockDesc")}
    checked={security.clearOnLock}
    onchange={(checked) => change("clearOnLock", checked)}
  />

  <SettingToggleCard
    icon="shield"
    label={t(lang, "security.minimizeTray")}
    description={t(lang, "security.minimizeTrayDesc")}
    checked={security.minimizeToTray}
    onchange={(checked) => change("minimizeToTray", checked)}
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
    <label class="settings-label" for="mk-new-password"
      >{t(lang, "security.newPasswordLabel")}</label
    >
    <TextField
      id="mk-new-password"
      size="control"
      type="password"
      placeholder={t(lang, "security.newPasswordPh")}
      bind:value={newPassword}
      autocomplete="new-password"
    />
    <label class="settings-label" for="mk-confirm-password"
      >{t(lang, "security.confirmPasswordLabel")}</label
    >
    <TextField
      id="mk-confirm-password"
      size="control"
      type="password"
      placeholder={t(lang, "security.confirmPasswordPh")}
      bind:value={confirmPassword}
      autocomplete="new-password"
    />
    <div class="setting-row mk-keyfile-row">
      <span class="mk-keyfile-name" class:mk-empty={!newKeyfile}>
        {newKeyfile ?? t(lang, "security.noKeyfile")}
      </span>
      <Button variant="field" onclick={pickKeyfile} disabled={!isTauriRuntime()}>
        {t(lang, "vault.pickKeyfileTitle")}
      </Button>
      {#if newKeyfile}
        <Button variant="field" onclick={() => (newKeyfile = null)}
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
