import { get } from "svelte/store";
import { appSettings } from "$lib/services/settings";

export async function copyText(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
  } else {
    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand("copy");
    textarea.remove();
  }
  scheduleClipboardClear();
}

let clearTimer: ReturnType<typeof setTimeout> | null = null;

export function scheduleClipboardClear(): void {
  const seconds = get(appSettings).security.clipboardClearSeconds;
  if (seconds <= 0) return;
  if (clearTimer) clearTimeout(clearTimer);
  clearTimer = setTimeout(() => void clearClipboard(), seconds * 1000);
}

/** Immediately wipe the clipboard (used by lock when `clearOnLock` is enabled). */
export async function clearClipboard(): Promise<void> {
  try {
    await copyRaw("");
  } catch {
    // clipboard unavailable; nothing to clear
  }
}

async function copyRaw(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
  } else {
    const textarea = document.createElement("textarea");
    textarea.value = text;
    textarea.style.position = "fixed";
    textarea.style.opacity = "0";
    document.body.appendChild(textarea);
    textarea.select();
    document.execCommand("copy");
    textarea.remove();
  }
}
