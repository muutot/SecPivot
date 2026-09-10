import type { Language, PasswordGeneratorSettings } from "$lib/types/settings";
import { t } from "$lib/i18n";

const UPPER = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER = "abcdefghijklmnopqrstuvwxyz";
const DIGITS = "0123456789";
const SYMBOLS = "!@#$%^&*()-_=+[]{};:,.<>?";
const SIMILAR = "Il1O0";
const AMBIGUOUS = "{}[]()/\\'\"`~,;:.<>";

export function generatePassword(settings: PasswordGeneratorSettings, lang: Language): string {
  const required = uniqueChars(settings.requiredChars ?? "");

  if (settings.pattern) {
    return generatePatternPassword(settings.pattern, settings, required, lang);
  }

  if (!Number.isInteger(settings.length) || settings.length <= 0) {
    throw new Error(t(lang, "pw.lengthPositive"));
  }

  const pool = buildPool(settings, lang);
  for (const char of required) {
    if (!pool.includes(char)) {
      throw new Error(t(lang, "pw.charNotInPool", { char }));
    }
  }

  const mandatory = [...required];
  const categories: { label: string; source: string; enabled: boolean }[] = [
    { label: t(lang, "pw.upper"), source: UPPER, enabled: settings.includeUpper },
    { label: t(lang, "pw.lower"), source: LOWER, enabled: settings.includeLower },
    { label: t(lang, "pw.digits"), source: DIGITS, enabled: settings.includeDigits },
    { label: t(lang, "pw.symbols"), source: SYMBOLS, enabled: settings.includeSymbols },
  ];
  if (!usesCustomCharset(settings)) {
    for (const { label, source, enabled } of categories) {
      if (!enabled) continue;
      const category = categoryPool(source, settings);
      if (category.length === 0) {
        throw new Error(t(lang, "pw.poolEmpty", { label }));
      }
      if (!mandatory.some((char) => category.includes(char))) {
        mandatory.push(randomPick(category, lang));
      }
    }
  }

  if (mandatory.length > settings.length) {
    throw new Error(t(lang, "pw.tooShort", { len: settings.length, n: mandatory.length }));
  }

  const chars = [...mandatory];
  while (chars.length < settings.length) chars.push(randomPick(pool, lang));
  shuffle(chars, lang);
  return chars.join("");
}

type PatternSlot = { literal: string } | { pool: string[] };

function generatePatternPassword(
  pattern: string,
  settings: PasswordGeneratorSettings,
  required: string[],
  lang: Language,
): string {
  let generalPool: string[] | null = null;
  const slots: PatternSlot[] = [...pattern].map((code) => {
    if (code === "u" || code === "l" || code === "d" || code === "s") {
      const source = code === "u" ? UPPER : code === "l" ? LOWER : code === "d" ? DIGITS : SYMBOLS;
      const pool = categoryPool(source, settings);
      if (pool.length === 0) throw new Error(t(lang, "pw.patternPoolEmpty", { code }));
      return { pool };
    }
    if (code === "a") {
      generalPool ??= buildPool(settings, lang);
      return { pool: generalPool };
    }
    return { literal: code };
  });

  const literals = new Set(slots.flatMap((slot) => ("literal" in slot ? [slot.literal] : [])));
  const missing = required.filter((char) => !literals.has(char));
  const positionAssignments = new Map<number, number>();

  const assign = (requiredIndex: number, seen: Set<number>): boolean => {
    const char = missing[requiredIndex];
    for (let position = 0; position < slots.length; position++) {
      const slot = slots[position];
      if (!("pool" in slot) || !slot.pool.includes(char) || seen.has(position)) continue;
      seen.add(position);
      const previous = positionAssignments.get(position);
      if (previous === undefined || assign(previous, seen)) {
        positionAssignments.set(position, requiredIndex);
        return true;
      }
    }
    return false;
  };

  for (let index = 0; index < missing.length; index++) {
    if (!assign(index, new Set())) {
      throw new Error(t(lang, "pw.patternNoFit", { char: missing[index] }));
    }
  }

  return slots
    .map((slot, position) => {
      if ("literal" in slot) return slot.literal;
      const requiredIndex = positionAssignments.get(position);
      return requiredIndex === undefined ? randomPick(slot.pool, lang) : missing[requiredIndex];
    })
    .join("");
}

function usesCustomCharset(settings: PasswordGeneratorSettings): boolean {
  return [...(settings.customCharset ?? "")].length > 0;
}

function buildPool(settings: PasswordGeneratorSettings, lang: Language): string[] {
  let pool: string[] = usesCustomCharset(settings) ? [...(settings.customCharset ?? "")] : [];
  if (pool.length === 0) {
    if (settings.includeUpper) pool.push(...UPPER);
    if (settings.includeLower) pool.push(...LOWER);
    if (settings.includeDigits) pool.push(...DIGITS);
    if (settings.includeSymbols) pool.push(...SYMBOLS);
  }
  if (settings.excludeSimilar) {
    pool = pool.filter((char) => !SIMILAR.includes(char));
  }
  if (settings.excludeAmbiguous) {
    pool = pool.filter((char) => !AMBIGUOUS.includes(char));
  }
  if (settings.excludeChars) {
    const excluded = new Set(uniqueChars(settings.excludeChars));
    pool = pool.filter((char) => !excluded.has(char));
  }
  pool = uniqueChars(pool.join(""));
  if (pool.length === 0) throw new Error(t(lang, "pw.genPoolEmpty"));
  return pool;
}

function categoryPool(category: string, settings: PasswordGeneratorSettings): string[] {
  let pool = [...category];
  if (settings.excludeSimilar) {
    pool = pool.filter((char) => !SIMILAR.includes(char));
  }
  if (settings.excludeAmbiguous) {
    pool = pool.filter((char) => !AMBIGUOUS.includes(char));
  }
  if (settings.excludeChars) {
    const excluded = new Set(uniqueChars(settings.excludeChars));
    pool = pool.filter((char) => !excluded.has(char));
  }
  return pool;
}

function uniqueChars(value: string): string[] {
  return [...value].filter((c, index, all) => all.indexOf(c) === index);
}

function shuffle<T>(values: T[], lang: Language): void {
  for (let i = values.length - 1; i > 0; i--) {
    const j = randomIndex(i + 1, lang);
    [values[i], values[j]] = [values[j], values[i]];
  }
}

function randomIndex(bound: number, lang: Language): number {
  if (!Number.isSafeInteger(bound) || bound <= 0 || bound > 0x1_0000_0000) {
    throw new Error(t(lang, "pw.badBound"));
  }
  const range = 0x1_0000_0000;
  const limit = range - (range % bound);
  const sample = new Uint32Array(1);
  do {
    crypto.getRandomValues(sample);
  } while (sample[0] >= limit);
  return sample[0] % bound;
}

function randomPick(pool: string[], lang: Language): string {
  return pool[randomIndex(pool.length, lang)];
}

/** Entropy estimate in bits. Mirror of the Rust `estimate_entropy` in
 *  `src-tauri/src/vault.rs`; keep both in sync. */
export function estimateEntropy(password: string): number {
  let pool = 0;
  if (/[A-Z]/.test(password)) pool += 26;
  if (/[a-z]/.test(password)) pool += 26;
  if (/[0-9]/.test(password)) pool += 10;
  if (/[^A-Za-z0-9]/.test(password)) pool += 32;
  if (pool === 0) return 0;
  return Math.round(password.length * Math.log2(pool));
}

export type StrengthLevel = "weak" | "fair" | "strong";

/** Strength bucket for an entropy estimate. Returns the stable `level`
 *  (callers translate it); the human label lives in the i18n dictionaries
 *  (`password.strength.*`) so it follows the UI locale. */
export function entropyLabel(bits: number): {
  level: StrengthLevel;
  className: StrengthLevel;
} {
  if (bits < 40) return { level: "weak", className: "weak" };
  if (bits < 72) return { level: "fair", className: "fair" };
  return { level: "strong", className: "strong" };
}
