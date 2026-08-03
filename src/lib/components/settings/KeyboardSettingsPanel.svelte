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

  let recordingAction = $state("");
  let recordingTimer: ReturnType<typeof setTimeout> | undefined = $state();

  function startRecording(actionId: string): void {
    stopRecording();
    recordingAction = actionId;
    window.addEventListener("keydown", onRecordingKey, true);
    recordingTimer = setTimeout(() => {
      recordingTimer = undefined;
      stopRecording();
    }, 5000);
  }

  function stopRecording(): void {
    if (recordingTimer !== undefined) {
      clearTimeout(recordingTimer);
      recordingTimer = undefined;
    }
    recordingAction = "";
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
    const actionId = recordingAction;
    stopRecording();
    if (actionId) setBinding(actionId, pressed.join("+"));
  }

  onDestroy(() => {
    if (recordingTimer !== undefined) clearTimeout(recordingTimer);
    window.removeEventListener("keydown", onRecordingKey, true);
  });
</script>

<section class="setting-card">
  <div class="setting-heading">
    <span class="setting-icon"><AppIcon name="keyboard" size={17} /></span>
    <div class="heading-inline">
      <div>
        <strong>全局自动填充热键</strong>
        <p>按快捷键时，把匹配前台窗口的条目自动键入；留空禁用</p>
        <p class="hint">
          使用 Ctrl/Alt/Shift/Command 加按键组合，如
          Ctrl+Shift+A；按窗口标题匹配条目网址域名或标题，回收站内条目不参与
        </p>
      </div>
    </div>
  </div>
  <input
    class="settings-input shortcut-input"
    type="text"
    spellcheck="false"
    value={keyboard.autoTypeGlobal}
    placeholder="Ctrl+Shift+A"
    oninput={(e) => change("autoTypeGlobal", (e.currentTarget as HTMLInputElement).value.trim())}
  />
</section>

{#each KEYBOARD_ACTIONS as action (action.id)}
  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name={action.icon} size={17} /></span>
      <div class="heading-inline">
        <div>
          <strong>{action.label}</strong>
          <p>{action.description}</p>
        </div>
      </div>
    </div>
    <div class="shortcut-row">
      {#if recordingAction === action.id}
        <span class="binding-chip recording">按下快捷键…（Esc 取消）</span>
        <button type="button" class="binding-add" onclick={stopRecording} aria-label="取消录制"
          >×</button
        >
      {:else if bindingFor(action.id)}
        <span class="binding-chip">
          <kbd>{bindingFor(action.id)}</kbd>
          <button
            type="button"
            class="binding-clear"
            aria-label="清除快捷键"
            onclick={() => setBinding(action.id, "")}>×</button
          >
        </span>
        <button
          type="button"
          class="binding-add"
          onclick={() => startRecording(action.id)}
          aria-label="录制快捷键">＋</button
        >
      {:else}
        <span class="binding-chip default"><kbd>{action.default}</kbd></span>
        <button
          type="button"
          class="binding-add"
          onclick={() => startRecording(action.id)}
          aria-label="录制快捷键">＋</button
        >
      {/if}
    </div>
  </section>
{/each}

<p class="panel-note">
  点击 ＋ 录制新快捷键；录制后可通过 × 移除绑定并恢复默认。快捷键在输入框或弹窗打开时不生效。
</p>

<style>
  .shortcut-input {
    margin-top: 10px;
    max-width: 240px;
  }

  .shortcut-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 10px;
  }

  .binding-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
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
    color: var(--selection-color);
    font-size: 11px;
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

  .binding-clear {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: none;
    border-radius: 4px;
    font-size: 12px;
    line-height: 1;
    color: var(--text-muted);
    background: transparent;
    cursor: pointer;
  }

  .binding-clear:hover {
    color: var(--danger-color);
    background: color-mix(in srgb, var(--danger-color) 12%, transparent);
  }

  .binding-add {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 1px dashed var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: transparent;
    font-size: 15px;
    cursor: pointer;
  }

  .binding-add:hover {
    color: var(--text-secondary);
    border-color: var(--text-muted);
  }

  .panel-note {
    margin: 4px 2px 0;
    font-size: 10px;
    color: var(--text-secondary);
    opacity: 0.75;
  }
</style>
