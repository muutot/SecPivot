import type { FileReloadResult } from "$lib/types/vault";

const blockers = new Set<() => boolean>();
/** Observe only activity flags; draft values never enter this registry. */
export function registerFileReloadBlocker(blocked: () => boolean): () => void {
  blockers.add(blocked);
  return () => {
    blockers.delete(blocked);
  };
}
export function isFileReloadBlocked(): boolean {
  return [...blockers].some((blocked) => blocked());
}

export interface ReloadOwner {
  sessionId: string;
  epoch: symbol;
}
interface Host {
  enabled: () => boolean;
  canApply: () => boolean;
  prepare: (sessionId: string) => Promise<FileReloadResult>;
  apply: (sessionId: string, token: string) => Promise<FileReloadResult>;
  result: (status: "reloaded" | "merged") => void;
  error: (error: unknown) => void;
}
function sameOwner(a: ReloadOwner | null, b: ReloadOwner | null): boolean {
  return (
    a === b || (a !== null && b !== null && a.sessionId === b.sessionId && a.epoch === b.epoch)
  );
}

/** One background check per controller; view/draft changes never admit a candidate. */
export class FileReloadController {
  #host: Host;
  #owner: ReloadOwner | null = null;
  #generation = 0;
  #busy = false;
  #disposed = false;
  #lastError = "";
  constructor(host: Host) {
    this.#host = host;
  }
  activate(owner: ReloadOwner | null): boolean {
    if (sameOwner(this.#owner, owner)) return false;
    this.#owner = owner;
    this.#generation++;
    this.#lastError = "";
    return true;
  }
  async check(): Promise<void> {
    const owner = this.#owner;
    if (this.#disposed || this.#busy || !owner || !this.#host.enabled() || !this.#host.canApply())
      return;
    const generation = this.#generation;
    const current = () =>
      !this.#disposed &&
      generation === this.#generation &&
      sameOwner(this.#owner, owner) &&
      this.#host.enabled() &&
      this.#host.canApply();
    this.#busy = true;
    try {
      const offer = await this.#host.prepare(owner.sessionId);
      if (!current()) return;
      if (offer.status !== "ready" || !offer.token) {
        this.#lastError = "";
        return;
      }
      const result = await this.#host.apply(owner.sessionId, offer.token);
      // A successful apply renews the session epoch synchronously in the service.
      // Its status is nonsecret; replacement activation must not suppress success.
      if (
        !this.#disposed &&
        this.#owner?.sessionId === owner.sessionId &&
        (result.status === "reloaded" || result.status === "merged")
      ) {
        this.#lastError = "";
        this.#host.result(result.status);
      }
    } catch (error) {
      if (current() && String(error) !== this.#lastError) {
        this.#lastError = String(error);
        this.#host.error(error);
      }
    } finally {
      this.#busy = false;
    }
  }
  dispose(): void {
    this.#disposed = true;
    this.#generation++;
    this.#owner = null;
  }
}
