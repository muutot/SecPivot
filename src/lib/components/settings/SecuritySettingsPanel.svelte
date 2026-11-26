<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import type { SecuritySettings } from "$lib/types/settings";
  import MasterKeyChangeForm from "$lib/components/MasterKeyChangeForm.svelte";
  import MasterKeyExpiryRuleCard from "$lib/components/settings/MasterKeyExpiryRuleCard.svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
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
  <MasterKeyExpiryRuleCard setting="masterKeyExpiryRec" />
  <MasterKeyExpiryRuleCard setting="masterKeyExpiryForce" />
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

  <MasterKeyChangeForm />

  <p class="auto-save-note">{t(lang, "settings.autoSaveNote")}</p>
</div>

<style>
  .master-key-tries-control {
    width: 96px;
    flex: 0 0 96px;
  }
</style>
