import type { TotpCode } from "$lib/types/vault";

/**
 * RFC 6238 TOTP computation for the browser demo fallback. Desktop builds
 * compute codes in the Rust backend via `keepass::db::TOTP`; this port keeps
 * the UI-dev surface (`npm run dev` outside Tauri) showing real codes. It is
 * not evidence of desktop TOTP behavior.
 */

const B32_ALPHABET = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

function decodeBase32(input: string): Uint8Array<ArrayBuffer> {
  const clean = input.replace(/=+$/g, "").replace(/[\s-]/g, "").toUpperCase();
  let bits = 0;
  let value = 0;
  const bytes: number[] = [];
  for (const char of clean) {
    const index = B32_ALPHABET.indexOf(char);
    if (index < 0) throw new Error("invalid base32 character");
    value = (value << 5) | index;
    bits += 5;
    if (bits >= 8) {
      bytes.push((value >>> (bits - 8)) & 0xff);
      bits -= 8;
    }
  }
  const buffer = new ArrayBuffer(bytes.length);
  const result = new Uint8Array(buffer);
  result.set(bytes);
  return result;
}

function parseSeed(seed: string): {
  secret: Uint8Array<ArrayBuffer>;
  period: number;
  digits: number;
  hash: "SHA-1" | "SHA-256" | "SHA-512";
} {
  const trimmed = seed.trim();
  if (trimmed.toLowerCase().startsWith("otpauth://")) {
    const url = new URL(trimmed);
    const secret = url.searchParams.get("secret") ?? "";
    const period = Number(url.searchParams.get("period") ?? 30) || 30;
    const digits = Number(url.searchParams.get("digits") ?? 6) || 6;
    const algorithm = (url.searchParams.get("algorithm") ?? "SHA1").toUpperCase();
    const hash = algorithm === "SHA256" ? "SHA-256" : algorithm === "SHA512" ? "SHA-512" : "SHA-1";
    return { secret: decodeBase32(secret), period, digits, hash };
  }
  return { secret: decodeBase32(trimmed), period: 30, digits: 6, hash: "SHA-1" };
}

async function hmacSha(
  data: Uint8Array<ArrayBuffer>,
  key: Uint8Array<ArrayBuffer>,
  hash: string,
): Promise<Uint8Array<ArrayBuffer>> {
  const cryptoKey = await crypto.subtle.importKey("raw", key, { name: "HMAC", hash }, false, [
    "sign",
  ]);
  const signature = await crypto.subtle.sign("HMAC", cryptoKey, data);
  return new Uint8Array(signature);
}

/** Compute the TOTP code at `now` (defaults to the current time). */
export async function computeTotp(seed: string, now = Date.now()): Promise<TotpCode> {
  const { secret, period, digits, hash } = parseSeed(seed);
  const unix = Math.floor(now / 1000);
  const counter = Math.floor(unix / period);
  const counterBytes = new Uint8Array(8);
  let value = counter;
  for (let i = 7; i >= 0; i--) {
    counterBytes[i] = value & 0xff;
    value = Math.floor(value / 256);
  }
  const mac = await hmacSha(counterBytes, secret, hash);
  const offset = mac[mac.length - 1] & 0x0f;
  const binary =
    ((mac[offset] & 0x7f) << 24) |
    ((mac[offset + 1] & 0xff) << 16) |
    ((mac[offset + 2] & 0xff) << 8) |
    (mac[offset + 3] & 0xff);
  const code = String(binary % 10 ** digits).padStart(digits, "0");
  const validFor = period - (unix % period);
  return { code, kind: "totp", validFor, period };
}
