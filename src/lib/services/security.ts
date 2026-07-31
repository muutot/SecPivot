import { get } from "svelte/store";
import { appSettings } from "$lib/services/settings";
import { vault } from "$lib/services/vault";
import { clearClipboard, copyText } from "$lib/utils/clipboard";

/**
 * Single lock path shared by the manual lock button, idle auto-lock, and
 * `lockAfterAction`. Wipes the clipboard when `clearOnLock` is enabled, then
 * closes the vault session (backend zeroizes the password).
 */
export async function lockVault(): Promise<void> {
  if (get(appSettings).security.clearOnLock) {
    await clearClipboard();
  }
  await vault.close();
}

/** Copy a sensitive value (password) and lock immediately when `lockAfterAction` is on. */
export async function copySensitive(value: string): Promise<void> {
  await copyText(value);
  if (get(appSettings).security.lockAfterAction) {
    await lockVault();
  }
}

let idleTimer: ReturnType<typeof setTimeout> | null = null;

function clearIdleTimer(): void {
  if (idleTimer) clearTimeout(idleTimer);
  idleTimer = null;
}

/** (Re)arm the idle auto-lock timer from `autoLockMinutes` (0 disables). */
export function armIdleLock(): void {
  clearIdleTimer();
  const minutes = get(appSettings).security.autoLockMinutes;
  if (minutes <= 0 || !vault.get()) return;
  idleTimer = setTimeout(() => {
    idleTimer = null;
    void lockVault();
  }, minutes * 60_000);
}

/** Install user-activity listeners that keep the idle timer fresh. Returns a cleanup. */
export function installAutoLock(): () => void {
  const onActivity = () => armIdleLock();
  window.addEventListener("pointerdown", onActivity);
  window.addEventListener("keydown", onActivity);
  armIdleLock();
  return () => {
    window.removeEventListener("pointerdown", onActivity);
    window.removeEventListener("keydown", onActivity);
    clearIdleTimer();
  };
}
