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
  const chars = new Array<string>(settings.length);
  for (let i = 0; i < settings.length; i++) {
    chars[i] = pool[arr[i] % pool.length];
  }

  // Guarantee at least one char per requested class (mirrors the Rust
  // `generate_password` in bridge/dispatch.rs): overwrite distinct random
  // positions with a char drawn from each missing category — never from the
  // whole pool, which could still miss the requested class.
  const wantUpper = settings.includeUpper;
  const wantLower = settings.includeLower;
  const wantDigits = settings.includeDigits;
  const candidates: { category: string; re: RegExp; enabled: boolean }[] = [
    { category: UPPER, re: /[A-Z]/, enabled: wantUpper },
    { category: LOWER, re: /[a-z]/, enabled: wantLower },
    { category: DIGITS, re: /[0-9]/, enabled: wantDigits },
  ];
  let fixIndex = 0;
  // Distinct random positions from a CSPRNG Fisher–Yates shuffle, so every
  // required class lands in its own slot (never `Math.random` or the old
  // deterministic `fixIndex % length`).
  const positions = [...Array(settings.length).keys()];
  const rnd = new Uint32Array(settings.length);
  crypto.getRandomValues(rnd);
  for (let i = positions.length - 1; i > 0; i--) {
    const j = rnd[i] % (i + 1);
    [positions[i], positions[j]] = [positions[j], positions[i]];
  }
  for (const { category, re, enabled } of candidates) {
    if (!enabled || re.test(chars.join(""))) continue;
    const categoryPool = [...category].filter((c) => {
      if (settings.excludeSimilar && SIMILAR.includes(c)) return false;
      if (settings.excludeAmbiguous && AMBIGUOUS.includes(c)) return false;
      return true;
    });
    if (categoryPool.length === 0) continue;
    const pos = positions[fixIndex % settings.length];
    fixIndex += 1;
    const pick = new Uint32Array(1);
    crypto.getRandomValues(pick);
    chars[pos] = categoryPool[pick[0] % categoryPool.length];
  }

  return chars.join("");
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
