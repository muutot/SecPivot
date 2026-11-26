<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";

  interface Props {
    password?: string;
    confirm?: string;
    showPassword?: boolean;
    repeat?: boolean;
    busy?: boolean;
    size?: "md" | "control";
    passwordLabel: string;
    passwordPlaceholder?: string;
    confirmLabel?: string;
    confirmPlaceholder?: string;
  }

  let {
    password = $bindable(""),
    confirm = $bindable(""),
    showPassword = $bindable(false),
    repeat = false,
    busy = false,
    size = "md",
    passwordLabel,
    passwordPlaceholder,
    confirmLabel,
    confirmPlaceholder,
  }: Props = $props();

  const id = $props.id();
  const lang = $derived($appSettings.general.language);
  const onlyWhenHidden = $derived($appSettings.security.repeatPasswordOnlyWhenHidden);
  const autoRepeat = $derived(repeat && onlyWhenHidden && showPassword);
  const visibilityLabel = $derived(
    t(lang, showPassword ? "vault.hidePassword" : "vault.showPassword"),
  );

  $effect(() => {
    // Like KeePass, do not retain a second secret while repetition is automatic.
    if (autoRepeat && confirm !== "") confirm = "";
  });

  function toggleVisibility(): void {
    if (busy) return;
    // Re-hiding a reviewed password restores the repeat value once. Subsequent
    // hidden edits must match it again; opening/resetting a form never copies it.
    if (showPassword && repeat && onlyWhenHidden) confirm = password;
    showPassword = !showPassword;
  }
</script>

<div class="field">
  <label for={`${id}-password`}>{passwordLabel}</label>
  <div class="password-row">
    <div class="password-input">
      <TextField
        id={`${id}-password`}
        {size}
        type={showPassword ? "text" : "password"}
        bind:value={password}
        placeholder={passwordPlaceholder}
        autocomplete={repeat ? "new-password" : "current-password"}
        spellcheck={false}
        disabled={busy}
      />
    </div>
    <Button
      variant="field"
      onclick={toggleVisibility}
      title={visibilityLabel}
      ariaLabel={visibilityLabel}
      disabled={busy}
    >
      <AppIcon name={showPassword ? "eye-off" : "eye"} size={15} />
    </Button>
  </div>
</div>

{#if repeat}
  <div class="field">
    <label for={`${id}-confirm`} class:disabled={autoRepeat}>
      {confirmLabel ?? t(lang, "vault.confirmPassword")}
    </label>
    <TextField
      id={`${id}-confirm`}
      {size}
      type={showPassword ? "text" : "password"}
      bind:value={confirm}
      placeholder={autoRepeat ? t(lang, "vault.repeatNotNeeded") : confirmPlaceholder}
      autocomplete="new-password"
      spellcheck={false}
      disabled={busy || autoRepeat}
    />
  </div>
{/if}

<style>
  .field {
    margin-top: 10px;
  }

  .field > label {
    display: block;
    margin-bottom: 5px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
    text-align: left;
  }

  .field > label.disabled {
    color: var(--text-faint);
  }

  .password-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .password-input {
    flex: 1;
    min-width: 0;
  }
</style>
