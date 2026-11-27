import type {
  CustomField,
  LinkedFieldSet,
  LinkedFieldTarget,
  LinkedFieldsChange,
} from "$lib/types/vault";

const reserved = new Set([
  "Title",
  "UserName",
  "Password",
  "URL",
  "Notes",
  "otp",
  "SecPivot.Favorite",
  "SecPivot.OriginalGroup",
  "SecPivot.WebsiteRules",
  "SecPivot.LinkedFields",
]);
const invalid = () => new Error("Invalid linked fields");

/** Field names only; clone every nested target so templates/drafts cannot share state. */
export function copyLinkedFields(value: LinkedFieldSet): LinkedFieldSet {
  if (!value || value.version !== 1 || !Array.isArray(value.links) || value.links.length > 4096)
    throw invalid();
  const validName = (name: unknown): name is string =>
    typeof name === "string" &&
    !!name &&
    name === name.trim() &&
    !/^[\u0009-\u000d\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]|[\u0009-\u000d\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]$/u.test(
      name,
    ) &&
    new TextEncoder().encode(name).length <= 1024 &&
    [...name].every((c) => {
      const n = c.codePointAt(0)!;
      return (
        n === 9 ||
        n === 10 ||
        n === 13 ||
        (n >= 0x20 && n <= 0xd7ff) ||
        (n >= 0xe000 && n <= 0xfffd) ||
        (n >= 0x10000 && n <= 0x10ffff)
      );
    });
  const aliases = new Set<string>();
  for (const link of value.links) {
    if (!link || !validName(link.name) || reserved.has(link.name) || aliases.has(link.name))
      throw invalid();
    aliases.add(link.name);
  }
  const links = value.links.map((link) => {
    const target = link.target;
    let names: string[];
    if (target?.kind === "field") names = [target.name];
    else if (
      target?.kind === "bitwardenFullName" &&
      Array.isArray(target.fields) &&
      target.fields.length === 4
    )
      names = target.fields;
    else throw invalid();
    if (target.kind === "bitwardenFullName" && new Set(names).size !== 4) throw invalid();
    for (const name of names)
      if (
        !validName(name) ||
        aliases.has(name) ||
        (reserved.has(name) &&
          (target.kind !== "field" || !["UserName", "Password"].includes(name)))
      )
        throw invalid();
    return {
      name: link.name,
      target:
        target.kind === "field"
          ? { kind: "field" as const, name: target.name }
          : {
              kind: "bitwardenFullName" as const,
              fields: [...target.fields] as [string, string, string, string],
            },
    };
  });
  const copy: LinkedFieldSet = { version: 1, links };
  if (new TextEncoder().encode(JSON.stringify(copy)).length > 65536) throw invalid();
  return copy;
}

/** Resolve only local literal values. Never interpret KeePass placeholders. */
export function linkedFieldValue(
  target: LinkedFieldTarget,
  read: (name: string) => string,
): string {
  return target.kind === "field"
    ? read(target.name)
    : target.fields
        .map(read)
        .filter((part) => part.trim() !== "")
        .join(" ")
        .trim();
}

export class LinkedFieldsDraft {
  private fields: LinkedFieldSet | null;
  private touched = false;
  constructor(initial?: LinkedFieldSet | null) {
    this.fields = initial ? copyLinkedFields(initial) : null;
  }
  get value(): LinkedFieldSet | null {
    return this.fields ? structuredClone(this.fields) : null;
  }
  target(name: string): LinkedFieldTarget | null {
    return this.value?.links.find((link) => link.name === name)?.target ?? null;
  }
  replace(value?: LinkedFieldSet | null): void {
    this.fields = value ? copyLinkedFields(value) : null;
    this.touched = true;
  }
  unlink(name: string): void {
    if (!this.fields) return;
    name = name.trim();
    this.fields.links = this.fields.links.filter((link) => link.name !== name);
    this.touched = true;
  }
  edit(before: CustomField, patch: Partial<CustomField>): void {
    before = { ...before, name: before.name.trim() };
    if ("value" in patch || patch.protected === false) this.unlink(before.name);
    if (patch.name !== undefined && this.fields) {
      const next = patch.name.trim();
      if (!next) {
        this.unlink(before.name);
        return;
      }
      for (const link of this.fields.links) {
        if (link.name === before.name) link.name = next;
        if (link.target.kind === "field") {
          if (link.target.name === before.name) link.target.name = next;
        } else
          link.target.fields = link.target.fields.map((name) =>
            name === before.name ? next : name,
          ) as [string, string, string, string];
      }
      this.touched = true;
    }
  }
  change(): LinkedFieldsChange | null {
    return this.fields?.links.length
      ? { mode: "set", fields: copyLinkedFields(this.fields) }
      : this.touched
        ? { mode: "clear" }
        : null;
  }
}
