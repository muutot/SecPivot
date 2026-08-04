const pad2 = (n: number): string => String(n).padStart(2, "0");

/** `YYYY-MM-DD` for a timestamp; empty string when absent, the original value
 *  when the date is unparseable (kept so column cells never blank out a raw
 *  date the user may recognize). */
export function formatDateOnly(value: string | undefined): string {
  if (!value) return "";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value;
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}`;
}

/** Convert an ISO-8601 UTC timestamp to the `datetime-local` input format
 *  (`YYYY-MM-DDTHH:mm`), or `""` when unparseable. */
export function toDateTimeInput(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return `${d.getFullYear()}-${pad2(d.getMonth() + 1)}-${pad2(d.getDate())}T${pad2(
    d.getHours(),
  )}:${pad2(d.getMinutes())}`;
}

/** Localized long date (`zh-CN`) for a timestamp; `—` for absent/unparseable. */
export function formatLocalDate(value: string | undefined): string {
  if (!value) return "—";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return "—";
  return d.toLocaleDateString("zh-CN", { year: "numeric", month: "2-digit", day: "2-digit" });
}
