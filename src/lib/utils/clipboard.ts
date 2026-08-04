import { get } from "svelte/store";
import { appSettings } from "$lib/services/settings";
import { isTauriRuntime } from "$lib/services/settings";
import { invoke } from "@tauri-apps/api/core";

let clearTimer: ReturnType<typeof setTimeout> | null = null;

/** Text this session last copied via `copyText`, used to make sure the
 * scheduled wipe only clears our own content, never something the user
 * copied in another app while the timer was pending. */
let lastCopiedText: string | null = null;

export async function copyText(text: string): Promise<void> {
  lastCopiedText = text;
  await copyRaw(text);
  scheduleClipboardClear();
}

export function scheduleClipboardClear(): void {
  const seconds = get(appSettings).security.clipboardClearSeconds;
  if (seconds <= 0) return;
  if (clearTimer) clearTimeout(clearTimer);
  clearTimer = setTimeout(() => void clearClipboardIfUnchanged(), seconds * 1000);
}

/** Wipe the clipboard only when it still holds the text we copied. If the
 * user has since copied something else (or the clipboard is unreadable /
 * non-text), leave it alone — the app must never destroy unrelated data. */
export async function clearClipboardIfUnchanged(): Promise<void> {
  if (!isTauriRuntime()) {
    // Browser demo: no backend read-back; fall back to the old behavior.
    try {
      await copyRaw("");
    } catch {
      // clipboard unavailable; nothing to clear
    }
    return;
  }
  try {
    const current = await invoke<string | null>("clipboard_read_text");
    if (current !== null && lastCopiedText !== null && current === lastCopiedText) {
      await invoke("clipboard_clear");
    }
  } catch {
    // backend unavailable; fall back to writing an empty string
    try {
      await copyRaw("");
    } catch {
      // clipboard unavailable; nothing to clear
    }
  }
}

/** Immediately wipe the clipboard (used by lock when `clearOnLock` is enabled). */
export async function clearClipboard(): Promise<void> {
  try {
    if (isTauriRuntime()) {
      await invoke("clipboard_clear");
      return;
    }
    await copyRaw("");
  } catch {
    try {
      await copyRaw("");
    } catch {
      // clipboard unavailable; nothing to clear
    }
  }
}

/** Best-effort write: modern async Clipboard API, falling back to a hidden
 *  textarea + `execCommand("copy")` when unavailable. */
async function copyRaw(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  document.body.appendChild(textarea);
  textarea.select();
  document.execCommand("copy");
  textarea.remove();
}
