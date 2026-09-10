<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  interface Props {
    password?: string;
    confirm?: string;
    keyfilePath?: string;
    showPassword?: boolean;
    busy?: boolean;
    error?: string;
    isCreate?: boolean;
    isDemo?: boolean;
    path?: string;
    showPathField?: boolean;
    onPickKeyfile?: () => void;
    onPickCreatePath?: () => void;
  }

  let {
    password = $bindable(""),
    confirm = $bindable(""),
    keyfilePath = $bindable(""),
    showPassword = $bindable(false),
    busy = false,
    error = "",
    isCreate = false,
    isDemo = false,
    path = $bindable(""),
    showPathField = false,
    onPickKeyfile,
    onPickCreatePath,
  }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);
</script>

{#if showPathField}
  <label class="field">
    <span>{t(lang, "vault.path")}</span>
    <div class="path-row">
      <TextField
        bind:value={path}
        placeholder={isTauriRuntime() ? t(lang, "vault.pickFile") : t(lang, "vault.demoStorage")}
        disabled={!isTauriRuntime()}
      />
      {#if isTauriRuntime() && onPickCreatePath}
        <button
          class="browse-button"
          onclick={onPickCreatePath}
          title={t(lang, "vault.pickPathTitle")}
          disabled={busy}
        >
          <AppIcon name="folder" size={15} />
        </button>
      {/if}
    </div>
  </label>
{/if}

<label class="field">
  <span>{t(lang, "vault.masterPassword")}</span>
  <div class="path-row">
    <TextField
      type={showPassword ? "text" : "password"}
      bind:value={password}
      placeholder={isDemo ? t(lang, "vault.demoBlank") : t(lang, "vault.required")}
      disabled={busy}
    />
    <button
      class="browse-button"
      onclick={() => (showPassword = !showPassword)}
      title={t(lang, "vault.showPassword")}
      disabled={busy}
    >
      <AppIcon name={showPassword ? "eye-off" : "eye"} size={15} />
    </button>
  </div>
</label>

{#if isCreate}
  <label class="field">
    <span>{t(lang, "vault.confirmPassword")}</span>
    <div class="path-row">
      <TextField type="password" bind:value={confirm} disabled={busy} />
    </div>
  </label>
{/if}

{#if isTauriRuntime()}
  <label class="field">
    <span>{t(lang, "vault.keyfile")}</span>
    <div class="path-row">
      <TextField
        bind:value={keyfilePath}
        placeholder={t(lang, "vault.pickKeyfile")}
        readonly
        disabled={busy}
      />
      <button
        class="browse-button"
        onclick={onPickKeyfile}
        title={t(lang, "vault.pickKeyfileTitle")}
        disabled={busy}
      >
        <AppIcon name="folder" size={15} />
      </button>
    </div>
  </label>
{/if}

{#if error}
  <p class="modal-error">{error}</p>
{/if}

<style>
  .field {
    display: block;
    margin-top: 10px;
  }

  .field > span {
    display: block;
    margin-bottom: 5px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
    text-align: left;
  }

  .path-row {
    display: flex;
    gap: 6px;
  }

  .browse-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    flex: 0 0 auto;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    cursor: pointer;
  }

  .browse-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .browse-button:disabled {
    cursor: not-allowed;
    opacity: 0.6;
  }

  .modal-error {
    margin: 10px 0 0;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
  }
</style>
