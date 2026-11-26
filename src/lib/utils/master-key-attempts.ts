/** KeePass's per-open-prompt retry budget; this is not a persistent vault lockout. */
export function normalizeMasterKeyTries(value: unknown): number {
  if (typeof value !== "number" || !Number.isFinite(value)) return 3;
  return Math.min(2_147_483_647, Math.max(1, Math.round(value)));
}

export interface MasterKeyAttempt {
  readonly run: symbol;
  readonly ordinal: number;
}

export class MasterKeyAttempts {
  #run: symbol | null = null;
  #pending: MasterKeyAttempt | null = null;
  #limit = 3;
  #spent = 0;

  start(limit: number): void {
    this.#run = Symbol();
    this.#pending = null;
    this.#spent = 0;
    this.#limit = normalizeMasterKeyTries(limit);
  }

  reserve(): MasterKeyAttempt | null {
    if (!this.#run || this.#pending || this.#spent >= this.#limit) return null;
    const attempt = { run: this.#run, ordinal: ++this.#spent };
    this.#pending = attempt;
    return attempt;
  }

  isPending(attempt: MasterKeyAttempt): boolean {
    return this.#run === attempt.run && this.#pending === attempt;
  }

  finish(
    attempt: MasterKeyAttempt,
    submitted = true,
  ): { exhausted: boolean; limit: number } | null {
    if (!this.isPending(attempt)) return null;
    this.#pending = null;
    if (!submitted) this.#spent--;
    return { exhausted: this.#spent >= this.#limit, limit: this.#limit };
  }

  close(): void {
    this.#run = null;
    this.#pending = null;
  }
}
