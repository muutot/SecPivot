import { vault } from "$lib/services/vault";
import { copyText } from "$lib/utils/clipboard";

/** Reactive one-time-password state shared by every OTP skin (detail widget,
 *  list badge). Centralizes the per-period fetch loop, HOTP static-code
 *  handling, countdown and copy+flash behavior so one change fixes every
 *  surface. All instances for the same session + entry uuid share one fetch loop and
 *  one countdown clock, so rendering N badges never means N IPC round-trips
 *  per period (a shared single ticker drives every entry). */
export interface TotpCodeState {
  code: string;
  remaining: number;
  period: number;
  kind: "totp" | "hotp" | "steam";
  counter: number | undefined;
  /** Empty when the last fetch succeeded, otherwise the error message. */
  error: string;
  copied: boolean;
  isHotp: boolean;
  /** 0..1 fraction of the period remaining (0 for HOTP). */
  fraction: number;
  copy: () => Promise<void>;
  refresh: () => Promise<boolean>;
}

/** Per-entry shared state, keyed by stable session id + entry uuid. Lives outside Svelte's
 *  reactivity (module scope); each `useTotpCode` instance copies the values
 *  it needs into its own `$state` via a listener. */
interface SharedTotp {
  refcount: number;
  listeners: Set<() => void>;
  code: string;
  /** Absolute timestamp when the current code expires (`0` while unknown). */
  validUntil: number;
  period: number;
  kind: "totp" | "hotp" | "steam";
  counter: number | undefined;
  error: string;
  fetching: boolean;
}

const cache = new Map<string, SharedTotp>();
let tickTimer: ReturnType<typeof setInterval> | undefined;
let lastTick = Date.now();
let visibilityBound = false;

function newShared(): SharedTotp {
  return {
    refcount: 0,
    listeners: new Set(),
    code: "",
    validUntil: 0,
    period: 30,
    kind: "totp",
    counter: undefined,
    error: "",
    fetching: false,
  };
}

function cacheKey(sessionId: string, uuid: string): string {
  return `${sessionId}\u0000${uuid}`;
}

function notify(key: string): void {
  const entry = cache.get(key);
  if (!entry) return;
  for (const listener of entry.listeners) listener();
}

/** Remaining seconds of the current code, from the expiry timestamp. */
function remainingOf(entry: SharedTotp): number {
  if (entry.validUntil <= 0) return 0;
  return Math.max(0, Math.ceil((entry.validUntil - Date.now()) / 1000));
}

/** Fetch (and cache) the current code for `uuid`. Transient failures never
 *  kill the loop: the next period boundary retries, so a one-off IPC hiccup
 *  does not freeze the badge on a stale code. */
async function fetchEntry(key: string, uuid: string, entry: SharedTotp): Promise<void> {
  if (entry.fetching) return;
  entry.fetching = true;
  try {
    const sessionId = key.slice(0, key.indexOf("\u0000"));
    const result = await vault.callInSession(sessionId, () => vault.totpCode(uuid));
    entry.code = result.code;
    entry.period = result.period;
    entry.kind = result.kind;
    entry.counter = result.counter;
    entry.validUntil = Date.now() + result.validFor * 1000;
    entry.error = "";
  } catch (e) {
    // Keep showing "无法生成验证码" and retry once per period instead of
    // hammering every second or stopping forever.
    entry.code = "";
    entry.error = String(e);
    entry.validUntil = Date.now() + Math.max(entry.period, 1) * 1000;
  } finally {
    entry.fetching = false;
    notify(key);
  }
}

/** Single 1 s clock shared by every cached entry. Decrements each countdown
 *  from its wall-clock expiry (drift-free) and re-fetches codes that expired
 *  while the loop was paused. */
function tick(): void {
  if (typeof document !== "undefined" && document.hidden) return;
  const now = Date.now();
  const elapsed = Math.floor((now - lastTick) / 1000);
  lastTick = now;
  if (elapsed < 1) return;
  for (const [key, entry] of cache) {
    if (entry.kind === "hotp") continue;
    if (entry.validUntil > 0 && now >= entry.validUntil) {
      const uuid = key.slice(key.indexOf("\u0000") + 1);
      void fetchEntry(key, uuid, entry);
    }
    notify(key);
  }
}

function ensureTicker(): void {
  if (tickTimer) return;
  lastTick = Date.now();
  tickTimer = setInterval(tick, 1000);
  if (typeof document !== "undefined" && !visibilityBound) {
    visibilityBound = true;
    document.addEventListener("visibilitychange", () => {
      // On return to a visible window, resync every countdown immediately so
      // badges are never stale after a pause.
      if (document.hidden) return;
      lastTick = Date.now() - 1000;
      tick();
    });
  }
}

function stopTickerIfIdle(): void {
  if (cache.size === 0 && tickTimer) {
    clearInterval(tickTimer);
    tickTimer = undefined;
  }
}

export function useTotpCode(getUuid: () => string): TotpCodeState {
  let sessionId = $state(vault.getActiveSessionId() ?? "browser");
  let code = $state("");
  let remaining = $state(0);
  let period = $state(30);
  let kind = $state<"totp" | "hotp" | "steam">("totp");
  let counter = $state<number | undefined>(undefined);
  let error = $state("");
  let copied = $state(false);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  $effect(() =>
    vault.activeId.subscribe((value) => {
      sessionId = value ?? "browser";
    }),
  );

  /** Subscribe to the shared per-entry state; one fetch loop and one countdown
   *  drive every badge for the same entry. Re-runs if `getUuid()` changes. */
  $effect(() => {
    const uuid = getUuid();
    const key = cacheKey(sessionId, uuid);
    let entry = cache.get(key);
    if (!entry) {
      entry = newShared();
      cache.set(key, entry);
      void fetchEntry(key, uuid, entry);
    }
    entry.refcount += 1;
    const sync = (): void => {
      code = entry.code;
      remaining = remainingOf(entry);
      period = entry.period;
      kind = entry.kind;
      counter = entry.counter;
      error = entry.error;
    };
    entry.listeners.add(sync);
    sync();
    ensureTicker();
    return () => {
      if (copiedTimer) clearTimeout(copiedTimer);
      copiedTimer = undefined;
      entry.listeners.delete(sync);
      entry.refcount -= 1;
      if (entry.refcount === 0) {
        cache.delete(key);
        stopTickerIfIdle();
      }
    };
  });

  const isHotp = $derived(kind === "hotp");
  const fraction = $derived(period > 0 ? Math.max(0, remaining) / period : 0);

  async function copy(): Promise<void> {
    const key = cacheKey(sessionId, getUuid());
    const entry = cache.get(key);
    if (!entry || !entry.code) return;
    try {
      await copyText(entry.code);
      copied = true;
      if (copiedTimer) clearTimeout(copiedTimer);
      copiedTimer = setTimeout(() => {
        copied = false;
        copiedTimer = undefined;
      }, 1200);
    } catch {
      // clipboard unavailable; ignore
    }
  }

  async function refresh(): Promise<boolean> {
    const uuid = getUuid();
    const key = cacheKey(sessionId, uuid);
    const entry = cache.get(key);
    if (!entry) return false;
    await fetchEntry(key, uuid, entry);
    return entry.error === "";
  }

  return {
    get code() {
      return code;
    },
    get remaining() {
      return remaining;
    },
    get period() {
      return period;
    },
    get kind() {
      return kind;
    },
    get counter() {
      return counter;
    },
    get error() {
      return error;
    },
    get copied() {
      return copied;
    },
    get isHotp() {
      return isHotp;
    },
    get fraction() {
      return fraction;
    },
    copy,
    refresh,
  };
}
