//! Vault import/export orchestration shared by the main window: file picking,
//! format conversion, browser-fallback downloads, and the bulk group-resolving
//! import pipeline. Session staleness guards stay with the caller (the page
//! owns the `SessionViewGuard`); this module receives an `IoHost` so every
//! stale-view branch behaves exactly as before while keeping no component
//! state here.

import { invoke } from "@tauri-apps/api/core";
import { open, save, type SaveDialogOptions } from "@tauri-apps/plugin-dialog";
import { buildCsv, parseCsv, parseCsvRows, type CsvRow } from "$lib/utils/csv";
import { buildKeePassXml, parseKdbxXml, type XmlExportRow } from "$lib/utils/kdbx-xml";
import { resolveImportGroupPath, type ImportGroupResolver } from "$lib/utils/import-groups";
import {
  LatestOperationGuard,
  SessionViewGuard,
  awaitCurrentView,
  type SessionViewToken,
} from "$lib/utils/session-state";
import { buildGroupPathIndex, findGroupIn } from "$lib/utils/tree";
import type { CustomField, EntryInput, ImportRow, VaultState } from "$lib/types/vault";
import type { Language } from "$lib/types/settings";
import { get } from "svelte/store";
import { isAndroid, isTauriRuntime, appSettings } from "$lib/services/settings";
import { createDocument, openDocument } from "$lib/services/saf";
import { t } from "$lib/i18n";
import { vault } from "$lib/services/vault";

/** Current UI locale for toasts (read lazily: the language can change at
 *  runtime via settings). */
function lang() {
  return get(appSettings).general.language;
}

/** Component hooks the IO flows need; see `IoHost` docs per method. */
export type IoHost = {
  /** Shared session-staleness guard owned by the page. */
  sessionView: SessionViewGuard;
  /** Busy-token guard invalidated on tab switches. */
  operations: LatestOperationGuard;
  /** Toolbar busy flag (reset by the page when the tab changes). */
  setBusy: (busy: boolean) => void;
  /** Toast feedback. */
  notify: (message: string) => void;
  /** Current vault snapshot (or `null` when closed). */
  currentState: () => VaultState | null;
};

/** Normalized import row shared by the CSV, KeePass-XML, Bitwarden and
 *  1Password importers. */
export type ImportEntry = {
  group: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  totp?: string;
  /** ISO-8601 expiry; empty/absent disables expiry (only KeePass XML supplies it). */
  expires?: string;
  customFields: CustomField[];
};

/** One flattened export row: entry fields plus its resolved group path. All
 *  entry properties are optional so a full `VaultEntry` is assignable. */
export type ExportSourceRow = {
  entry: {
    title?: string;
    username?: string;
    password?: string | null;
    url?: string;
    notes?: string;
    totp?: string;
    expires?: string;
    customFields?: CustomField[];
    favorite?: boolean | null;
  };
  path: string;
};

/** Map flattened report rows to the CSV exporter input (with the Favorite
 *  column; KeePass XML has no such concept). */
export function toCsvExportRows(source: ExportSourceRow[]): CsvRow[] {
  return source.map(({ entry, path }) => ({
    group: path,
    title: entry.title ?? "",
    username: entry.username ?? "",
    password: entry.password ?? "",
    url: entry.url ?? "",
    notes: entry.notes ?? "",
    totp: entry.totp ?? "",
    favorite: entry.favorite === true,
  }));
}

/** Map flattened report rows to the KeePass XML exporter input. */
export function toXmlExportRows(source: ExportSourceRow[]): XmlExportRow[] {
  return source.map(({ entry, path }) => ({
    group: path,
    title: entry.title ?? "",
    username: entry.username ?? "",
    password: entry.password ?? "",
    url: entry.url ?? "",
    notes: entry.notes ?? "",
    totp: entry.totp ?? "",
    expires: entry.expires,
    customFields: entry.customFields?.map((field) => ({ ...field })),
  }));
}

/** Browser-fallback download used by the demo-mode exporters. */
export function downloadTextFile(content: string, fileName: string, mime: string): void {
  const blob = new Blob([content], { type: mime });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = fileName;
  anchor.click();
  // Revoking synchronously aborts the download in Firefox/Safari; Chromium
  // tolerates it, which is why the empty-file failure stayed hidden.
  window.setTimeout(() => URL.revokeObjectURL(url), 4000);
}

export function csvToImportEntries(text: string): ImportEntry[] {
  return parseCsvRows(parseCsv(text)).map((row) => ({ ...row, customFields: [] }));
}

export function xmlToImportEntries(text: string, lang: Language): ImportEntry[] {
  return parseKdbxXml(text, lang).map((row) => ({
    ...row,
    totp: row.totp || undefined,
    expires: row.expires || undefined,
  }));
}

/** Shared row mapping for the JSON-based importers (Bitwarden, 1Password). */
export function importRowsToEntries(rows: ImportRow[]): ImportEntry[] {
  return rows.map((row) => ({
    group: row.group,
    title: row.title,
    username: row.username,
    password: row.password,
    url: row.url,
    notes: row.notes,
    totp: row.totp || undefined,
    customFields: row.customFields,
  }));
}

/** Native destination picker shared by plaintext exports and attachment saves.
 *  Callers keep their captured view guard around this promise before reading
 *  secrets or starting backend writes. This never remembers an export as a vault. */
export async function pickSaveFile(
  options: Pick<SaveDialogOptions, "filters"> & { defaultPath: string },
): Promise<string | null> {
  if (isAndroid()) {
    const types: Record<string, string> = {
      csv: "text/csv",
      xml: "application/xml",
      html: "text/html",
    };
    const mimeTypes = [
      ...new Set(
        (options.filters ?? [])
          .flatMap((filter) => filter.extensions)
          .map((ext) => types[ext.replace(/^\./, "").toLowerCase()])
          .filter(Boolean),
      ),
    ];
    const document = await createDocument(
      options.defaultPath,
      mimeTypes.length ? mimeTypes : ["application/octet-stream"],
    );
    return document?.uri || null;
  }
  return await save(options);
}

function readPickedFile(accept: string): Promise<string | null> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept;
    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) {
        resolve(null);
        return;
      }
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result ?? ""));
      reader.onerror = () => resolve(null);
      reader.readAsText(file);
    };
    input.click();
  });
}

/** Pick an import file via SAF (Android), the Tauri dialog (desktop), or a hidden file input,
 *  returning its text, or `null` when cancelled / unreadable / stale view. */
export async function pickImportFile(
  host: IoHost,
  view: SessionViewToken,
  filters: { name: string; extensions: string[] }[],
): Promise<string | null> {
  try {
    const result = await awaitCurrentView(host.sessionView, view, async () => {
      if (isTauriRuntime()) {
        if (isAndroid()) {
          // Android providers often report CSV/1PIF as plain text or generic
          // binary. The backend checks DISPLAY_NAME rather than the opaque URI.
          const types: Record<string, string> = {
            csv: "text/csv",
            xml: "application/xml",
            json: "application/json",
            "1pif": "application/octet-stream",
          };
          const mimeTypes = [
            ...new Set([
              ...filters
                .flatMap((filter) => filter.extensions)
                .map((ext) => types[ext.replace(/^\./, "").toLowerCase()])
                .filter(Boolean),
              ...(filters.some((filter) => filter.extensions.some((ext) => /^\.?xml$/i.test(ext)))
                ? ["text/xml"]
                : []),
              "text/plain",
              "application/octet-stream",
            ]),
          ];
          const picked = await openDocument(mimeTypes);
          if (!picked?.uri || !host.sessionView.isCurrent(view)) return null;
          return await invoke<string>("read_text_file", {
            path: picked.uri,
            fileName: picked.name ?? null,
          });
        }
        const selected = await open({ multiple: false, filters });
        if (!host.sessionView.isCurrent(view)) return null;
        return selected ? await invoke<string>("read_text_file", { path: String(selected) }) : null;
      }
      // Mirror the native dialog's filters in the hidden input's accept list.
      const accept = filters
        .flatMap((f) => f.extensions)
        .map((ext) => (ext === "*" ? "" : `.${ext.replace(/^\./, "")}`))
        .filter(Boolean)
        .join(",");
      return await readPickedFile(accept);
    });
    return result.current ? result.value : null;
  } catch (e) {
    if (host.sessionView.isCurrent(view)) host.notify(t(lang(), "io.readFailed", { e: String(e) }));
    return null;
  }
}

/** Resolve each row's group and add it as an entry; reports a one-shot summary.
 *  Every unique group path is resolved once (creating missing groups), then all
 *  entries are bulk-inserted in a single IPC call instead of one `add_entry`
 *  round-trip per row. */
export async function importEntries(
  host: IoHost,
  entries: ImportEntry[],
  view: SessionViewToken,
  baseGroupUuid: string | null,
): Promise<void> {
  if (!host.sessionView.isCurrent(view)) return;
  const startState = host.currentState();
  if (!startState) return;
  const { sessionId } = view;
  const operation = host.operations.begin();
  host.setBusy(true);
  try {
    const groupCache = new Map<string, string>();
    const resolver: ImportGroupResolver<VaultState> = {
      state: startState,
      baseUuid: baseGroupUuid ?? startState.root.uuid,
      groups: buildGroupPathIndex(startState.root),
    };
    for (const entry of entries) {
      if (!groupCache.has(entry.group)) {
        const groupUuid = await resolveImportGroupPath({
          path: entry.group,
          sessionId,
          lang: lang(),
          resolver,
          createGroup: (ownerId, parentUuid, name) =>
            vault.callInSession(ownerId, () => vault.addGroup({ parentUuid, name })),
          findCreatedUuid: (state, parentUuid, name) => {
            const parent = findGroupIn(state.root, parentUuid);
            return parent?.children.find((group) => group.name === name)?.uuid ?? null;
          },
        });
        groupCache.set(entry.group, groupUuid);
      }
    }
    const inputs: EntryInput[] = entries.map((entry) => ({
      groupUuid: groupCache.get(entry.group)!,
      title: entry.title,
      username: entry.username,
      password: entry.password,
      url: entry.url,
      notes: entry.notes,
      totp: entry.totp || undefined,
      expires: entry.expires || undefined,
      customFields: entry.customFields,
      attachments: [],
    }));
    await vault.callInSession(sessionId, () => vault.addEntries(inputs));
    if (!host.sessionView.isCurrent(view)) return;
    host.notify(t(lang(), "io.imported", { n: entries.length }));
  } catch (e) {
    if (!host.sessionView.isCurrent(view)) return;
    host.notify(t(lang(), "io.importFailed", { e: String(e) }));
  } finally {
    if (host.sessionView.isCurrent(view) && host.operations.isCurrent(operation))
      host.setBusy(false);
  }
}
