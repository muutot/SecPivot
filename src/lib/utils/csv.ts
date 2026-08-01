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
  if (/[",\r\n]/.test(value)) return `"${value.replace(/"/g, '""')}"`;
  return value;
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

/** Parse RFC 4180 CSV text (optional UTF-8 BOM, quoted cells, CRLF/LF endings). */
export function parseCsv(text: string): string[][] {
  const src = text.replace(/^\uFEFF/, "");
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = "";
  let inQuotes = false;
  for (let i = 0; i < src.length; i++) {
    const ch = src[i];
    if (inQuotes) {
      if (ch === '"') {
        if (src[i + 1] === '"') {
          cell += '"';
          i++;
        } else {
          inQuotes = false;
        }
      } else {
        cell += ch;
      }
    } else if (ch === '"') {
      inQuotes = true;
    } else if (ch === ",") {
      row.push(cell);
      cell = "";
    } else if (ch === "\n") {
      row.push(cell);
      rows.push(row);
      row = [];
      cell = "";
    } else if (ch !== "\r") {
      cell += ch;
    }
  }
  if (cell.length > 0 || row.length > 0) {
    row.push(cell);
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
    const indices = HEADERS.map((h) => normalized.indexOf(h));
    if (indices.some((i) => i >= 0)) {
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
    if (!row.title && !row.password) continue;
    rows.push(row);
  }
  return rows;
}
