<script lang="ts">
  import { onDestroy } from "svelte";
  import { appSettings } from "$lib/services/settings";
  import { KEYBOARD_ACTIONS } from "$lib/services/keyboard";
  import { t } from "$lib/i18n";
  import type { KeyboardSettings } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    onclose?: () => void;
    showHeader?: boolean;
  }

  let { onclose = () => {}, showHeader = true }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const keyboard: KeyboardSettings = $derived(s.keyboard);
  const lang = $derived(s.general.language);

  function change<K extends keyof KeyboardSettings>(key: K, value: KeyboardSettings[K]): void {
    appSettings.updateKeyboard(key, value);
  }

  function bindingFor(actionId: string): string {
    return keyboard.shortcuts[actionId] ?? "";
  }

  function setBinding(actionId: string, shortcut: string): void {
    const shortcuts = { ...keyboard.shortcuts };
    if (shortcut) shortcuts[actionId] = shortcut;
    else delete shortcuts[actionId];
    change("shortcuts", shortcuts);
  }

  type RecordingTarget = "autoType" | "tcato" | string;
  let recordingTarget = $state<RecordingTarget | "">("");
  let recordingTimer: ReturnType<typeof setTimeout> | undefined = $state();

  function startRecording(target: RecordingTarget): void {
    stopRecording();
    recordingTarget = target;
    window.addEventListener("keydown", onRecordingKey, true);
    recordingTimer = setTimeout(() => {
      recordingTimer = undefined;
      stopRecording();
    }, 3000);
  }

  function stopRecording(): void {
    if (recordingTimer !== undefined) {
      clearTimeout(recordingTimer);
      recordingTimer = undefined;
    }
    recordingTarget = "";
    window.removeEventListener("keydown", onRecordingKey, true);
  }

  function onRecordingKey(event: KeyboardEvent): void {
    event.preventDefault();
    event.stopPropagation();

    if (event.key === "Escape") {
      stopRecording();
      return;
    }

    const modKeys = ["Control", "Alt", "Shift", "Meta"];
    if (modKeys.includes(event.key)) return;

    const pressed: string[] = [];
    if (event.ctrlKey) pressed.push("Ctrl");
    if (event.altKey) pressed.push("Alt");
    if (event.shiftKey) pressed.push("Shift");
    if (event.metaKey) pressed.push("Meta");

    const ignored = ["AltGraph", "NumLock", "ScrollLock", "PrintScreen"];
    if (!ignored.includes(event.key)) {
      pressed.push(
        event.key === " " ? "Space" : event.key.length === 1 ? event.key.toUpperCase() : event.key,
      );
    }

    if (pressed.length === 0) return;
    const target = recordingTarget;
    stopRecording();
    if (!target) return;
    const shortcut = pressed.join("+");
    if (target === "autoType") change("autoTypeGlobal", shortcut);
    else if (target === "tcato") change("tcatoSummonGlobal", shortcut);
    else setBinding(target, shortcut);
  }

  onDestroy(() => {
    if (recordingTimer !== undefined) clearTimeout(recordingTimer);
    window.removeEventListener("keydown", onRecordingKey, true);
  });
</script>

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · {t(lang, "keyboard.title")}</span>
      <h2>{t(lang, "keyboard.title")}</h2>
      <p>{t(lang, "keyboard.desc")}</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label={t(lang, "common.close")}>×</button>
  </header>
{/if}

<div class="settings-scroll">
  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="keyboard" size={17} /></span>
      <div>
        <strong>{t(lang, "keyboard.autoType")}</strong>
        <p>
          {t(lang, "keyboard.autoTypeDesc")}
        </p>
      </div>
    </div>
    <div class="shortcut-bindings">
      {#if recordingTarget === "autoType"}
        <div class="binding-chip recording">
          <kbd>{t(lang, "keyboard.recording")}</kbd>
          <button
            type="button"
            class="binding-chip-close"
            onclick={stopRecording}
            aria-label={t(lang, "keyboard.cancelRecording")}>&times;</button
          >
        </div>
      {:else if keyboard.autoTypeGlobal}
        <div class="binding-chip">
          <kbd>{keyboard.autoTypeGlobal}</kbd>
          <button
            type="button"
            class="binding-chip-close"
            onclick={() => change("autoTypeGlobal", "")}
            aria-label={t(lang, "keyboard.removeBinding")}>&minus;</button
          >
        </div>
        <button
          type="button"
          class="binding-add"
          onclick={() => startRecording("autoType")}
          aria-label={t(lang, "keyboard.rerecord")}>+</button
        >
      {:else}
        <span class="binding-disabled">{t(lang, "keyboard.unbound")}</span>
        <button
          type="button"
          class="binding-add"
          onclick={() => startRecording("autoType")}
          aria-label={t(lang, "keyboard.record")}>+</button
        >
      {/if}
    </div>
  </section>

  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="keyboard" size={17} /></span>
      <div>
        <strong>{t(lang, "keyboard.tcato")}</strong>
        <p>
          {t(lang, "keyboard.tcatoDesc")}
        </p>
      </div>
    </div>
    <div class="shortcut-bindings">
      {#if recordingTarget === "tcato"}
        <div class="binding-chip recording">
          <kbd>{t(lang, "keyboard.recording")}</kbd>
          <button
            type="button"
            class="binding-chip-close"
            onclick={stopRecording}
            aria-label={t(lang, "keyboard.cancelRecording")}>&times;</button
          >
        </div>
      {:else if keyboard.tcatoSummonGlobal}
        <div class="binding-chip">
          <kbd>{keyboard.tcatoSummonGlobal}</kbd>
          <button
            type="button"
            class="binding-chip-close"
            onclick={() => change("tcatoSummonGlobal", "")}
            aria-label={t(lang, "keyboard.removeBinding")}>&minus;</button
          >
        </div>
        <button
          type="button"
          class="binding-add"
          onclick={() => startRecording("tcato")}
          aria-label={t(lang, "keyboard.rerecord")}>+</button
        >
      {:else}
        <span class="binding-disabled">{t(lang, "keyboard.unbound")}</span>
        <button
          type="button"
          class="binding-add"
          onclick={() => startRecording("tcato")}
          aria-label={t(lang, "keyboard.record")}>+</button
        >
      {/if}
    </div>
  </section>

  {#each KEYBOARD_ACTIONS as action (action.id)}
    <section class="setting-card toggle-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name={action.icon} size={17} /></span>
        <div>
          <strong>{t(lang, action.label)}</strong>
          <p>{t(lang, action.description)}</p>
        </div>
      </div>
      <div class="shortcut-bindings">
        {#if recordingTarget === action.id}
          <div class="binding-chip recording">
            <kbd>{t(lang, "keyboard.recording")}</kbd>
            <button
              type="button"
              class="binding-chip-close"
              onclick={stopRecording}
              aria-label={t(lang, "keyboard.cancelRecording")}>&times;</button
            >
          </div>
        {:else if bindingFor(action.id)}
          <div class="binding-chip">
            <kbd>{bindingFor(action.id)}</kbd>
            <button
              type="button"
              class="binding-chip-close"
              onclick={() => setBinding(action.id, "")}
              aria-label={t(lang, "keyboard.removeBinding")}>&minus;</button
            >
          </div>
          <button
            type="button"
            class="binding-add"
            onclick={() => startRecording(action.id)}
            aria-label={t(lang, "keyboard.rerecord")}>+</button
          >
        {:else}
          <div class="binding-chip default">
            <kbd>{action.default}</kbd>
          </div>
          <button
            type="button"
            class="binding-add"
            onclick={() => startRecording(action.id)}
            aria-label={t(lang, "keyboard.record")}>+</button
          >
        {/if}
      </div>
    </section>
  {/each}

  <p class="settings-note">
    {t(lang, "keyboard.footer")}
  </p>
  <p class="auto-save-note">{t(lang, "settings.autoSaveNote")}</p>
</div>

<style>
  .shortcut-bindings {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .binding-chip {
    position: relative;
    display: inline-flex;
    align-items: center;
    gap: 4px;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
    box-sizing: border-box;
  }

  .binding-chip kbd {
    font:
      11px "Cascadia Code",
      Consolas,
      monospace;
    color: var(--text-primary);
  }

  .binding-chip.default kbd {
    color: var(--text-muted);
  }

  .binding-chip.recording {
    border-color: var(--selection-color);
    animation: pulse-recording 1s ease-in-out infinite;
  }

  @keyframes pulse-recording {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }

  .binding-chip-close {
    position: absolute;
    top: -7px;
    right: -7px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: 50%;
    font-size: 10px;
    line-height: 1;
    color: var(--text-muted);
    background: var(--card-bg);
    cursor: pointer;
    opacity: 0;
    transition: opacity 100ms ease;
  }

  .binding-chip:hover .binding-chip-close {
    opacity: 1;
  }

  .binding-chip-close:hover {
    color: var(--danger-color);
    border-color: var(--danger-color);
    background: color-mix(in srgb, var(--danger-color) 12%, transparent);
  }

  .binding-add {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    padding: 0;
    border: 1px dashed var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: transparent;
    font-size: 17px;
    cursor: pointer;
    transition:
      color 100ms ease,
      border-color 100ms ease;
  }

  .binding-add:hover {
    color: var(--text-secondary);
    border-color: var(--text-muted);
  }

  .binding-disabled {
    color: var(--text-faint);
    font-size: var(--settings-description-size, var(--font-size-secondary, 11px));
    font-style: italic;
  }
</style>
