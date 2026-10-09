// Settings-contract drift guard: every TypeScript settings key must exist in
// the Rust config structs and vice versa. The Rust side uses
// `#[serde(default)]` and drops unknown fields silently, so a key added on
// only one side works in-session but resets on restart. Run via
// `npm run check:contract` (part of `npm run verify`).
import { readFileSync } from "node:fs";

const TS_FILES = [
  "src/lib/types/settings.ts",
  "src/lib/types/theme.ts",
  "src/lib/utils/entry-search.ts",
];
// Rust-only legacy alias: pre-`keyboard`-section configs migrate it on load
// (`settings.rs`) and it is never re-serialized. Not a drift.
const RUST_LEGACY_ALLOWLIST = new Set(["globalAutoTypeShortcut"]);

function snakeToCamel(s) {
  return s.replace(/_([a-z])/g, (_, c) => c.toUpperCase());
}

function tsKeys(path) {
  const src = readFileSync(path, "utf8");
  const keys = new Set();
  for (const line of src.split("\n")) {
    const m = line.match(/^\s+([A-Za-z_]\w*)\??:/);
    if (!m || m[1] === "default" || line.trimStart().startsWith("case ")) continue;
    keys.add(m[1]);
  }
  return keys;
}

function rustKeys(path) {
  const src = readFileSync(path, "utf8");
  const keys = new Set();
  for (const m of src.matchAll(/^\s*pub (\w+):/gm)) keys.add(snakeToCamel(m[1]));
  return keys;
}

const ts = new Set(TS_FILES.flatMap((f) => [...tsKeys(f)]));
const rs = rustKeys("src-tauri/src/config/settings.rs");

const tsOnly = [...ts].filter((k) => !rs.has(k)).sort();
const rsOnly = [...rs].filter((k) => !ts.has(k) && !RUST_LEGACY_ALLOWLIST.has(k)).sort();

if (tsOnly.length === 0 && rsOnly.length === 0) {
  console.log(`settings contract OK (${ts.size} TS keys ⊆ Rust, legacy allowlist holds)`);
  process.exit(0);
}
if (tsOnly.length > 0) console.error(`TS-only keys missing from Rust config: ${tsOnly.join(", ")}`);
if (rsOnly.length > 0) console.error(`Rust-only keys missing from TS types: ${rsOnly.join(", ")}`);
console.error(
  "settings contract DRIFT: add the key on both sides (or document a legacy allowlist entry)",
);
process.exit(1);
