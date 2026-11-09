export interface CsvRow {
  group: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  totp: string;
  favorite: boolean;
}

function escapeCsv(value: string): string {
  // Spreadsheet formula-injection guard: a leading =, +, -, @, tab or CR
  // would execute as a formula when the export is opened in Excel or
  // LibreOffice; a leading apostrophe forces text interpretation.
  // Guard the value before RFC quoting so separators stay inside the cell.
  const cell = /^[=+\-@\t\r]/.test(value) ? `'${value}` : value;
  return /[",\r\n]/.test(cell) ? `"${cell.replace(/"/g, '""')}"` : cell;
}

/** Serialize entries as RFC 4180 CSV (CRLF, quoted cells, UTF-8 BOM). */
export function buildCsv(rows: CsvRow[]): string {
  const header = ["Group", "Title", "Username", "Password", "URL", "Notes", "TOTP", "Favorite"];
  const lines = [
    header,
    ...rows.map((row) => [
      row.group,
      row.title,
      row.username,
      row.password,
      row.url,
      row.notes,
      row.totp,
      row.favorite ? "true" : "false",
    ]),
  ];
  const body = lines.map((line) => line.map(escapeCsv).join(",")).join("\r\n");
  return `\uFEFF${body}\r\n`;
}

/** Content-free syntax error; import callers translate it for the UI. */
export class CsvParseError extends Error {
  constructor() {
    super("Invalid CSV quoting");
    this.name = "CsvParseError";
  }
}

/** Parse quoted CSV text (optional UTF-8 BOM, CRLF/LF/CR records).
 * Reject malformed quotes before returning any rows. */
export function parseCsv(text: string): string[][] {
  const src = text.replace(/^\uFEFF/, "");
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let inQuotes = false;
  let closedQuote = false;
  let fieldStarted = false;
  const finishCell = () => {
    row.push(cell);
    cell = "";
    closedQuote = false;
    fieldStarted = false;
  };
  for (let i = 0; i < src.length; i++) {
    const ch = src[i];
    if (inQuotes) {
      if (ch === '"') {
        if (src[i + 1] === '"') {
          cell += '"';
          i++;
        } else {
          inQuotes = false;
          closedQuote = true;
        }
      } else {
        cell += ch;
      }
    } else if (ch === ",") {
      finishCell();
    } else if (ch === "\n" || ch === "\r") {
      finishCell();
      rows.push(row);
      row = [];
      if (ch === "\r" && src[i + 1] === "\n") i++;
    } else if (closedQuote) {
      throw new CsvParseError();
    } else if (ch === '"') {
      if (fieldStarted) throw new CsvParseError();
      inQuotes = true;
      fieldStarted = true;
    } else {
      cell += ch;
      fieldStarted = true;
    }
  }
  if (inQuotes) throw new CsvParseError();
  if (fieldStarted || row.length > 0) {
    finishCell();
    rows.push(row);
  }
  return rows;
}

export interface ImportCsvRow {
  group: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  totp: string;
}

const HEADERS = ["group", "title", "username", "password", "url", "notes", "totp"];
/** LastPass CSV export uses different column names for three fields. */
const HEADER_ALIASES: Record<string, string> = {
  name: "title",
  extra: "notes",
  grouping: "group",
};

/**
 * Map raw parsed cells to rows. A leading row matching the known header names
 * (or KeePass-style columns) is treated as a header; otherwise cells are read
 * positionally as group, title, username, password, url, notes.
 */
export function parseCsvRows(raw: string[][]): ImportCsvRow[] {
  let start = 0;
  let cols: number[] | null = null;
  if (raw.length > 0) {
    const normalized = raw[0].map((c) => c.trim().toLowerCase());
    const indexOf = (header: string): number => {
      const exact = normalized.indexOf(header);
      if (exact >= 0) return exact;
      // HEADER_ALIASES maps alias -> canonical (e.g. name -> title), so
      // resolve the canonical header back to any alias present in the row.
      for (const [alias, canonical] of Object.entries(HEADER_ALIASES)) {
        if (canonical === header) {
          const hit = normalized.indexOf(alias);
          if (hit >= 0) return hit;
        }
      }
      return -1;
    };
    const indices = HEADERS.map(indexOf);
    // Require at least two known columns: a data row can legitimately contain
    // a single header-like cell (e.g. a title of "url"), which must not be
    // mistaken for a header row and silently dropped.
    if (indices.filter((i) => i >= 0).length >= 2) {
      cols = indices;
      start = 1;
    }
  }
  const rows: ImportCsvRow[] = [];
  for (let i = start; i < raw.length; i++) {
    const cells = raw[i];
    if (cells.every((c) => c.trim() === "")) continue;
    const pick = (index: number): string =>
      (cols ? cells[cols[index]] : (cells[index + 1] ?? cells[index])) ?? "";
    const row: ImportCsvRow = cols
      ? {
          group: pick(0),
          title: pick(1),
          username: pick(2),
          password: pick(3),
          url: pick(4),
          notes: pick(5),
          totp: pick(6),
        }
      : {
          group: cells[0] ?? "",
          title: cells[1] ?? "",
          username: cells[2] ?? "",
          password: cells[3] ?? "",
          url: cells[4] ?? "",
          notes: cells[5] ?? "",
          totp: cells[6] ?? "",
        };
    if (!row.title && !row.username && !row.password && !row.url && !row.notes && !row.totp)
      continue;
    rows.push(row);
  }
  return rows;
}
