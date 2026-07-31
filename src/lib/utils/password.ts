import type { PasswordGeneratorSettings } from "$lib/types/settings";

const UPPER = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER = "abcdefghijklmnopqrstuvwxyz";
const DIGITS = "0123456789";
const SYMBOLS = "!@#$%^&*()-_=+[]{};:,.<>?";
const SIMILAR = "Il1O0";
const AMBIGUOUS = "{}[]()/\\'\"`~,;:.<>";

export function generatePassword(settings: PasswordGeneratorSettings): string {
  let pool = "";
  if (settings.includeUpper) pool += UPPER;
  if (settings.includeLower) pool += LOWER;
  if (settings.includeDigits) pool += DIGITS;
  if (settings.includeSymbols) pool += SYMBOLS;

  if (settings.excludeSimilar) {
    pool = [...pool].filter((c) => !SIMILAR.includes(c)).join("");
  }
  if (settings.excludeAmbiguous) {
    pool = [...pool].filter((c) => !AMBIGUOUS.includes(c)).join("");
  }
  if (pool.length === 0) pool = UPPER + LOWER + DIGITS;

  const arr = new Uint32Array(settings.length);
  crypto.getRandomValues(arr);
  let result = "";
  for (let i = 0; i < settings.length; i++) {
    result += pool[arr[i] % pool.length];
  }

  if (settings.includeUpper && !/[A-Z]/.test(result)) result = result.slice(1) + pool[0];
  if (settings.includeLower && !/[a-z]/.test(result))
    result = result.slice(1) + pool[Math.floor(Math.random() * pool.length)];
  if (settings.includeDigits && !/[0-9]/.test(result))
    result = result.slice(1) + pool[Math.floor(Math.random() * pool.length)];

  return result;
}

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
