/** Parser for KeePass 2.x XML export files (`File ▸ Export ▸ KeePass XML`).
 *  Walks the `<Root><Group>` tree, mapping nested groups to `A / B` paths and
 *  each entry's `String` fields to the fixed columns (Title/UserName/Password/
 *  URL/Notes) plus a TOTP seed (the `otp`/`TimeOtp`/`HmacOtp`/`SteamOtp` keys)
 *  and any remaining non-standard `String`s as custom fields. */

export interface XmlImportEntry {
  group: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  totp: string;
  customFields: { name: string; value: string }[];
}

function childElements(el: Element): Element[] {
  return Array.from(el.children).filter((c) => c.nodeType === 1);
}

function textOf(el: Element | null | undefined): string {
  return el?.textContent?.trim() ?? "";
}

/** KeePass stores `Protected="True"` values as Base64 of the UTF-8 bytes. */
function decodeProtectedValue(value: string): string {
  const b64 = value.trim();
  if (!b64) return "";
  try {
    const decoded = atob(b64);
    const bytes = new Uint8Array(decoded.length);
    for (let i = 0; i < decoded.length; i++) bytes[i] = decoded.charCodeAt(i);
    return new TextDecoder("utf-8").decode(bytes);
  } catch {
    return value;
  }
}

function parseEntry(entry: Element, group: string): XmlImportEntry {
  const out: XmlImportEntry = {
    group,
    title: "",
    username: "",
    password: "",
    url: "",
    notes: "",
    totp: "",
    customFields: [],
  };
  for (const str of childElements(entry)) {
    if (str.tagName !== "String") continue;
    const key = textOf(childElements(str).find((e) => e.tagName === "Key"));
    if (!key) continue;
    const valueEl = childElements(str).find((e) => e.tagName === "Value");
    const protectedValue = valueEl?.getAttribute("Protected")?.toLowerCase() === "true";
    const raw = valueEl?.textContent ?? "";
    const value = protectedValue ? decodeProtectedValue(raw) : raw;
    switch (key) {
      case "Title":
        out.title = value;
        break;
      case "UserName":
        out.username = value;
        break;
      case "Password":
        out.password = value;
        break;
      case "URL":
        out.url = value;
        break;
      case "Notes":
        out.notes = value;
        break;
      case "otp":
      case "TimeOtp":
      case "HmacOtp":
      case "SteamOtp":
        if (!out.totp) out.totp = value;
        break;
      default:
        out.customFields.push({ name: key, value });
    }
  }
  return out;
}

/** Parse a KeePass 2.x XML import file into entries with `A / B` group paths
 *  relative to the import target. Throws when the document is not a KeePass
 *  XML (or the XML is malformed). */
export function parseKdbxXml(xmlText: string): XmlImportEntry[] {
  const doc = new DOMParser().parseFromString(xmlText, "text/xml");
  const parseError = doc.querySelector("parsererror");
  if (parseError) {
    throw new Error("文件不是有效的 XML");
  }
  const root = doc.querySelector("KeePassFile > Root > Group");
  if (!root) {
    throw new Error("不是有效的 KeePass XML 文件（缺少 KeePassFile/Root/Group）");
  }
  const entries: XmlImportEntry[] = [];
  const groupOf = (group: Element, parts: string[]) => {
    for (const child of childElements(group)) {
      if (child.tagName === "Entry") {
        entries.push(parseEntry(child, parts.join(" / ")));
      }
    }
    for (const sub of childElements(group)) {
      if (sub.tagName !== "Group") continue;
      const name = textOf(childElements(sub).find((e) => e.tagName === "Name"));
      groupOf(sub, [...parts, name || "未命名"]);
    }
  };
  groupOf(root, []);
  return entries;
}

/** One flat row of a vault export (mirrors the CSV export shape). */
export interface XmlExportRow {
  group: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  totp: string;
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

/** Base64 of the UTF-8 bytes, the KeePass convention for Protected values. */
function encodeProtected(value: string): string {
  const bytes = new TextEncoder().encode(value);
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

interface XmlGroupNode {
  name: string;
  entries: XmlExportRow[];
  children: Map<string, XmlGroupNode>;
}

/** Build a KeePass 2.x XML document from flat rows (`A / B` group paths),
 *  mirroring the desktop `export_xml` payload so both runtimes produce the
 *  same format. Passwords are written `Protected="True"` + Base64. */
export function buildKeePassXml(rows: XmlExportRow[]): string {
  const root: XmlGroupNode = { name: "", entries: [], children: new Map() };
  for (const row of rows) {
    let node = root;
    if (row.group) {
      for (const part of row.group.split(" / ")) {
        let child = node.children.get(part);
        if (!child) {
          child = { name: part || "未命名", entries: [], children: new Map() };
          node.children.set(part, child);
        }
        node = child;
      }
    }
    node.entries.push(row);
  }

  let xml =
    '<?xml version="1.0" encoding="utf-8" standalone="yes"?>\n<KeePassFile><Meta><Generator>SecPivot</Generator></Meta><Root>';
  const writeField = (key: string, value: string, protect: boolean): void => {
    xml += `<String><Key>${escapeXml(key)}</Key><Value${
      protect ? ' Protected="True">' + encodeProtected(value) : ">" + escapeXml(value)
    }</Value></String>`;
  };
  const walk = (node: XmlGroupNode): void => {
    xml += `<Group><Name>${escapeXml(node.name)}</Name>`;
    for (const row of node.entries) {
      xml += "<Entry>";
      writeField("Title", row.title, false);
      writeField("UserName", row.username, false);
      writeField("Password", row.password, true);
      writeField("URL", row.url, false);
      writeField("Notes", row.notes, false);
      if (row.totp) writeField("otp", row.totp, false);
      xml += "</Entry>";
    }
    for (const child of node.children.values()) walk(child);
    xml += "</Group>";
  };
  walk(root);
  xml += "</Root></KeePassFile>\n";
  return xml;
}
