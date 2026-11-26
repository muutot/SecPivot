import { writable } from "svelte/store";
import type { KdfUpgradeOffer } from "$lib/types/vault";

export interface KdfPromptOwner {
  sessionId: string;
  epoch: symbol;
}

export interface KdfOpenPrompt {
  owner: KdfPromptOwner;
  offer: KdfUpgradeOffer;
  busy: boolean;
  error: string;
}

interface Host {
  enabled: () => boolean;
  inspect: (sessionId: string) => Promise<KdfUpgradeOffer>;
  upgrade: (sessionId: string, offer: KdfUpgradeOffer) => Promise<unknown>;
  error: (error: unknown) => string;
}

function sameOwner(a: KdfPromptOwner | null, b: KdfPromptOwner | null): boolean {
  return (
    a === b || (a !== null && b !== null && a.sessionId === b.sessionId && a.epoch === b.epoch)
  );
}

/** One review per decrypted lifetime. View changes invalidate pending UI work;
 * native token/parameter validation independently guards the actual mutation. */
export class KdfOpenPromptController {
  readonly state = writable<KdfOpenPrompt | null>(null);
  #owner: KdfPromptOwner | null = null;
  #generation = 0;
  #seen = new Map<string, symbol>();
  #prompt: KdfOpenPrompt | null = null;
  #host: Host;

  constructor(host: Host) {
    this.#host = host;
  }

  #publish(prompt: KdfOpenPrompt | null): void {
    this.#prompt = prompt;
    this.state.set(prompt);
  }

  activate(owner: KdfPromptOwner | null): void {
    if (sameOwner(this.#owner, owner)) return;
    this.#owner = owner;
    const generation = ++this.#generation;
    this.#publish(null);
    if (!owner || this.#seen.get(owner.sessionId) === owner.epoch) return;
    if (!this.#host.enabled()) {
      this.#seen.set(owner.sessionId, owner.epoch);
      return;
    }
    void this.#host
      .inspect(owner.sessionId)
      .then((offer) => {
        if (generation !== this.#generation || !sameOwner(this.#owner, owner)) return;
        this.#seen.set(owner.sessionId, owner.epoch);
        if (this.#host.enabled() && offer.recommended) {
          this.#publish({ owner, offer, busy: false, error: "" });
        }
      })
      .catch(() => {
        // An inspection failure must not turn a successful unlock into a failure.
        // Revisit can inspect again; no upgrade or acknowledgement was published.
      });
  }

  retain(sessionIds: string[]): void {
    const live = new Set(sessionIds);
    if (this.#owner) live.add(this.#owner.sessionId);
    for (const id of this.#seen.keys()) if (!live.has(id)) this.#seen.delete(id);
  }

  dismiss(): void {
    if (!this.#prompt?.busy) this.#publish(null);
  }

  async upgrade(): Promise<void> {
    const prompt = this.#prompt;
    if (!prompt || prompt.busy || prompt.offer.readOnly || !sameOwner(this.#owner, prompt.owner))
      return;
    if (!this.#host.enabled()) {
      this.#publish(null);
      return;
    }
    const generation = this.#generation;
    this.#publish({ ...prompt, busy: true, error: "" });
    try {
      await this.#host.upgrade(prompt.owner.sessionId, prompt.offer);
      if (generation === this.#generation) this.#publish(null);
    } catch (error) {
      if (generation === this.#generation) {
        this.#publish({ ...prompt, busy: false, error: this.#host.error(error) });
      }
    }
  }

  dispose(): void {
    this.#generation++;
    this.#owner = null;
    this.#seen.clear();
    this.#publish(null);
  }
}
