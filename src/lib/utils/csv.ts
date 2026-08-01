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
