import { writable } from "svelte/store";
import type { MasterKeyExpiryOffer } from "$lib/types/vault";
import type { KdfPromptOwner } from "$lib/services/kdf-open-prompt";

export interface MasterKeyOpenPrompt {
  owner: KdfPromptOwner;
  offer: MasterKeyExpiryOffer | null;
  phase: "inspect" | "review" | "change" | "error";
  busy: boolean;
  error: string;
}
interface Host {
  enabled: () => boolean;
  inspect: (sessionId: string) => Promise<MasterKeyExpiryOffer>;
  close: (owner: KdfPromptOwner) => Promise<void>;
  error: (error: unknown) => string;
}
function same(a: KdfPromptOwner | null, b: KdfPromptOwner | null): boolean {
  return a === b || (!!a && !!b && a.sessionId === b.sessionId && a.epoch === b.epoch);
}

/** Opening flow policy, resolved before KDF upgrade. No credentials live here. */
export class MasterKeyOpenPromptController {
  readonly state = writable<MasterKeyOpenPrompt | null>(null);
  readonly ready = writable<KdfPromptOwner | null>(null);
  #owner: KdfPromptOwner | null = null;
  #prompt: MasterKeyOpenPrompt | null = null;
  #generation = 0;
  #seen = new Map<string, symbol>();
  #host: Host;
  constructor(host: Host) {
    this.#host = host;
  }
  #publish(prompt: MasterKeyOpenPrompt | null): void {
    this.#prompt = prompt;
    this.state.set(prompt);
  }
  #finish(owner: KdfPromptOwner): void {
    if (!same(this.#owner, owner)) return;
    this.#seen.set(owner.sessionId, owner.epoch);
    this.#publish(null);
    this.ready.set(owner);
  }
  activate(owner: KdfPromptOwner | null): void {
    if (same(this.#owner, owner)) return;
    this.#owner = owner;
    this.#generation++;
    this.ready.set(null);
    this.#publish(null);
    if (!owner) return;
    if (!this.#host.enabled() || this.#seen.get(owner.sessionId) === owner.epoch) {
      this.#finish(owner);
      return;
    }
    this.#inspect(owner);
  }
  #inspect(owner: KdfPromptOwner): void {
    const generation = ++this.#generation;
    this.#publish({ owner, offer: null, phase: "inspect", busy: true, error: "" });
    void this.#host
      .inspect(owner.sessionId)
      .then((offer) => {
        if (generation !== this.#generation || !same(this.#owner, owner)) return;
        if (!offer.required && !offer.recommended && offer.invalidRules.length === 0) {
          this.#finish(owner);
          return;
        }
        this.#publish({ owner, offer, phase: "review", busy: false, error: "" });
      })
      .catch((error) => {
        if (generation === this.#generation && same(this.#owner, owner)) {
          this.#publish({
            owner,
            offer: null,
            phase: "error",
            busy: false,
            error: this.#host.error(error),
          });
        }
      });
  }
  retry(): void {
    if (this.#owner && this.#prompt?.phase === "error" && !this.#prompt.busy)
      this.#inspect(this.#owner);
  }
  decline(): void {
    const prompt = this.#prompt;
    if (prompt?.phase === "review" && !prompt.busy && prompt.offer && !prompt.offer.required)
      this.#finish(prompt.owner);
  }
  change(): void {
    const prompt = this.#prompt;
    if (prompt?.phase === "review" && !prompt.busy && prompt.offer && !prompt.offer.readOnly) {
      this.#publish({ ...prompt, phase: "change", error: "" });
    }
  }
  back(): void {
    const prompt = this.#prompt;
    if (prompt?.phase === "change" && !prompt.busy)
      this.#publish({ ...prompt, phase: "review", error: "" });
  }
  setBusy(owner: KdfPromptOwner, busy: boolean): void {
    const prompt = this.#prompt;
    if (prompt?.phase === "change" && same(prompt.owner, owner) && prompt.busy !== busy)
      this.#publish({ ...prompt, busy });
  }
  changed(owner: KdfPromptOwner): void {
    if (this.#prompt?.phase === "change" && same(this.#prompt.owner, owner)) this.#finish(owner);
  }
  async close(): Promise<void> {
    const prompt = this.#prompt;
    if (!prompt || prompt.busy || !same(this.#owner, prompt.owner)) return;
    const generation = this.#generation;
    this.#publish({ ...prompt, busy: true, error: "" });
    try {
      await this.#host.close(prompt.owner);
      // The vault subscription normally detached this owner already. Never
      // acknowledge a failed/cancelled close or a replacement lifetime.
      if (generation === this.#generation) {
        this.#owner = null;
        this.ready.set(null);
        this.#publish(null);
      }
    } catch (error) {
      if (generation === this.#generation)
        this.#publish({ ...prompt, busy: false, error: this.#host.error(error) });
    }
  }
  retain(ids: string[]): void {
    const live = new Set(ids);
    if (this.#owner) live.add(this.#owner.sessionId);
    for (const id of this.#seen.keys()) if (!live.has(id)) this.#seen.delete(id);
  }
  dispose(): void {
    this.#generation++;
    this.#owner = null;
    this.#seen.clear();
    this.ready.set(null);
    this.#publish(null);
  }
}
