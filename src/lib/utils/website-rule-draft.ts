import type { WebsiteRule, WebsiteRuleSet, WebsiteRulesChange } from "$lib/types/vault";

export interface WebsiteDraftState {
  enabled: boolean;
  rules: WebsiteRule[];
  ready: boolean;
  loading: boolean;
  error: boolean;
  touched: boolean;
}

export function initialWebsiteDraft(enabled: boolean): WebsiteDraftState {
  return { enabled, rules: [], ready: !enabled, loading: false, error: false, touched: false };
}

/** Editor-local metadata only. No writes occur until the whole draft saves. */
export class WebsiteRulesDraft {
  private state: WebsiteDraftState;
  private operation = 0;
  private alive = true;
  private baselineEnabled: boolean;
  private publish: (state: WebsiteDraftState) => void;
  private current: () => boolean;

  constructor(
    enabled: boolean,
    publish: (state: WebsiteDraftState) => void,
    current: () => boolean,
  ) {
    this.state = initialWebsiteDraft(enabled);
    this.baselineEnabled = enabled;
    this.publish = publish;
    this.current = current;
  }

  private update(patch: Partial<WebsiteDraftState>): void {
    this.state = { ...this.state, ...patch };
    this.publish({ ...this.state, rules: this.state.rules.map((rule) => ({ ...rule })) });
  }

  async load(read: () => Promise<WebsiteRuleSet | null>): Promise<void> {
    if (!this.alive || !this.current()) return;
    const operation = ++this.operation;
    this.update({ loading: true, ready: false, error: false });
    const owns = () => this.alive && operation === this.operation && this.current();
    try {
      const value = await read();
      if (!owns()) return;
      this.baselineEnabled = value !== null;
      this.update({
        enabled: value !== null,
        rules: value?.rules.map((rule) => ({ ...rule })) ?? [],
        ready: true,
        loading: false,
        error: false,
        touched: false,
      });
    } catch {
      if (owns()) this.update({ loading: false, ready: false, error: true });
    }
  }

  replace(value: WebsiteRuleSet | null): void {
    if (!this.alive || !this.current()) return;
    this.operation++;
    this.update({
      enabled: value !== null,
      rules: value?.rules.map((rule) => ({ ...rule })) ?? [],
      ready: true,
      loading: false,
      error: false,
      touched: true,
    });
  }

  enable(enabled: boolean, seed: string): void {
    if (!this.alive || !this.current() || this.state.loading || !this.state.ready) return;
    const rules =
      enabled && this.state.rules.length === 0 && seed.trim()
        ? [{ url: seed.trim(), mode: "host" as const }]
        : this.state.rules;
    this.update({ enabled, rules, touched: true });
  }

  add(): void {
    if (!this.canEdit() || this.state.rules.length >= 128) return;
    this.update({ rules: [...this.state.rules, { url: "", mode: "host" }], touched: true });
  }

  edit(index: number, patch: Partial<WebsiteRule>): void {
    if (!this.canEdit() || !this.state.rules[index]) return;
    this.update({
      rules: this.state.rules.map((rule, i) => (i === index ? { ...rule, ...patch } : rule)),
      touched: true,
    });
  }

  remove(index: number): void {
    if (!this.canEdit() || !this.state.rules[index]) return;
    this.update({ rules: this.state.rules.filter((_, i) => i !== index), touched: true });
  }

  private canEdit(): boolean {
    return (
      this.alive && this.current() && this.state.ready && !this.state.loading && this.state.enabled
    );
  }

  change(): WebsiteRulesChange | null {
    if (!this.alive || !this.current() || !this.state.ready || this.state.loading)
      throw new Error("Website draft unavailable");
    if (!this.state.touched || (!this.state.enabled && !this.baselineEnabled)) return null;
    return this.state.enabled
      ? { mode: "set", rules: { version: 1, rules: this.state.rules.map((rule) => ({ ...rule })) } }
      : { mode: "clear" };
  }

  dispose(): void {
    this.alive = false;
    this.operation++;
  }
}
