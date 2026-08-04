import { get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { appSettings, isTauriRuntime } from "$lib/services/settings";
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

/**
 * Persist the master password in the OS credential store (Windows Hello)
 * after a successful password unlock, when the setting is enabled.
 * Non-fatal: a storage failure must never block unlocking.
 */
export async function rememberCredential(path: string, password: string): Promise<void> {
  if (!isTauriRuntime()) return;
  if (!get(appSettings).security.rememberPassword) return;
  try {
    await invoke("remember_credential", { path, password });
  } catch {
    // best-effort only
  }
}

/** Copy a sensitive value (password) and lock immediately when `lockAfterAction` is on. */
export async function copySensitive(value: string): Promise<void> {
  await copyText(value);
  if (get(appSettings).security.lockAfterAction) {
    await lockVault();
  }
}

/** Copy a value, applying the sensitive copy path (with `lockAfterAction`)
 *  when the flag is set, otherwise a plain copy. Single dispatch used by every
 *  copy target so lock-after-action always behaves the same. */
export async function copyValue(value: string, sensitive: boolean): Promise<void> {
  if (sensitive) {
    await copySensitive(value);
  } else {
    await copyText(value);
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
 * Activity handler shared by all idle-watch events. Discrete user actions
 * (`pointerdown`/`keydown`) always re-arm the timer: a key press 20 s before
 * the deadline is still real activity. High-frequency events
 * (`mousemove`/`wheel`/`scroll`) only re-arm when less than 15 s of the
 * timeout remain, so the timer is not churned on every fire.
 */
function onActivity(discrete: boolean): void {
  if (discrete || Date.now() >= idleDeadline - 15_000) armIdleLock();
}

/** Install user-activity listeners that keep the idle timer fresh. Returns a cleanup. */
export function installAutoLock(): () => void {
  const onDiscrete = (): void => onActivity(true);
  const onContinuous = (): void => onActivity(false);
  window.addEventListener("pointerdown", onDiscrete);
  window.addEventListener("keydown", onDiscrete);
  window.addEventListener("mousemove", onContinuous);
  window.addEventListener("wheel", onContinuous, { capture: true, passive: true });
  window.addEventListener("scroll", onContinuous, { capture: true, passive: true });
  armIdleLock();
  return () => {
    window.removeEventListener("pointerdown", onDiscrete);
    window.removeEventListener("keydown", onDiscrete);
    window.removeEventListener("mousemove", onContinuous);
    window.removeEventListener("wheel", onContinuous, true);
    window.removeEventListener("scroll", onContinuous, true);
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
