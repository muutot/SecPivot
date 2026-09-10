import type { I18nKey } from "./zh-CN";

/** English dictionary. The `Record<I18nKey, string>` type enforces key
 *  parity with `zh-CN.ts` at compile time: adding a key to either side
 *  without the other fails `npm run check`. */

export const en: Record<I18nKey, string> = {
  "common.ok": "OK",
  "common.cancel": "Cancel",
  "common.close": "Close",
  "common.save": "Save",
  "common.delete": "Delete",
  "common.retry": "Retry",
  "common.confirm": "Confirm",
  "common.back": "Back",
};
