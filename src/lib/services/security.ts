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
let idleDeadline = 0;

function clearIdleTimer(): void {
  if (idleTimer) clearTimeout(idleTimer);
  idleTimer = null;
}

/** (Re)arm the idle auto-lock timer from `autoLockMinutes` (0 disables). */
export function armIdleLock(): void {
  clearIdleTimer();
  const minutes = get(appSettings).security.autoLockMinutes;
  if (minutes <= 0 || !vault.get()) {
    idleDeadline = 0;
    return;
  }
  idleDeadline = Date.now() + minutes * 60_000;
  idleTimer = setTimeout(() => {
    idleTimer = null;
    void lockVault();
  }, minutes * 60_000);
}

/**
 * Activity handler shared by all idle-watch events. High-frequency events
 * (`mousemove`/`wheel`/`scroll`) only re-arm when less than 15 s of the
 * timeout remain, so the timer is not churned on every fire.
 */
function onActivity(): void {
  if (Date.now() >= idleDeadline - 15_000) armIdleLock();
}

/** Install user-activity listeners that keep the idle timer fresh. Returns a cleanup. */
export function installAutoLock(): () => void {
  window.addEventListener("pointerdown", onActivity);
  window.addEventListener("keydown", onActivity);
  window.addEventListener("mousemove", onActivity);
  window.addEventListener("wheel", onActivity, { capture: true, passive: true });
  window.addEventListener("scroll", onActivity, { capture: true, passive: true });
  armIdleLock();
  return () => {
    window.removeEventListener("pointerdown", onActivity);
    window.removeEventListener("keydown", onActivity);
    window.removeEventListener("mousemove", onActivity);
    window.removeEventListener("wheel", onActivity, true);
    window.removeEventListener("scroll", onActivity, true);
    clearIdleTimer();
  };
}

/**
 * Install the focus-loss lock: when `lockOnFocusLoss` is enabled and a vault
 * is open, losing window focus locks immediately. Returns a cleanup.
 */
export function installFocusLock(): () => void {
  const onBlur = () => {
    if (!get(appSettings).security.lockOnFocusLoss) return;
    if (!vault.get()) return;
    void lockVault();
  };
  window.addEventListener("blur", onBlur);
  return () => {
    window.removeEventListener("blur", onBlur);
  };
}
