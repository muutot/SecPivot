//! Favicon download flow for the main window: progress-dialog state machine
//! around `vault.downloadFavicons`, with live `favicon-progress` events gated
//! by session staleness. The page owns the dialog `$state` and hands setters
//! in via `FaviconFlowHost` so every stale-view branch behaves exactly as
//! before this was extracted from `+page.svelte`.

import { listen } from "@tauri-apps/api/event";
import { get } from "svelte/store";
import { isTauriRuntime, appSettings } from "$lib/services/settings";
import { t } from "$lib/i18n";
import { vault } from "$lib/services/vault";
import {
  LatestOperationGuard,
  SessionViewGuard,
  type SessionViewToken,
} from "$lib/utils/session-state";

/** Current UI locale for toasts/dialog text (read lazily: the language can
 *  change at runtime via settings). */
function lang() {
  return get(appSettings).general.language;
}
import type { FaviconProgress } from "$lib/types/vault";

export type FaviconDialogState = {
  phase: "working" | "done";
  progress: FaviconProgress;
  result: string;
  error: boolean;
};

export type FaviconFlowHost = {
  /** Shared session-staleness guard owned by the page. */
  sessionView: SessionViewGuard;
  /** Busy-token guard invalidated on tab switches. */
  operations: LatestOperationGuard;
  isBusy: () => boolean;
  setBusy: (busy: boolean) => void;
  notify: (message: string) => void;
  setDialog: (state: FaviconDialogState | null) => void;
};

/** Download favicons (all entries, or the given uuid subset) while streaming
 *  progress into the page's dialog. No-op when busy, outside Tauri, or when
 *  no session view can be captured. */
export async function runFaviconDownload(
  host: FaviconFlowHost,
  uuids: string[] | undefined,
  noneMessage: string,
): Promise<void> {
  if (host.isBusy()) return;
  if (!isTauriRuntime()) {
    host.notify(t(lang(), "browser.unsupported", { feature: t(lang(), "browser.featFavicon") }));
    return;
  }
  const view: SessionViewToken | null = host.sessionView.capture();
  if (!view) return;
  const { sessionId } = view;
  const operation = host.operations.begin();
  host.setBusy(true);
  host.setDialog({
    phase: "working",
    progress: { sessionId, done: 0, total: 0 },
    result: t(lang(), "favicon.dlConnecting"),
    error: false,
  });
  try {
    const unlisten = await listen<FaviconProgress>("favicon-progress", (e) => {
      if (e.payload.sessionId !== sessionId || !host.sessionView.isCurrent(view)) return;
      host.setDialog({
        phase: "working",
        progress: e.payload,
        result: t(lang(), "favicon.dlProgress", { done: e.payload.done, total: e.payload.total }),
        error: false,
      });
    });
    try {
      const report = await vault.callInSession(sessionId, () => vault.downloadFavicons(uuids));
      if (!host.sessionView.isCurrent(view)) return;
      host.setDialog({
        phase: "done",
        progress: { sessionId, done: report.attempted, total: report.attempted },
        result:
          report.attempted === 0
            ? noneMessage
            : t(lang(), "favicon.dlDone", { done: report.downloaded, total: report.attempted }),
        error: false,
      });
    } finally {
      unlisten();
    }
  } catch (e) {
    if (!host.sessionView.isCurrent(view)) return;
    host.setDialog({
      phase: "done",
      progress: { sessionId, done: 0, total: 0 },
        result: t(lang(), "favicon.dlFailed", { e: String(e) }),
      error: true,
    });
  } finally {
    if (host.sessionView.isCurrent(view) && host.operations.isCurrent(operation))
      host.setBusy(false);
  }
}

export async function cancelFaviconDownload(): Promise<void> {
  await vault.cancelFavicons();
}
