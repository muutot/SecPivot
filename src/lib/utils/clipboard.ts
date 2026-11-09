import { get } from "svelte/store";
import { appSettings, isTauriRuntime } from "$lib/services/settings";
import { invoke } from "@tauri-apps/api/core";

interface ClipboardOwnership {
  fingerprint: Uint8Array;
}

let clearTimer: ReturnType<typeof setTimeout> | null = null;
let lastCopy: ClipboardOwnership | null = null;
let operationEpoch = 0;
let writeTail: Promise<void> | null = null;

/** Preserve this renderer's write order. Start an idle write synchronously so
 * the caller's visible-view guard still immediately precedes Clipboard API use.
 * A rejected write releases the queue without swallowing the caller's error. */
function serializeWrite<T>(operation: () => Promise<T>): Promise<T> {
  const previous = writeTail;
  let result: Promise<T>;
  try {
    result = previous ? previous.then(operation) : operation();
  } catch (error) {
    result = Promise.reject(error);
  }
  const tail = result.then(
    () => {},
    () => {},
  );
  writeTail = tail;
  void tail.then(() => {
    if (writeTail === tail) writeTail = null;
  });
  return result;
}

function releaseOwnership(owned: ClipboardOwnership | null): void {
  if (!owned) return;
  if (lastCopy === owned) lastCopy = null;
  owned.fingerprint.fill(0);
}

function cancelRendererTimer(): void {
  if (clearTimer) clearTimeout(clearTimer);
  clearTimer = null;
}

async function fingerprintText(text: string): Promise<Uint8Array> {
  const bytes = new TextEncoder().encode(text);
  try {
    return new Uint8Array(await crypto.subtle.digest("SHA-256", bytes));
  } finally {
    bytes.fill(0);
  }
}

export async function copyText(text: string): Promise<void> {
  const epoch = ++operationEpoch;
  cancelRendererTimer();
  releaseOwnership(lastCopy);
  try {
    await serializeWrite(() => (epoch === operationEpoch ? copyRaw(text) : Promise.resolve()));
  } catch (error) {
    if (epoch === operationEpoch) releaseOwnership(lastCopy);
    throw error;
  }
  if (epoch !== operationEpoch) return;
  const seconds = get(appSettings).security.clipboardClearSeconds;
  if (isTauriRuntime()) {
    // Also send zero: it supersedes an earlier backend timer. The backend
    // zeroizes this short-lived input and retains only its fingerprint.
    void invoke("clipboard_schedule_wipe", { text, seconds }).catch(() => {});
  }
  if (seconds <= 0) return;
  try {
    const fingerprint = await fingerprintText(text);
    if (epoch !== operationEpoch) {
      fingerprint.fill(0);
      return;
    }
    const owned = { fingerprint };
    lastCopy = owned;
    clearTimer = setTimeout(() => void clearOwnedClipboard(owned), seconds * 1000);
  } catch {
    // No Web Crypto means no renderer baseline, never a retained plaintext
    // fallback. The native scheduled wipe remains available on Windows.
  }
}

/** Clear only the copy owned by this timer. Object identity distinguishes
 * separate copies even when their contents are identical. */
async function clearOwnedClipboard(owned: ClipboardOwnership): Promise<void> {
  if (lastCopy !== owned) return;
  try {
    if (isTauriRuntime()) {
      const cleared = await serializeWrite(async () => {
        if (lastCopy !== owned) return false;
        return invoke<boolean | null>("clipboard_clear_if_matches", {
          fingerprint: Array.from(owned.fingerprint),
        });
      });
      if (cleared !== null) return;
    }
    // The Web Clipboard API has no atomic compare-and-clear. Require a
    // successful read, then revalidate ownership after every async step.
    const read = navigator.clipboard?.readText;
    if (!read) return;
    const current = await read.call(navigator.clipboard);
    if (lastCopy !== owned) return;
    const fingerprint = await fingerprintText(current);
    try {
      if (lastCopy !== owned || !fingerprint.every((byte, i) => byte === owned.fingerprint[i])) {
        return;
      }
      await serializeWrite(() => (lastCopy === owned ? copyRaw("") : Promise.resolve()));
    } finally {
      fingerprint.fill(0);
    }
  } catch {
    // Unreadable clipboard content must never be blindly cleared.
  } finally {
    releaseOwnership(owned);
  }
}

export async function clearClipboardIfUnchanged(): Promise<void> {
  const owned = lastCopy;
  if (owned) await clearOwnedClipboard(owned);
}

/** Schedule native clearing of the secret-bearing binary entry format. */
export async function scheduleExchangeWipe(): Promise<void> {
  operationEpoch += 1;
  cancelRendererTimer();
  releaseOwnership(lastCopy);
  const seconds = get(appSettings).security.clipboardClearSeconds;
  if (isTauriRuntime()) {
    try {
      await invoke("clipboard_schedule_exchange_wipe", { seconds });
    } catch {
      // best-effort; never fall back to text for a binary payload
    }
  }
}

/** Lock-time clearing invalidates outstanding renderer ownership work. */
export async function clearClipboard(): Promise<void> {
  operationEpoch += 1;
  cancelRendererTimer();
  releaseOwnership(lastCopy);
  // An already-started browser write cannot be cancelled. Drain it before the
  // explicit clear, and invalidate queued copies synchronously above.
  await serializeWrite(clearRawClipboard);
}

async function clearRawClipboard(): Promise<void> {
  if (isTauriRuntime()) {
    try {
      await invoke("clipboard_cancel_scheduled_wipe");
    } catch {
      // Continue with explicit clearing even if cancellation is unavailable.
    }
  }
  try {
    if (isTauriRuntime()) {
      const cleared = await invoke<boolean>("clipboard_clear");
      if (cleared) return;
    }
    await copyRaw("");
  } catch {
    try {
      await copyRaw("");
    } catch {
      // Clipboard access unavailable; locking still closes the vault.
    }
  }
}

async function copyRaw(text: string): Promise<void> {
  if (navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text);
    return;
  }
  const textarea = document.createElement("textarea");
  textarea.value = text;
  textarea.style.position = "fixed";
  textarea.style.opacity = "0";
  try {
    document.body.appendChild(textarea);
    textarea.select();
    if (!document.execCommand("copy")) throw new Error("复制失败");
  } finally {
    textarea.value = "";
    textarea.remove();
  }
}
