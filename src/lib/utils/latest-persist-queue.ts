/**
 * Serializes persistence while retaining only the newest queued value.
 * A completed older write is applied only when no newer value arrived while
 * it was in flight, so stale acknowledgements cannot roll back local state.
 */
export class LatestPersistQueue<T, Saved = T> {
  private pending: T | undefined;
  private running: Promise<void> | null = null;

  enqueue(value: T): void {
    this.pending = value;
  }

  get hasPending(): boolean {
    return this.pending !== undefined;
  }

  drain(
    write: (value: T) => Promise<Saved>,
    apply: (saved: Saved, submitted: T) => void,
  ): Promise<void> {
    if (this.running) return this.running;
    this.running = this.persist(write, apply).finally(() => {
      this.running = null;
    });
    return this.running;
  }

  private async persist(
    write: (value: T) => Promise<Saved>,
    apply: (saved: Saved, submitted: T) => void,
  ): Promise<void> {
    while (this.pending !== undefined) {
      const value = this.pending;
      this.pending = undefined;

      let saved: Saved;
      try {
        saved = await write(value);
      } catch {
        if (this.pending === undefined) {
          // Retain a failed latest value for an explicit flush or later edit.
          this.pending = value;
          return;
        }
        // A newer value supersedes the failed write; keep draining it now.
        continue;
      }

      if (this.pending === undefined) {
        apply(saved, value);
      }
    }
  }
}
