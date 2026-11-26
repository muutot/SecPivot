<script lang="ts">
  import { onDestroy } from "svelte";
  import { get } from "svelte/store";
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { vault } from "$lib/services/vault";
  import { rememberCredential } from "$lib/services/security";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  import StandaloneVaultShell from "$lib/components/StandaloneVaultShell.svelte";
  import VaultCredentialFields from "$lib/components/VaultCredentialFields.svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import Feedback from "$lib/components/templates/form/Feedback.svelte";
  import { MasterKeyAttempts, type MasterKeyAttempt } from "$lib/utils/master-key-attempts";

  interface Props {
    remembered: { path: string; fileName: string } | null;
    onopened: () => void;
    onswitch: () => void;
  }

  let { remembered, onopened, onswitch }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  let password = $state("");
  let keyfilePath = $state("");
  let showPassword = $state(false);
  let busy = $state(false);
  let error = $state("");
  let helloAvailable = $state(false);
  let attemptsExhausted = $state(false);
  const openAttempts = new MasterKeyAttempts();
  onDestroy(() => openAttempts.close());

  function restartUnlock(): void {
    openAttempts.start(get(appSettings).security.masterKeyTries);
    attemptsExhausted = false;
    password = "";
    keyfilePath = "";
    showPassword = false;
    error = "";
  }
  $effect(() => {
    const path = remembered?.path;
    if (path) restartUnlock();
    else openAttempts.close();
  });

  function failedUnlock(attempt: MasterKeyAttempt, failure: unknown, submitted = true): void {
    const result = openAttempts.finish(attempt, submitted);
    if (!result) return;
    error = String(failure);
    if (result.exhausted) {
      attemptsExhausted = true;
      password = "";
      keyfilePath = "";
      showPassword = false;
      error += `\n${t(lang, "security.masterKeyAttemptsExceeded", { count: result.limit })}`;
    }
  }

  $effect(() => {
    const path = remembered?.path;
    if (!path || !isTauriRuntime()) {
      helloAvailable = false;
      return;
    }
    void invoke<{ password?: string } | null>("get_saved_credential", { path })
      .then((result) => {
        helloAvailable = result != null;
      })
      .catch(() => {
        helloAvailable = false;
      });
  });

  async function pickKeyfile(): Promise<void> {
    const selected = await open({ multiple: false });
    if (selected) keyfilePath = String(selected);
  }

  async function unlock(): Promise<void> {
    if (busy || !remembered) return;
    if (!password && !keyfilePath) {
      error = t(lang, "lock.needPassword");
      return;
    }
    const attempt = openAttempts.reserve();
    if (!attempt) return;
    const openingPath = remembered.path;
    const openingName = remembered.fileName;
    const openingPassword = password;
    const openingKeyfile = keyfilePath;
    busy = true;
    error = "";
    try {
      await vault.open(openingPath, openingPassword, openingKeyfile || undefined, {
        name: openingName,
      });
      // A stored password alone can never open a keyfile-protected vault, so
      // don't remember one — it would present a Windows Hello button that can
      // only ever fail.
      if (!openingKeyfile) void rememberCredential(openingPath, openingPassword);
      if (openAttempts.finish(attempt)) onopened();
    } catch (e) {
      failedUnlock(attempt, e);
    } finally {
      busy = false;
    }
  }

  async function unlockWithHello(): Promise<void> {
    if (busy || !remembered) return;
    const attempt = openAttempts.reserve();
    if (!attempt) return;
    const openingPath = remembered.path;
    const openingName = remembered.fileName;
    let submitted = false;
    busy = true;
    error = "";
    try {
      const saved = await invoke<{ password?: string } | null>("get_saved_credential", {
        path: openingPath,
      });
      if (!saved?.password) {
        if (openAttempts.finish(attempt, false)) {
          error = t(lang, "lock.noSavedCredential");
          helloAvailable = false;
        }
        return;
      }
      // A replacement/unmounted prompt must not submit a late saved credential.
      if (!openAttempts.isPending(attempt) || remembered?.path !== openingPath) {
        openAttempts.finish(attempt, false);
        return;
      }
      submitted = true;
      await vault.open(openingPath, saved.password, undefined, { name: openingName });
      if (openAttempts.finish(attempt)) onopened();
    } catch (e) {
      failedUnlock(attempt, e, submitted);
    } finally {
      busy = false;
    }
  }
</script>

<StandaloneVaultShell
  icon="lock"
  title={t(lang, "lock.title")}
  subtitle={remembered?.fileName ?? ""}
>
  <div class="lock-fields">
    {#if attemptsExhausted}
      <Feedback inline message={error} />
    {:else}
      <VaultCredentialFields
        bind:password
        bind:keyfilePath
        bind:showPassword
        {busy}
        {error}
        isCreate={false}
        onPickKeyfile={pickKeyfile}
      />
    {/if}
  </div>

  <div class="unlock-actions">
    <Button onclick={onswitch} disabled={busy}>{t(lang, "lock.switchDatabase")}</Button>
    {#if attemptsExhausted}
      <Button variant="primary" onclick={restartUnlock} disabled={busy}>
        <span>{t(lang, "security.restartUnlock")}</span>
      </Button>
    {:else}
      {#if helloAvailable}
        <Button onclick={() => void unlockWithHello()} disabled={busy}>
          <AppIcon name="unlock" size={15} />Windows Hello
        </Button>
      {/if}
      <Button
        variant="primary"
        onclick={() => void unlock()}
        disabled={busy || (!password && !keyfilePath)}
        {busy}
      >
        {busy ? t(lang, "lock.unlocking") : t(lang, "lock.unlock")}
      </Button>
    {/if}
  </div>

  {#if remembered}
    <div class="lock-path" title={remembered.path}>
      <AppIcon name="file" size={11} />
      <span class="lock-path__text">{remembered.path}</span>
    </div>
  {/if}
</StandaloneVaultShell>

<style>
  .lock-fields {
    width: 100%;
    margin-top: 22px;
    text-align: left;
  }

  .lock-fields :global(.field > span) {
    text-align: left;
  }

  .unlock-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
    width: 100%;
    margin-top: 20px;
  }

  .unlock-actions :global(.btn) {
    flex: 0 0 auto;
  }

  .lock-path {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    max-width: 100%;
    margin-top: 18px;
    padding: 6px 10px;
    box-sizing: border-box;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: color-mix(in srgb, var(--card-bg) 70%, transparent);
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .lock-path__text {
    min-width: 0;
    overflow: hidden;
    font-family: var(--font-mono, monospace);
    font-variant-numeric: tabular-nums;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lock-path :global(.app-icon) {
    flex: 0 0 auto;
    opacity: 0.75;
  }
</style>
