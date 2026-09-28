//! Plain-text rendering of a whole entry for the entry right-click menu's
//! "Copy entry" action. Pure and secret-agnostic: it formats whatever the
//! caller resolved, so the layout is unit-testable and the page stays the only
//! place that touches the vault session.

/** Field labels for {@link formatEntryText}; supplied by the caller so the
 *  output follows the UI locale. */
export type EntryCopyLabels = {
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
};

/** The subset of an entry the copy renders. `password` is `null`/`undefined`
 *  when the caller could not resolve it (locked session, read failure). */
export type EntryCopySource = {
  title?: string;
  username?: string;
  password?: string | null;
  url?: string;
  notes?: string;
  customFields?: { name: string; value?: string; protected?: boolean }[];
};

/**
 * Render an entry as `label: value` lines separated by newlines, in KeePass
 * field order (title, username, password, URL, notes, custom fields). Empty
 * fields are omitted so a minimal entry copies just its title.
 *
 * Protected custom fields arrive with an empty value (the snapshot never
 * carries the secret) and are therefore omitted as well — the copy is limited
 * to what the renderer already holds. Multi-line values (notes, custom field
 * values) keep their inner newlines; only the first line carries the label.
 */
export function formatEntryText(entry: EntryCopySource, labels: EntryCopyLabels): string {
  const lines: string[] = [];
  const push = (label: string, value: string | null | undefined): void => {
    if (value) lines.push(`${label}: ${value}`);
  };

  push(labels.title, entry.title);
  push(labels.username, entry.username);
  push(labels.password, entry.password);
  push(labels.url, entry.url);
  push(labels.notes, entry.notes);
  for (const field of entry.customFields ?? []) {
    push(field.name, field.value);
  }
  return lines.join("\n");
}
