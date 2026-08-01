import type { SecurityReport, VaultGroup } from "$lib/types/vault";

/** Mirror of the Rust `estimate_entropy` in vault.rs; keep both in sync. */
export function estimateEntropyBits(password: string): number {
  let pool = 0;
  if (/[A-Z]/.test(password)) pool += 26;
  if (/[a-z]/.test(password)) pool += 26;
  if (/[0-9]/.test(password)) pool += 10;
  if (/[^A-Za-z0-9]/.test(password)) pool += 32;
  if (pool === 0) return 0;
  return Math.round(password.length * Math.log2(pool));
}

/** Browser fallback: analyze the local state (passwords stay in-memory here). */
export function computeSecurityReport(root: VaultGroup): SecurityReport {
  const empty: string[] = [];
  const weak: { uuid: string; bits: number }[] = [];
  const byPassword = new Map<string, string[]>();
  let total = 0;

  function scan(group: VaultGroup): void {
    for (const entry of group.entries) {
      total += 1;
      const password = entry.password ?? "";
      if (!password) {
        empty.push(entry.uuid);
        continue;
      }
      const bits = estimateEntropyBits(password);
      if (bits < 72) weak.push({ uuid: entry.uuid, bits });
      const list = byPassword.get(password) ?? [];
      list.push(entry.uuid);
      byPassword.set(password, list);
    }
    for (const child of group.children) scan(child);
  }
  scan(root);

  const duplicates = [...byPassword.entries()]
    .filter(([, uuids]) => uuids.length > 1)
    .map(([, uuids]) => ({ count: uuids.length, uuids }))
    .sort((a, b) => b.count - a.count);

  return {
    total,
    empty,
    weak: weak.sort((a, b) => a.bits - b.bits),
    duplicates,
  };
}
