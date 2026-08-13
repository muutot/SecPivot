/**
 * Store a vault snapshot only when it is at least as new as the cached
 * revision. Late IPC responses from the same session must never roll the
 * renderer back after a newer mutation has already completed.
 */
export function commitNewestSessionState<T extends { revision: number }>(
  states: Map<string, T>,
  sessionId: string,
  incoming: T,
): T {
  const current = states.get(sessionId);
  if (current && incoming.revision < current.revision) return current;
  states.set(sessionId, incoming);
  return incoming;
}

/** Keep the current renderer id only while it still exists in the backend
 * session list; otherwise follow the backend-active first item. */
export function resolveListedActiveId<T extends { sessionId: string }>(
  currentId: string | null,
  sessions: T[],
): string | null {
  return currentId && sessions.some((session) => session.sessionId === currentId)
    ? currentId
    : (sessions[0]?.sessionId ?? null);
}

export interface SessionViewToken {
  sessionId: string;
  epoch: number;
}

/** Identify one continuous visible-tab lifetime. Returning A after A→B is a
 * new epoch, so callbacks captured before the switch cannot reopen dialogs or
 * clear activity started after returning to A. */
export class SessionViewGuard {
  #sessionId: string | null = null;
  #epoch = 0;

  activate(sessionId: string | null): void {
    if (sessionId === this.#sessionId) return;
    this.#sessionId = sessionId;
    this.#epoch += 1;
  }

  capture(): SessionViewToken | null {
    return this.#sessionId ? { sessionId: this.#sessionId, epoch: this.#epoch } : null;
  }

  isCurrent(token: SessionViewToken): boolean {
    return token.sessionId === this.#sessionId && token.epoch === this.#epoch;
  }
}

/** Only the latest operation may clear a shared activity flag. */
export class LatestOperationGuard {
  #generation = 0;

  begin(): number {
    this.#generation += 1;
    return this.#generation;
  }

  invalidate(): void {
    this.#generation += 1;
  }

  isCurrent(generation: number): boolean {
    return generation === this.#generation;
  }
}

/** A secret read may affect reveal state only when it produced a value for the
 * still-visible session and entry. Empty strings are valid secret values;
 * `null` means missing, still loading, or invalidated. */
export function canToggleSecretReveal(
  value: string | null,
  requestedSessionId: string,
  currentSessionId: string | null,
  requestedUuid: string,
  currentUuid: string,
): value is string {
  return value !== null && requestedSessionId === currentSessionId && requestedUuid === currentUuid;
}

/**
 * Serialize complete backend-active tab switch attempts in request order.
 * Enqueue is synchronous, so snapshot validation for an uncached tab and its
 * subsequent backend swap cannot be overtaken by a later click.
 */
export class SessionSwitchQueue {
  #tail: Promise<void> = Promise.resolve();

  enqueue<T>(operation: () => Promise<T>): Promise<T> {
    let result: T;
    const run = async (): Promise<void> => {
      result = await operation();
    };
    const queued = this.#tail.then(run, run);
    this.#tail = queued.catch(() => undefined);
    return queued.then(() => result!);
  }

  async idle(): Promise<void> {
    await this.#tail;
  }
}

/**
 * Run one complete tab-switch attempt. An uncached target is validated before
 * the backend-active swap, and the renderer publishes the tab only after that
 * swap succeeds. Keeping this orchestration independent from Tauri makes the
 * failure and rapid-click ordering contract directly testable.
 */
export async function switchSession<T>(options: {
  queue: SessionSwitchQueue;
  cached: T | undefined;
  load: () => Promise<T | null>;
  activate: () => Promise<T>;
  commit: (incoming: T) => T;
  publish: (committed: T) => void | Promise<void>;
}): Promise<T> {
  return options.queue.enqueue(async () => {
    let resolved = options.cached;
    if (!resolved) {
      const snapshot = await options.load();
      if (!snapshot) throw new Error("数据库会话未打开");
      resolved = options.commit(snapshot);
    }
    resolved = options.commit(await options.activate());
    await options.publish(resolved);
    return resolved;
  });
}

/**
 * Epoch-aware cache used when a session can be wholesale-replaced (for
 * example by downloading a remote database). Revision ordering handles
 * ordinary mutations; epochs reject every response that started before the
 * replacement, even if that old branch had a larger numeric revision.
 */
export class SessionStateCache<T extends { revision: number }> {
  #states = new Map<string, T>();
  #epochs = new Map<string, number>();

  get(sessionId: string): T | undefined {
    return this.#states.get(sessionId);
  }

  capture(sessionId: string): number {
    return this.#epochs.get(sessionId) ?? 0;
  }

  commit(sessionId: string, epoch: number, incoming: T): T {
    if (this.capture(sessionId) !== epoch) return this.#states.get(sessionId) ?? incoming;
    return commitNewestSessionState(this.#states, sessionId, incoming);
  }

  replace(sessionId: string, incoming: T): T {
    this.#epochs.set(sessionId, this.capture(sessionId) + 1);
    this.#states.set(sessionId, incoming);
    return incoming;
  }

  delete(sessionId: string): void {
    this.#states.delete(sessionId);
    this.#epochs.delete(sessionId);
  }

  clear(): void {
    this.#states.clear();
    this.#epochs.clear();
  }
}
