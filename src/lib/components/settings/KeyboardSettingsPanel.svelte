<script lang="ts">
  import { onDestroy } from "svelte";
  import { appSettings } from "$lib/services/settings";
  import { KEYBOARD_ACTIONS } from "$lib/services/keyboard";
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

  type RecordingTarget = "global" | string;
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
    if (target === "global") change("autoTypeGlobal", shortcut);
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
      <span class="eyebrow">Settings · 快捷键</span>
      <h2>快捷键</h2>
      <p>全局自动填充热键与常用操作的窗口内快捷键。</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label="关闭">×</button>
  </header>
{/if}

<div class="settings-scroll">
  <section class="setting-card toggle-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="keyboard" size={17} /></span>
      <div>
        <strong>全局自动填充热键</strong>
        <p>
          按下快捷键时，把匹配前台窗口的条目自动键入；按窗口标题匹配条目的网址域名或标题，回收站内条目不参与。未绑定时禁用。
        </p>
      </div>
    </div>
    <div class="shortcut-bindings">
      {#if recordingTarget === "global"}
        <div class="binding-chip recording">
          <kbd>按下快捷键…</kbd>
          <button
            type="button"
            class="binding-chip-close"
            onclick={stopRecording}
            aria-label="取消录制">&times;</button
          >
        </div>
      {:else if keyboard.autoTypeGlobal}
        <div class="binding-chip">
          <kbd>{keyboard.autoTypeGlobal}</kbd>
          <button
            type="button"
            class="binding-chip-close"
            onclick={() => change("autoTypeGlobal", "")}
            aria-label="移除绑定">&minus;</button
          >
        </div>
        <button
          type="button"
          class="binding-add"
          onclick={() => startRecording("global")}
          aria-label="重新录制">+</button
        >
      {:else}
        <span class="binding-disabled">未绑定</span>
        <button
          type="button"
          class="binding-add"
          onclick={() => startRecording("global")}
          aria-label="录制快捷键">+</button
        >
      {/if}
    </div>
  </section>

  {#each KEYBOARD_ACTIONS as action (action.id)}
    <section class="setting-card toggle-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name={action.icon} size={17} /></span>
        <div>
          <strong>{action.label}</strong>
          <p>{action.description}</p>
        </div>
      </div>
      <div class="shortcut-bindings">
        {#if recordingTarget === action.id}
          <div class="binding-chip recording">
            <kbd>按下快捷键…</kbd>
            <button
              type="button"
              class="binding-chip-close"
              onclick={stopRecording}
              aria-label="取消录制">&times;</button
            >
          </div>
        {:else if bindingFor(action.id)}
          <div class="binding-chip">
            <kbd>{bindingFor(action.id)}</kbd>
            <button
              type="button"
              class="binding-chip-close"
              onclick={() => setBinding(action.id, "")}
              aria-label="移除绑定">&minus;</button
            >
          </div>
          <button
            type="button"
            class="binding-add"
            onclick={() => startRecording(action.id)}
            aria-label="重新录制">+</button
          >
        {:else}
          <div class="binding-chip default">
            <kbd>{action.default}</kbd>
          </div>
          <button
            type="button"
            class="binding-add"
            onclick={() => startRecording(action.id)}
            aria-label="录制快捷键">+</button
          >
        {/if}
      </div>
    </section>
  {/each}

  <p class="settings-note">
    点击 ＋ 录制新快捷键；录制后通过 × 移除绑定并恢复默认。快捷键在输入框或弹窗打开时不生效。
  </p>
  <p class="auto-save-note">修改即时生效并自动保存</p>
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
