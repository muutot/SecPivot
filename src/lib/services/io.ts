//! Vault import/export orchestration shared by the main window: file picking,
//! format conversion, browser-fallback downloads, and the bulk group-resolving
//! import pipeline. Session staleness guards stay with the caller (the page
//! owns the `SessionViewGuard`); this module receives an `IoHost` so every
//! stale-view branch behaves exactly as before while keeping no component
//! state here.

import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
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
import type { EntryInput, ImportRow, VaultState } from "$lib/types/vault";
import { isTauriRuntime } from "$lib/services/settings";
import { vault } from "$lib/services/vault";

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
  customFields: { name: string; value: string }[];
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
  URL.revokeObjectURL(url);
}

export function csvToImportEntries(text: string): ImportEntry[] {
  return parseCsvRows(parseCsv(text)).map((row) => ({ ...row, customFields: [] }));
}

export function xmlToImportEntries(text: string): ImportEntry[] {
  return parseKdbxXml(text);
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

/** Pick an import file via the Tauri dialog (desktop) or a hidden file input,
 *  returning its text, or `null` when cancelled / unreadable / stale view. */
export async function pickImportFile(
  host: IoHost,
  view: SessionViewToken,
  filters: { name: string; extensions: string[] }[],
): Promise<string | null> {
  try {
    const result = await awaitCurrentView(host.sessionView, view, async () => {
      if (isTauriRuntime()) {
        const selected = await open({ multiple: false, filters });
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
    if (host.sessionView.isCurrent(view)) host.notify(`读取文件失败：${e}`);
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
      customFields: entry.customFields,
      attachments: [],
    }));
    await vault.callInSession(sessionId, () => vault.addEntries(inputs));
    if (!host.sessionView.isCurrent(view)) return;
    host.notify(`已导入 ${entries.length} 个条目`);
  } catch (e) {
    if (!host.sessionView.isCurrent(view)) return;
    host.notify(`导入失败：${e}`);
  } finally {
    if (host.sessionView.isCurrent(view) && host.operations.isCurrent(operation))
      host.setBusy(false);
  }
}
