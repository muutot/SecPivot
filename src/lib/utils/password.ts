import type { PasswordGeneratorSettings } from "$lib/types/settings";

const UPPER = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER = "abcdefghijklmnopqrstuvwxyz";
const DIGITS = "0123456789";
const SYMBOLS = "!@#$%^&*()-_=+[]{};:,.<>?";
const SIMILAR = "Il1O0";
const AMBIGUOUS = "{}[]()/\\'\"`~,;:.<>";

export function generatePassword(settings: PasswordGeneratorSettings): string {
  const pool = buildPool(settings);
  const required = uniqueChars(settings.requiredChars ?? "");
  for (const char of required) {
    if (!pool.includes(char)) {
      throw new Error(`必含字符 ${char} 不在字符池中`);
    }
  }

  if (settings.pattern) {
    const chars = new Array<string>(settings.pattern.length);
    for (let i = 0; i < settings.pattern.length; i++) {
      const code = settings.pattern[i];
      if (code === "u" || code === "l" || code === "d" || code === "s") {
        const category =
          code === "u" ? UPPER : code === "l" ? LOWER : code === "d" ? DIGITS : SYMBOLS;
        const categoryChars = categoryPool(category, settings);
        chars[i] = categoryChars.length > 0 ? randomPick(categoryChars) : randomPick(pool);
      } else if (code === "a") {
        chars[i] = randomPick(pool);
      } else {
        chars[i] = code;
      }
    }
    guaranteeRequired(chars, required);
    return chars.join("");
  }

  const arr = new Uint32Array(settings.length);
  crypto.getRandomValues(arr);
  const chars = new Array<string>(settings.length);
  for (let i = 0; i < settings.length; i++) {
    chars[i] = pool[arr[i] % pool.length];
  }

  // Guarantee at least one char per requested class (mirrors the Rust
  // `generate_password` in bridge/dispatch.rs): overwrite distinct random
  // positions with a char drawn from each missing category — never from the
  // whole pool, which could still miss the requested class.
  const candidates: { category: string; re: RegExp; enabled: boolean }[] = [
    { category: UPPER, re: /[A-Z]/, enabled: settings.includeUpper },
    { category: LOWER, re: /[a-z]/, enabled: settings.includeLower },
    { category: DIGITS, re: /[0-9]/, enabled: settings.includeDigits },
  ];
  // A custom charset replaces the built-in classes entirely, so class
  // guarantees do not apply; `requiredChars` still enforces inclusions.
  if (!settings.customCharset) {
    const positions = shuffledPositions(settings.length);
    for (const { category, re, enabled } of candidates) {
      if (!enabled || re.test(chars.join(""))) continue;
      const categoryChars = categoryPool(category, settings);
      if (categoryChars.length === 0) continue;
      const pos = positions.shift() ?? 0;
      chars[pos] = randomPick(categoryChars);
    }
  }
  guaranteeRequired(chars, required);

  return chars.join("");
}

function buildPool(settings: PasswordGeneratorSettings): string {
  let pool = settings.customCharset ?? "";
  if (pool.length === 0) {
    if (settings.includeUpper) pool += UPPER;
    if (settings.includeLower) pool += LOWER;
    if (settings.includeDigits) pool += DIGITS;
    if (settings.includeSymbols) pool += SYMBOLS;
  }
  if (settings.excludeSimilar) {
    pool = [...pool].filter((c) => !SIMILAR.includes(c)).join("");
  }
  if (settings.excludeAmbiguous) {
    pool = [...pool].filter((c) => !AMBIGUOUS.includes(c)).join("");
  }
  if (settings.excludeChars) {
    const excluded = uniqueChars(settings.excludeChars);
    pool = [...pool].filter((c) => !excluded.includes(c)).join("");
  }
  if (pool.length === 0) pool = UPPER + LOWER + DIGITS;
  return pool;
}

function categoryPool(category: string, settings: PasswordGeneratorSettings): string {
  let pool = category;
  if (settings.excludeSimilar) {
    pool = [...pool].filter((c) => !SIMILAR.includes(c)).join("");
  }
  if (settings.excludeAmbiguous) {
    pool = [...pool].filter((c) => !AMBIGUOUS.includes(c)).join("");
  }
  if (settings.excludeChars) {
    const excluded = uniqueChars(settings.excludeChars);
    pool = [...pool].filter((c) => !excluded.includes(c)).join("");
  }
  return pool;
}

function uniqueChars(value: string): string[] {
  return [...value].filter((c, index, all) => all.indexOf(c) === index);
}

function shuffledPositions(length: number): number[] {
  const positions = [...Array(length).keys()];
  const rnd = new Uint32Array(length);
  crypto.getRandomValues(rnd);
  for (let i = positions.length - 1; i > 0; i--) {
    const j = rnd[i] % (i + 1);
    [positions[i], positions[j]] = [positions[j], positions[i]];
  }
  return positions;
}

function randomPick(pool: string): string {
  const pick = new Uint32Array(1);
  crypto.getRandomValues(pick);
  return pool[pick[0] % pool.length];
}

function guaranteeRequired(chars: string[], required: string[]): void {
  // Slots that already hold a required char must never be overwritten.
  const used = new Set<number>();
  chars.forEach((c, i) => {
    if (required.includes(c)) used.add(i);
  });
  let candidates = [...Array(chars.length).keys()].filter((i) => !used.has(i));
  if (candidates.length === 0) candidates = [0];
  const order = shuffledPositions(candidates.length);
  let index = 0;
  for (const char of required) {
    if (chars.includes(char)) continue;
    chars[candidates[order[index++ % candidates.length]]] = char;
  }
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

export function entropyLabel(bits: number): {
  label: string;
  className: "weak" | "fair" | "strong";
} {
  if (bits < 40) return { label: "弱", className: "weak" };
  if (bits < 72) return { label: "中等", className: "fair" };
  return { label: "强", className: "strong" };
}
