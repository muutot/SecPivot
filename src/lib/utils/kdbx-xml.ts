/** Parser for KeePass 2.x XML export files (`File ▸ Export ▸ KeePass XML`).
 *  Walks the `<Root><Group>` tree, mapping nested groups to `A / B` paths and
 *  each entry's `String` fields to the fixed columns (Title/UserName/Password/
 *  URL/Notes) plus the dedicated `otp` seed, preserving native OTP keys as
 *  custom fields so their type, counter and protection flag survive,
 *  `<Times>` expiry (`<Expires>` + `<ExpiryTime>`), and any remaining
 *  non-standard `String`s as custom fields. */
import type { Language } from "$lib/types/settings";
import { t } from "$lib/i18n";
import type { CustomField } from "$lib/types/vault";

export interface XmlImportEntry {
  group: string;
  title: string;
  username: string;
  password: string;
  url: string;
  notes: string;
  totp: string;
  /** Raw `<ExpiryTime>` text, kept only when `<Expires>` is true; empty = no expiry. */
  expires: string;
  customFields: CustomField[];
}

function childElements(el: Element): Element[] {
  return Array.from(el.children).filter((c) => c.nodeType === 1);
}

function textOf(el: Element | null | undefined): string {
  return el?.textContent?.trim() ?? "";
}

/** Older SecPivot exports incorrectly used unencrypted UTF-8 Base64 with
 *  `Protected`. Only decode that explicitly identified legacy format. */
function decodeLegacySecPivotValue(value: string, lang: Language): string {
  const b64 = value.trim();
  if (!b64) return "";
  try {
    const decoded = atob(b64);
    const bytes = new Uint8Array(decoded.length);
    for (let i = 0; i < decoded.length; i++) bytes[i] = decoded.charCodeAt(i);
    return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
  } catch {
    throw new Error(t(lang, "error.invalidXml"));
  }
}

function parseEntry(entry: Element, group: string, lang: Language): XmlImportEntry {
  const out: XmlImportEntry = {
    group,
    title: "",
    username: "",
    password: "",
    url: "",
    notes: "",
    totp: "",
    expires: "",
    customFields: [],
  };
  for (const child of childElements(entry)) {
    if (child.tagName === "Times") {
      out.expires = parseExpiryTime(child);
      continue;
    }
    if (child.tagName !== "String") continue;
    const key = textOf(childElements(child).find((e) => e.tagName === "Key"));
    if (!key) continue;
    const valueEl = childElements(child).find((e) => e.tagName === "Value");
    const protectedValue = valueEl?.getAttribute("Protected")?.toLowerCase() === "true";
    const protectInMemory = valueEl?.getAttribute("ProtectInMemory")?.toLowerCase() === "true";
    const raw = valueEl?.textContent ?? "";
    const value = protectedValue ? decodeLegacySecPivotValue(raw, lang) : raw;
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
        if (!out.totp) out.totp = value;
        break;
      default:
        out.customFields.push({ name: key, value, protected: protectedValue || protectInMemory });
    }
  }
  return out;
}

/** Read `<Times><Expires>/<ExpiryTime>` of one KeePass XML entry. Returns the
 *  raw expiry text only when expiry is enabled; otherwise empty (no expiry).
 *  The backend `parse_expiry` accepts both RFC 3339 and naive
 *  `%Y-%m-%dT%H:%M:%S` forms, so the text passes through untouched. */
function parseExpiryTime(times: Element): string {
  const kids = childElements(times);
  const expires = textOf(kids.find((e) => e.tagName === "Expires")).toLowerCase();
  if (expires !== "true") return "";
  return textOf(kids.find((e) => e.tagName === "ExpiryTime"));
}

/** Parse a KeePass 2.x XML import file into entries with `A / B` group paths
 *  relative to the import target. Throws when the document is not a KeePass
 *  XML (or the XML is malformed). */
export function parseKdbxXml(xmlText: string, lang: Language): XmlImportEntry[] {
  const doc = new DOMParser().parseFromString(xmlText, "text/xml");
  const parseError = doc.querySelector("parsererror");
  if (parseError) {
    throw new Error(t(lang, "error.invalidXml"));
  }
  const root = doc.querySelector("KeePassFile > Root > Group");
  if (!root) {
    throw new Error(t(lang, "error.notKeepassXml"));
  }
  const legacySecPivot = textOf(doc.querySelector("KeePassFile > Meta > Generator")) === "SecPivot";
  for (const value of doc.querySelectorAll("Value[Protected]")) {
    if (value.getAttribute("Protected")?.toLowerCase() !== "true") continue;
    // Default KDBX XML contains stream-encrypted Base64 and cannot be read
    // without the inner key. PlainXml instead uses literal text and
    // ProtectInMemory. Refuse a mixed marker even in a legacy SecPivot file.
    if (!legacySecPivot || value.getAttribute("ProtectInMemory")?.toLowerCase() === "true") {
      throw new Error(t(lang, "error.encryptedKeepassXml"));
    }
  }
  const entries: XmlImportEntry[] = [];
  const groupOf = (group: Element, parts: string[]) => {
    for (const child of childElements(group)) {
      if (child.tagName === "Entry") {
        entries.push(parseEntry(child, parts.join(" / "), lang));
      }
    }
    for (const sub of childElements(group)) {
      if (sub.tagName !== "Group") continue;
      const name = textOf(childElements(sub).find((e) => e.tagName === "Name"));
      groupOf(sub, [...parts, name || "Unnamed"]);
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
  expires?: string;
  customFields?: CustomField[];
}

function escapeXml(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#39;");
}

interface XmlGroupNode {
  name: string;
  entries: XmlExportRow[];
  children: Map<string, XmlGroupNode>;
}

/** Build a KeePass 2.x XML document from flat rows (`A / B` group paths),
 *  mirroring the desktop `export_xml` payload so both runtimes produce the
 *  same PlainXml format. Passwords are escaped plaintext with
 *  `ProtectInMemory="True"`, following the official KeePass XML writer. */
export function buildKeePassXml(rows: XmlExportRow[]): string {
  const root: XmlGroupNode = { name: "", entries: [], children: new Map() };
  for (const row of rows) {
    let node = root;
    if (row.group) {
      for (const part of row.group.split(" / ")) {
        let child = node.children.get(part);
        if (!child) {
          child = { name: part || "Unnamed", entries: [], children: new Map() };
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
      protect ? ' ProtectInMemory="True"' : ""
    }>${escapeXml(value)}</Value></String>`;
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
      for (const field of row.customFields ?? []) {
        if (["Title", "UserName", "Password", "URL", "Notes", "otp"].includes(field.name)) continue;
        writeField(field.name, field.value, field.protected === true);
      }
      xml += `<Times><Expires>${row.expires ? "True" : "False"}</Expires>`;
      if (row.expires) xml += `<ExpiryTime>${escapeXml(row.expires)}</ExpiryTime>`;
      xml += "</Times>";
      xml += "</Entry>";
    }
    for (const child of node.children.values()) walk(child);
    xml += "</Group>";
  };
  walk(root);
  xml += "</Root></KeePassFile>\n";
  return xml;
}
