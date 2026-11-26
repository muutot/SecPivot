import { get } from "svelte/store";
import { appSettings } from "$lib/services/settings";
import { showTip } from "$lib/services/tips";
import { t } from "$lib/i18n";

export function closeErrorKind(error: unknown): "cancelled" | "busy" | "changed" | "failed" {
  const text = error instanceof Error ? error.message : String(error);
  if (text.startsWith("VAULT_CLOSE_CANCELLED:")) return "cancelled";
  if (text.startsWith("VAULT_CLOSE_BUSY:")) return "busy";
  if (text.startsWith("VAULT_CLOSE_CHANGED:")) return "changed";
  return "failed";
}

export function reportCloseError(error: unknown, automatic = false): void {
  const kind = closeErrorKind(error);
  if (kind === "cancelled" || (automatic && kind === "busy")) return;
  const lang = get(appSettings).general.language;
  showTip(
    kind === "changed"
      ? t(lang, "error.closeChanged")
      : kind === "busy"
        ? t(lang, "error.closeBusy")
        : t(lang, "error.closeFailed", {
            error: error instanceof Error ? error.message : String(error),
          }),
    "error",
  );
}
