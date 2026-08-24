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
  scheduleClipboardClear(text);
}

export function scheduleClipboardClear(text: string): void {
  const seconds = get(appSettings).security.clipboardClearSeconds;
  if (clearTimer) clearTimeout(clearTimer);
  clearTimer = null;
  if (seconds <= 0) return;
  clearTimer = setTimeout(() => void clearClipboardIfUnchanged(), seconds * 1000);
  if (isTauriRuntime()) {
    // Backend safety net: clears even when this renderer dies before its
    // timer fires. Best-effort — a scheduling failure just leaves the
    // renderer timer in charge. The backend re-verifies ownership before
    // wiping, so a superseded or already-replaced clipboard stays untouched.
    void invoke("clipboard_schedule_wipe", { text, seconds }).catch(() => {});
  }
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

/** Immediately wipe the clipboard (used by lock when `clearOnLock` is enabled)
 *  and drop all in-memory traces of what this session copied: the scheduled
 *  wipe timer and the remembered text, so the password string does not outlive
 *  the lock. */
export async function clearClipboard(): Promise<void> {
  if (clearTimer) clearTimeout(clearTimer);
  clearTimer = null;
  lastCopiedText = null;
  if (isTauriRuntime()) {
    // Cancel any pending backend wipe: after an explicit clear it must not
    // fire later and destroy content the user copied in another app.
    void invoke("clipboard_cancel_scheduled_wipe").catch(() => {});
  }
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
