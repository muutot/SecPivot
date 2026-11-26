import { writable } from "svelte/store";

export interface KeyfileSuggestions {
  paths: string[];
  incomplete: boolean;
  supported: boolean;
}
interface Owner {
  scope: string;
  removable: boolean;
}
export interface KeyfileSearchState {
  busy: boolean;
  result: KeyfileSuggestions | null;
  failed: boolean;
}

/** Suggestions contain paths only; explicit selection never submits a vault. */
export class KeyfileSearchController {
  readonly state = writable<KeyfileSearchState>({ busy: false, result: null, failed: false });
  #owner: Owner | null = null;
  #generation = 0;
  #result: KeyfileSuggestions | null = null;
  #inspect: () => Promise<KeyfileSuggestions>;
  constructor(inspect: () => Promise<KeyfileSuggestions>) {
    this.#inspect = inspect;
  }
  activate(owner: Owner | null): void {
    if (
      owner === this.#owner ||
      (owner &&
        this.#owner &&
        owner.scope === this.#owner.scope &&
        owner.removable === this.#owner.removable)
    )
      return;
    this.#owner = owner;
    const generation = ++this.#generation;
    this.#result = null;
    this.state.set({ busy: !!owner, result: null, failed: false });
    if (!owner) return;
    void this.#inspect()
      .then((result) => {
        if (generation !== this.#generation) return;
        this.#result = result;
        this.state.set({ busy: false, result, failed: false });
      })
      .catch(() => {
        if (generation === this.#generation)
          this.state.set({ busy: false, result: null, failed: true });
      });
  }
  allows(path: string): boolean {
    return !!this.#owner && !!this.#result?.supported && this.#result.paths.includes(path);
  }
  dispose(): void {
    this.#generation++;
    this.#owner = null;
    this.#result = null;
    this.state.set({ busy: false, result: null, failed: false });
  }
}
