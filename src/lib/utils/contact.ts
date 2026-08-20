/** Contact-kind detection for field values and free text (custom fields, notes).
 *
 * - `classifyContact` decides whether a whole single-line value is a URL, email,
 *   or phone number so custom-field values can render as actionable links.
 * - `detectContacts` extracts every URL, email, and phone number from free text
 *   for the notes detection strip.
 */

export type ContactKind = "url" | "email" | "phone";

export interface ContactMatch {
  kind: ContactKind;
  value: string;
}

const EMAIL_WHOLE_RE =
  /^[a-z0-9._%+-]+@[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+$/i;
const URL_SCHEME_WHOLE_RE = /^(?:https?:\/\/|ftp:\/\/|www\.)[^\s]+$/i;
const URL_DOMAIN_WHOLE_RE =
  /^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}(?::\d{1,5})?(?:[\/?#][^\s]*)?$/i;
const PHONE_WHOLE_RE =
  /^(?:(?:\+[1-9]\d{0,2}[-\s]?)?(?:\(\d{2,4}\)[-\s]?)?(?:\d{3,4}[-\s]?\d{3,4}[-\s]?\d{4}|\d{11})|(?:0\d{2,3}[-\s]?\d{7,8}))$/;

const EMAIL_RE =
  /[a-z0-9._%+-]+@[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+/gi;
const URL_RE =
  /(?:https?:\/\/|ftp:\/\/|www\.)[^\s<>"'()\u3000-\u303f\uff00-\uffef\u4e00-\u9fff]+/gi;
const PHONE_RE =
  /(?:(?:\+[1-9]\d{0,2}[-\s]?)?(?:\(\d{2,4}\)[-\s]?)?(?:\d{3,4}[-\s]?\d{3,4}[-\s]?\d{4}|\d{11})|(?:0\d{2,3}[-\s]?\d{7,8}))/g;

/** Classify a whole trimmed value as a URL, email, phone, or none of those. */
export function classifyContact(value: string): ContactKind | null {
  const v = value.trim();
  if (!v) return null;
  if (URL_SCHEME_WHOLE_RE.test(v)) return "url";
  if (URL_DOMAIN_WHOLE_RE.test(v)) return "url";
  if (EMAIL_WHOLE_RE.test(v)) return "email";
  if (PHONE_WHOLE_RE.test(v)) return "phone";
  return null;
}

/** Extract distinct URLs, emails, and phone numbers from free text. */
export function detectContacts(text: string): ContactMatch[] {
  const results: ContactMatch[] = [];
  const seen = new Set<string>();
  const push = (kind: ContactKind, raw: string): void => {
    const value = kind === "url" ? raw.replace(/[.,;:!?)\]，。、！？；：》」』]+$/, "") : raw;
    if (!value) return;
    const key = `${kind}\u0000${value.toLowerCase()}`;
    if (seen.has(key)) return;
    seen.add(key);
    results.push({ kind, value });
  };
  for (const match of text.matchAll(EMAIL_RE)) push("email", match[0]);
  for (const match of text.matchAll(PHONE_RE)) push("phone", match[0]);
  for (const match of text.matchAll(URL_RE)) push("url", match[0]);
  return results;
}
