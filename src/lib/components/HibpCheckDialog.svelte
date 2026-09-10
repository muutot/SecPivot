<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import type { BreachFinding, HibpProgress } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import { hibpResultState } from "$lib/utils/hibp";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  import ModalShell from "$lib/components/ModalShell.svelte";

  import Button from "$lib/components/templates/action/Button.svelte";
  interface Props {
    uuids?: string[];
    onclose: () => void;
    onselect?: (uuid: string) => void;
  }

  let { uuids = [], onclose, onselect }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  // Strict opt-in: the check only runs after the user explicitly clicks
  // the start button on the privacy screen.
  let started = $state(false);
  let running = $state(false);
  let findings = $state<BreachFinding[]>([]);
  let error = $state("");
  let progress = $state<HibpProgress | null>(null);
  /** Set when the user clicks the stop-waiting button: the resolved findings
   *  are partial, so the dialog must not present the run as a completed
   *  clean check. */
  let cancelled = $state(false);
  const sessionId = vault.getActiveSessionId();

  const resultState = $derived(
    hibpResultState({
      started,
      running,
      failed: error !== "",
      findingCount: findings.length,
      cancelled,
    }),
  );

  const progressPct = $derived(
    progress && progress.total > 0
      ? `${Math.round((progress.done / progress.total) * 100)}%`
      : "0%",
  );

  async function start(): Promise<void> {
    if (running || !sessionId) return;
    running = true;
    error = "";
    started = true;
    cancelled = false;
    progress = { sessionId, done: 0, total: 0 };
    const unlisten = await listen<HibpProgress>("hibp-progress", (e) => {
      if (e.payload.sessionId !== sessionId) return;
      if (vault.getActiveSessionId() !== sessionId) return;
      progress = e.payload;
    });
    try {
      const value = await vault.callInSession(sessionId, () =>
        vault.checkHibp(uuids.length > 0 ? uuids : undefined),
      );
      if (vault.getActiveSessionId() !== sessionId) return;
      findings = value;
    } catch (e) {
      if (vault.getActiveSessionId() !== sessionId) return;
      error = String(e);
    } finally {
      unlisten();
      running = false;
    }
  }

  function cancel(): void {
    cancelled = true;
    void vault.cancelHibp();
  }
</script>

<ModalShell
  title={t(lang, "hibp.title")}
  description={t(lang, "hibp.desc")}
  size="medium"
  scrollable
  closeOnEscape={!running}
  {onclose}
>
  {#snippet children()}
    {#if !started}
      <div class="privacy">
        <p>{t(lang, "hibp.privacyIntro")}</p>
        <ul>
          <li>{t(lang, "hibp.privacyLocal")}</li>
          <li>{t(lang, "hibp.privacyOnce")}</li>
          <li>{t(lang, "hibp.privacyNetwork")}</li>
        </ul>
      </div>
    {:else if running}
      <p class="note">
        {progress && progress.total > 0
          ? t(lang, "hibp.progress", { done: progress.done, total: progress.total })
          : t(lang, "hibp.checking")}
      </p>
      <div class="progress-track">
        <div
          class="progress-fill"
          class:indeterminate={!progress || progress.total === 0}
          style:--progress-pct={progressPct}
        ></div>
      </div>
    {:else if resultState === "error"}
      <p class="note error">{error}</p>
    {:else if resultState === "cancelled-clean"}
      <p class="note">{t(lang, "hibp.cancelledClean")}</p>
    {:else if resultState === "clean"}
      <p class="note success">{t(lang, "hibp.clean")}</p>
    {:else}
      {#if resultState === "cancelled-hits"}
        <p class="note">{t(lang, "hibp.cancelledPartial")}</p>
      {:else}
        <p class="note">{t(lang, "hibp.hits", { count: findings.length })}</p>
      {/if}
      <ul class="list">
        {#each findings as finding (finding.uuid)}
          <li class="row">
            <button
              type="button"
              class="main"
              onclick={() => onselect?.(finding.uuid)}
              title={t(lang, "hibp.locateEntry")}
            >
              <span class="title">{finding.title}</span>
              <span class="sub">{finding.username}</span>
            </button>
            <span class="count">{t(lang, "hibp.countTimes", { count: finding.count })}</span>
          </li>
        {/each}
      </ul>
      <p class="note">{t(lang, "hibp.advice")}</p>
    {/if}
  {/snippet}
  {#snippet actions()}
    {#if !started}
      <Button onclick={onclose}>{t(lang, "common.cancel")}</Button>
      <Button variant="primary" onclick={() => void start()}>{t(lang, "hibp.start")}</Button>
    {:else if running}
      <Button onclick={cancel}>{t(lang, "common.cancelWait")}</Button>
      <Button variant="primary" onclick={onclose}>{t(lang, "common.close")}</Button>
    {:else}
      <Button variant="primary" onclick={onclose}>{t(lang, "common.close")}</Button>
    {/if}
  {/snippet}
</ModalShell>

<style>
  .privacy p {
    margin: 0 0 8px;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    line-height: 1.5;
  }

  .privacy ul {
    margin: 0;
    padding-left: 18px;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
    line-height: 1.6;
  }

  .note {
    margin: 8px 0;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
  }

  .note.error {
    color: var(--danger-color);
  }

  .note.success {
    color: var(--success-color);
  }

  .list {
    margin: 0;
    padding: 0;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    list-style: none;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .row:last-child {
    border-bottom: none;
  }

  .main {
    display: flex;
    flex: 1;
    min-width: 0;
    flex-direction: column;
    gap: 2px;
    padding: 0;
    border: none;
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .title {
    overflow: hidden;
    color: var(--text-primary);
    font-size: var(--font-size-secondary, 11px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    overflow: hidden;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    flex: none;
    color: var(--danger-color);
    font-size: var(--font-size-tiny, 10px);
  }

  .progress-track {
    height: 6px;
    margin-top: 8px;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
    overflow: hidden;
  }

  .progress-fill {
    width: var(--progress-pct, 0%);
    height: 100%;
    border-radius: inherit;
    background: var(--selection-color);
    transition: width 0.2s ease;
  }

  .progress-fill.indeterminate {
    width: 40%;
    animation: progress-slide 1.1s ease-in-out infinite alternate;
  }

  @keyframes progress-slide {
    from {
      transform: translateX(-110%);
    }
    to {
      transform: translateX(260%);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .progress-fill {
      transition: none;
    }
    .progress-fill.indeterminate {
      animation: none;
    }
  }
</style>
