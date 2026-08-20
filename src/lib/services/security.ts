import { get } from "svelte/store";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { appSettings, isTauriRuntime } from "$lib/services/settings";
import { vault } from "$lib/services/vault";
import { clearClipboard, copyText } from "$lib/utils/clipboard";
import { ActivityLeaseGuard } from "$lib/utils/session-state";

/**
 * Single lock path shared by the manual lock button, idle auto-lock, and
 * `lockAfterAction`. Wipes the clipboard when `clearOnLock` is enabled, then
 * closes every open session (backend zeroizes the passwords); `remembered`
 * stays so the lock screen can offer a quick reopen.
 */
export async function lockVault(): Promise<void> {
  if (get(appSettings).security.clearOnLock) {
    await clearClipboard();
  }
  await vault.closeAll();
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
  // Re-arm immediately when `autoLockMinutes` changes instead of waiting for
  // the next user activity or vault transition. Guard on the actual value so
  // unrelated settings emissions (theme, clipboard, …) do not reset the idle
  // deadline or churn the timer on every write.
  let lastAutoLockMinutes = get(appSettings).security.autoLockMinutes;
  const unsubSettings = appSettings.subscribe((settings) => {
    if (settings.security.autoLockMinutes !== lastAutoLockMinutes) {
      lastAutoLockMinutes = settings.security.autoLockMinutes;
      armIdleLock();
    }
  });
  return () => {
    window.removeEventListener("pointerdown", onDiscrete);
    window.removeEventListener("keydown", onDiscrete);
    window.removeEventListener("mousemove", onContinuous);
    window.removeEventListener("wheel", onContinuous, true);
    window.removeEventListener("scroll", onContinuous, true);
    unsubSettings();
    clearIdleTimer();
  };
}

/** True while the TCATO overlay owns focus. Set synchronously before the
 *  overlay is opened (the blur it causes would otherwise trip the focus-loss
 *  lock) and cleared by the backend's open/close events. */
const tcatoOverlayActivity = new ActivityLeaseGuard();

/** Suppress focus-loss locking while one TCATO open attempt is pending.
 * Releasing this lease never clears an overlay confirmed by another attempt
 * or by the backend open event. */
export function beginTcatoOverlayOpen(): { confirm: () => void; release: () => void } {
  return tcatoOverlayActivity.acquire();
}

export function setTcatoOverlayOpen(open: boolean): void {
  tcatoOverlayActivity.setConfirmed(open);
}

/**
 * Install the focus-loss lock: when `lockOnFocusLoss` is enabled and a vault
 * is open, losing window focus locks immediately. Returns a cleanup.
 */
export function installFocusLock(): () => void {
  const onBlur = () => {
    if (tcatoOverlayActivity.isActive()) return;
    if (!get(appSettings).security.lockOnFocusLoss) return;
    if (!vault.get()) return;
    void lockVault();
  };
  window.addEventListener("blur", onBlur);
  let stopOpen: UnlistenFn | undefined;
  let stopClose: UnlistenFn | undefined;
  if (isTauriRuntime()) {
    void listen("tcato-overlay-open", () => {
      setTcatoOverlayOpen(true);
    }).then((fn) => {
      stopOpen = fn;
    });
    void listen("tcato-overlay-close", () => {
      setTcatoOverlayOpen(false);
    }).then((fn) => {
      stopClose = fn;
    });
  }
  return () => {
    window.removeEventListener("blur", onBlur);
    stopOpen?.();
    stopClose?.();
  };
}
