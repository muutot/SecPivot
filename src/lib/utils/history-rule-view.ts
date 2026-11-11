import type { HistoryRuleSource, WebsiteRuleSet } from "$lib/types/vault";

export interface HistoryRuleViewState {
  visible: boolean;
  source: HistoryRuleSource;
  loading: boolean;
  ready: boolean;
  error: boolean;
  value: WebsiteRuleSet | null;
}

export function initialHistoryRuleView(): HistoryRuleViewState {
  return {
    visible: false,
    source: "version",
    loading: false,
    ready: false,
    error: false,
    value: null,
  };
}

/** Ephemeral explicit reads, bound to one selected native history lifetime. */
export class HistoryRuleView {
  private state = initialHistoryRuleView();
  private alive = true;
  private operation = 0;
  private read: (source: HistoryRuleSource) => Promise<WebsiteRuleSet | null>;
  private publish: (state: HistoryRuleViewState) => void;
  private current: () => boolean;

  constructor(
    read: (source: HistoryRuleSource) => Promise<WebsiteRuleSet | null>,
    publish: (state: HistoryRuleViewState) => void,
    current: () => boolean,
  ) {
    this.read = read;
    this.publish = publish;
    this.current = current;
  }

  private update(patch: Partial<HistoryRuleViewState>): void {
    this.state = { ...this.state, ...patch };
    this.publish({ ...this.state });
  }

  async load(source: HistoryRuleSource): Promise<void> {
    if (!this.alive || !this.current()) return;
    const operation = ++this.operation;
    this.update({ visible: true, source, loading: true, ready: false, error: false, value: null });
    const owns = () => this.alive && this.current() && operation === this.operation;
    try {
      const value = await this.read(source);
      if (!owns()) return;
      this.update({
        loading: false,
        ready: true,
        value:
          value === null
            ? null
            : { version: value.version, rules: value.rules.map((rule) => ({ ...rule })) },
      });
    } catch {
      if (owns()) this.update({ loading: false, ready: false, error: true, value: null });
    }
  }

  hide(): void {
    if (!this.alive) return;
    this.operation++;
    this.update({ ...initialHistoryRuleView(), source: this.state.source });
  }

  dispose(): void {
    this.alive = false;
    this.operation++;
    this.state = initialHistoryRuleView();
  }
}
