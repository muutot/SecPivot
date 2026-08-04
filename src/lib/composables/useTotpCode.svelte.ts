import { vault } from "$lib/services/vault";
import { copyText } from "$lib/utils/clipboard";

/** Reactive one-time-password state shared by every OTP skin (detail widget,
 *  list badge). Centralizes the per-period fetch loop, HOTP static-code
 *  handling, countdown and copy+flash behavior so one change fixes every
 *  surface. */
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

export function useTotpCode(getUuid: () => string): TotpCodeState {
  let code = $state("");
  let remaining = $state(0);
  let period = $state(30);
  let kind = $state<"totp" | "hotp" | "steam">("totp");
  let counter = $state<number | undefined>(undefined);
  let error = $state("");
  let copied = $state(false);
  let copiedTimer: ReturnType<typeof setTimeout> | undefined;

  async function refresh(): Promise<boolean> {
    try {
      const result = await vault.totpCode(getUuid());
      code = result.code;
      remaining = result.validFor;
      period = result.period;
      kind = result.kind;
      counter = result.counter;
      error = "";
      return true;
    } catch (e) {
      code = "";
      error = String(e);
      return false;
    }
  }

  $effect(() => {
    let timer: ReturnType<typeof setInterval> | undefined;
    const tick = async (): Promise<void> => {
      // A failing seed (invalid TOTP URI) must stop the per-second loop
      // instead of hammering the backend forever.
      if (!(await refresh()) && timer) clearInterval(timer);
    };
    void tick();
    // HOTP is counter-driven, not clock-driven: fetch once, never count down
    // (the counter only advances when a code is requested).
    if (kind !== "hotp") {
      timer = setInterval(() => {
        remaining -= 1;
        if (remaining <= 0) void tick();
      }, 1000);
    }
    return () => {
      if (timer) clearInterval(timer);
    };
  });

  const isHotp = $derived(kind === "hotp");
  const fraction = $derived(period > 0 ? Math.max(0, remaining) / period : 0);

  async function copy(): Promise<void> {
    if (!code) return;
    try {
      await copyText(code);
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

  return {
    code,
    remaining,
    period,
    kind,
    counter,
    error,
    copied,
    isHotp,
    fraction,
    copy,
    refresh,
  };
}
