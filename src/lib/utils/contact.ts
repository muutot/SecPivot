/** Contact-kind detection for field values and free text (custom fields, notes).
 *
 * - `classifyContact` decides whether a whole single-line value is a URL, email,
 *   or phone number so custom-field values can render as actionable links.
 * - `detectContacts` extracts every URL, email, and phone number from free text
 *   (kept for tests and any future strip consumers).
 * - `linkifyContacts` splits free text into a token stream that marks inline
 *   URLs, emails, and phone numbers so notes can render them as clickable links
 *   without losing the surrounding text.
 */

export type ContactKind = "url" | "email" | "phone";

export interface ContactMatch {
  kind: ContactKind;
  value: string;
}

export type ContactToken = { kind: "text"; value: string } | { kind: ContactKind; value: string };

const EMAIL_WHOLE_RE =
  /^[a-z0-9._%+-]+@[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+$/i;
const URL_SCHEME_WHOLE_RE = /^(?:https?:\/\/|ftp:\/\/|www\.)[^\s]+$/i;
const URL_DOMAIN_WHOLE_RE =
  /^(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}(?::\d{1,5})?(?:[\/?#][^\s]*)?$/i;
const PHONE_WHOLE_RE =
  /^(?:(?:\+[1-9]\d{0,2}[-\s]?)?(?:\(\d{2,4}\)[-\s]?)?(?:\d{3,4}[-\s]?\d{3,4}[-\s]?\d{4}|\d{11})|(?:0\d{2,3}[-\s]?\d{7,8}))$/;

const EMAIL_SRC =
  "[a-z0-9._%+-]+@[a-z0-9](?:[a-z0-9-]*[a-z0-9])?(?:\\.[a-z0-9](?:[a-z0-9-]*[a-z0-9])?)+";
const URL_SRC =
  "(?:https?:\\/\\/|ftp:\\/\\/|www\\.)[^\\s<>\"'()\\u3000-\\u303f\\uff00-\\uffef\\u4e00-\\u9fff]+";
const PHONE_SRC =
  "(?:(?:\\+[1-9]\\d{0,2}[-\\s]?)?(?:\\(\\d{2,4}\\)[-\\s]?)?(?:\\d{3,4}[-\\s]?\\d{3,4}[-\\s]?\\d{4}|\\d{11})|(?:0\\d{2,3}[-\\s]?\\d{7,8}))";

const EMAIL_RE = new RegExp(EMAIL_SRC, "gi");
const URL_RE = new RegExp(URL_SRC, "gi");
const PHONE_RE = new RegExp(PHONE_SRC, "g");

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

interface RawMatch {
  kind: ContactKind;
  start: number;
  end: number;
  raw: string;
  clean: string;
}

const TRAILING_PUNCT_RE = /[.,;:!?)\]，。、！？；：》」』]+$/;

/** Split free text into a token stream that flags inline contacts. Matches are
 *  merged by position (earliest wins) so a URL is never double-linked when it
 *  contains an email- or phone-shaped fragment, and the surrounding text (newlines,
 *  whitespace, trailing punctuation) is preserved verbatim as plain tokens. */
export function linkifyContacts(text: string): ContactToken[] {
  const matches: RawMatch[] = [];
  const collect = (kind: ContactKind, re: RegExp): void => {
    for (const m of text.matchAll(new RegExp(re.source, "gi"))) {
      const raw = m[0];
      const clean = kind === "url" ? raw.replace(TRAILING_PUNCT_RE, "") : raw;
      if (!clean) continue;
      matches.push({ kind, start: m.index ?? 0, end: (m.index ?? 0) + raw.length, raw, clean });
    }
  };
  collect("email", EMAIL_RE);
  collect("phone", PHONE_RE);
  collect("url", URL_RE);
  matches.sort((a, b) => a.start - b.start || a.end - b.end);

  const tokens: ContactToken[] = [];
  const pushText = (value: string): void => {
    if (!value) return;
    const last = tokens[tokens.length - 1];
    if (last && last.kind === "text") last.value += value;
    else tokens.push({ kind: "text", value });
  };
  let cursor = 0;
  for (const m of matches) {
    if (m.start < cursor) continue;
    pushText(text.slice(cursor, m.start));
    tokens.push({ kind: m.kind, value: m.clean });
    pushText(m.raw.slice(m.clean.length));
    cursor = m.end;
  }
  pushText(text.slice(cursor));
  return tokens;
}
