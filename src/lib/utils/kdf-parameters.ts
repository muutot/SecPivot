import type { DatabaseKdf, KdfParameters } from "$lib/types/vault";

const U64_MAX = (1n << 64n) - 1n;
const MIB = 1048576n;

function unsigned(value: string, max = U64_MAX, allowZero = false): bigint | null {
  if (!/^\d{1,20}$/.test(value)) return null;
  const parsed = BigInt(value);
  return (allowZero || parsed > 0n) && parsed <= max ? parsed : null;
}

/** A power-of-two denominator has an exact terminating decimal expansion. */
export function memoryMiB(bytes: string): string {
  const decimal = (BigInt(bytes) * 5n ** 20n).toString().padStart(21, "0");
  return `${decimal.slice(0, -20)}.${decimal.slice(-20)}`.replace(/\.?0+$/, "");
}

export function memoryBytes(mib: string): string | null {
  if (!/^\d{1,20}(\.\d{1,20})?$/.test(mib)) return null;
  const [whole, fraction = ""] = mib.split(".");
  const numerator = BigInt(whole + fraction) * MIB;
  const denominator = 10n ** BigInt(fraction.length);
  if (numerator % denominator !== 0n) return null;
  const bytes = numerator / denominator;
  return bytes > 0n && bytes <= U64_MAX ? bytes.toString() : null;
}

export function defaultKdfParameters(
  algorithm: DatabaseKdf,
  original: KdfParameters,
): KdfParameters {
  if (original.algorithm === algorithm) return { ...original };
  if (algorithm === "Aes") return { algorithm, rounds: "600000" };
  if (original.algorithm !== "Aes") return { ...original, algorithm };
  return { algorithm, iterations: "3", memoryBytes: "67108864", parallelism: 4, version: 19 };
}

export function readKdfParameters(
  algorithm: DatabaseKdf,
  rounds: string,
  iterations: string,
  mib: string,
  lanes: string,
  version: number,
): KdfParameters | null {
  if (algorithm === "Aes") {
    const cost = unsigned(rounds, U64_MAX, true);
    return cost === null ? null : { algorithm, rounds: cost.toString() };
  }
  const cost = unsigned(iterations, 0xffffffffn);
  const parallelism = unsigned(lanes, 0xffffffn);
  const memory = memoryBytes(mib);
  if (cost === null || parallelism === null || memory === null || ![16, 19].includes(version))
    return null;
  const kib = BigInt(memory) / 1024n;
  if (kib < parallelism * 8n || kib > 0xffffffffn) return null;
  return {
    algorithm,
    iterations: cost.toString(),
    memoryBytes: memory,
    parallelism: Number(parallelism),
    version,
  };
}
