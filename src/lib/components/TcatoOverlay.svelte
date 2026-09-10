<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  import AppIcon from "$lib/components/AppIcon.svelte";

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  let title = $state("");
  let hasPassword = $state(false);
  let hasUsername = $state(false);
  let hasTotp = $state(false);
  let lastWindow = $state<string | null>(null);
  let candidates = $state<{ sessionId: string; uuid: string; title: string; username: string }[]>(
    [],
  );
  let feedback = $state("");
  let error = $state("");

  async function loadState(): Promise<void> {
    try {
      const info = await invoke<{
        title: string;
        username: string;
        hasPassword: boolean;
        hasUsername: boolean;
        hasTotp: boolean;
        lastWindow: string | null;
        pending: { sessionId: string; uuid: string; title: string; username: string }[];
      } | null>("tcato_state");
      if (!info) {
        error = t(lang, "tcato.noTarget");
        return;
      }
      title = info.title;
      hasPassword = info.hasPassword;
      hasUsername = info.hasUsername;
      hasTotp = info.hasTotp ?? false;
      lastWindow = info.lastWindow ?? null;
      candidates = info.pending ?? [];
    } catch (e) {
      error = t(lang, "tcato.readFailed", { error: String(e) });
    }
  }

  onMount(() => {
    void loadState();
  });

  async function send(channel: "username" | "password" | "totp"): Promise<void> {
    feedback = "";
    error = "";
    try {
      await invoke("tcato_send", { channel });
      feedback =
        channel === "username"
          ? t(lang, "tcato.injectedUsername")
          : channel === "password"
            ? t(lang, "tcato.injectedPassword")
            : t(lang, "tcato.injectedTotp");
    } catch (e) {
      error = `${e}`;
    }
  }

  async function pick(candidate: { sessionId: string; uuid: string }): Promise<void> {
    feedback = "";
    error = "";
    try {
      await invoke("open_tcato_overlay", {
        sessionId: candidate.sessionId,
        uuid: candidate.uuid,
      });
      await loadState();
    } catch (e) {
      error = `${e}`;
    }
  }

  function close(): void {
    // Surface a failed close instead of leaving a dead overlay silently open.
    void invoke("close_tcato_overlay").catch((e) => {
      error = `${e}`;
    });
  }
</script>

<div class="tcato-shell">
  <header>
    <span class="title-icon"><AppIcon name="shield" size={15} /></span>
    <div class="heading">
      <strong>{t(lang, "tcato.title")}</strong>
      <p {title}>{title || t(lang, "tcato.loading")}</p>
    </div>
    <button class="close-button" onclick={close} aria-label={t(lang, "common.close")}>×</button>
  </header>

  <p class="hint">{t(lang, "tcato.hint")}</p>
  {#if lastWindow}
    <p class="hint">{t(lang, "tcato.lastTarget", { window: lastWindow })}</p>
  {/if}

  {#if candidates.length > 0}
    <div class="pick-list" role="listbox" aria-label={t(lang, "tcato.pickLabel")}>
      {#each candidates as candidate (candidate.uuid)}
        <button
          type="button"
          class="pick-item"
          role="option"
          aria-selected="false"
          onmousedown={(e) => e.preventDefault()}
          onclick={() => pick(candidate)}
        >
          <span class="pick-title">{candidate.title || t(lang, "tcato.untitled")}</span>
          {#if candidate.username}
            <span class="pick-username">{candidate.username}</span>
          {/if}
        </button>
      {/each}
    </div>
  {:else}
    <div class="actions">
      <button
        class="channel-button"
        onmousedown={(e) => e.preventDefault()}
        onclick={() => send("username")}
        disabled={!hasUsername}
      >
        <AppIcon name="user" size={13} />{t(lang, "tcato.username")}
      </button>
      <button
        class="channel-button primary"
        onmousedown={(e) => e.preventDefault()}
        onclick={() => send("password")}
        disabled={!hasPassword}
      >
        <AppIcon name="key" size={13} />{t(lang, "tcato.password")}
      </button>
      <button
        class="channel-button"
        onmousedown={(e) => e.preventDefault()}
        onclick={() => send("totp")}
        disabled={!hasTotp}
        title={t(lang, "tcato.totpTitle")}
      >
        <AppIcon name="clock" size={13} />{t(lang, "tcato.totp")}
      </button>
    </div>
  {/if}

  {#if feedback}
    <p class="feedback ok">{feedback}</p>
  {/if}
  {#if error}
    <p class="feedback error">{error}</p>
  {/if}
</div>

<style>
  .tcato-shell {
    display: flex;
    flex-direction: column;
    gap: 10px;
    height: 100vh;
    padding: 12px;
    box-sizing: border-box;
    background: var(--bg-app);
    color: var(--text-primary);
    font-size: 12px;
    user-select: none;
  }

  header {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .title-icon {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    background: color-mix(in srgb, var(--selection-color) 18%, transparent);
    color: var(--selection-color);
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  .heading strong {
    display: block;
    font-size: 13px;
  }

  .heading p {
    margin: 2px 0 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-secondary);
    font-size: 11px;
  }

  .close-button {
    border: none;
    background: transparent;
    color: var(--text-secondary);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 4px;
  }

  .close-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .hint {
    margin: 0;
    color: var(--text-secondary);
    font-size: 11px;
    line-height: 1.5;
  }

  .actions {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .channel-button {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 30px;
    border: 1px solid var(--border-color);
    border-radius: 6px;
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: 12px;
    cursor: pointer;
  }

  .channel-button:hover:not(:disabled) {
    border-color: var(--selection-color);
    background: var(--hover-bg);
  }

  .channel-button.primary {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 16%, var(--input-bg));
  }

  .channel-button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .pick-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .pick-item {
    display: flex;
    align-items: baseline;
    gap: 8px;
    min-width: 0;
    padding: 6px 8px;
    border: 1px solid var(--border-subtle);
    border-radius: 6px;
    background: var(--input-bg);
    color: var(--text-primary);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .pick-item:hover {
    background: var(--hover-bg);
  }

  .pick-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .pick-username {
    flex: 1;
    overflow: hidden;
    color: var(--text-faint);
    font-size: 10px;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .feedback {
    margin: 0;
    font-size: 11px;
  }

  .feedback.ok {
    color: var(--success-color);
  }

  .feedback.error {
    color: var(--danger-color);
  }
</style>
