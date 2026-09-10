<script lang="ts">
  import type { SimilarPasswordGroup } from "$lib/types/vault";
  import { vault } from "$lib/services/vault";
  import ModalShell from "$lib/components/ModalShell.svelte";

  import Button from "$lib/components/templates/action/Button.svelte";
  import { appSettings } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  interface Props {
    onclose: () => void;
    onselect?: (uuid: string) => void;
  }

  let { onclose, onselect }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const lang = $derived(s.general.language);

  let groups = $state<SimilarPasswordGroup[]>([]);
  let loading = $state(true);
  let error = $state("");
  const sessionId = vault.getActiveSessionId();

  $effect(() => {
    if (!sessionId) return;
    void vault
      .callInSession(sessionId, () => vault.similarPasswords())
      .then((value) => {
        if (vault.getActiveSessionId() !== sessionId) return;
        groups = value;
        loading = false;
      })
      .catch((e) => {
        if (vault.getActiveSessionId() !== sessionId) return;
        error = String(e);
        loading = false;
      });
  });
</script>

<ModalShell
  title={t(lang, "similar.title")}
  description={t(lang, "similar.desc")}
  size="large"
  scrollable
  closeOnEscape
  {onclose}
>
  {#snippet children()}
    {#if loading}
      <p class="note">{t(lang, "similar.analyzing")}</p>
    {:else if error}
      <p class="note error">{error}</p>
    {:else if groups.length === 0}
      <p class="note">{t(lang, "similar.empty")}</p>
    {:else}
      <p class="note">{t(lang, "similar.count", { count: groups.length })}</p>
      {#each groups as group, gi (gi)}
        <section class="group">
          <h3 class="group-title">
            {t(lang, "similar.groupTitle", { index: gi + 1, count: group.entries.length })}
          </h3>
          <ul class="members">
            {#each group.entries as member (member.uuid)}
              <li>
                <button
                  type="button"
                  class="member"
                  onclick={() => onselect?.(member.uuid)}
                  title={t(lang, "hibp.locateEntry")}
                >
                  <span class="member-title">{member.title}</span>
                  <span class="member-user">{member.username}</span>
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    {/if}
  {/snippet}
  {#snippet actions()}
    <Button onclick={onclose}>{t(lang, "common.close")}</Button>
  {/snippet}
</ModalShell>

<style>
  .note {
    margin: 8px 0;
    color: var(--text-faint);
    font-size: var(--font-size-secondary, 11px);
  }

  .note.error {
    color: var(--danger-color);
  }

  .group {
    margin-bottom: 14px;
  }

  .group-title {
    margin: 0 0 6px;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    font-weight: 600;
  }

  .members {
    margin: 0;
    padding: 0;
    overflow: hidden;
    border: 1px solid var(--border-subtle);
    border-radius: var(--settings-control-radius, 6px);
    list-style: none;
  }

  .member {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 6px 10px;
    border: none;
    border-bottom: 1px solid var(--border-subtle);
    color: var(--text-primary);
    background: transparent;
    font-size: var(--font-size-secondary, 11px);
    text-align: left;
    cursor: pointer;
  }

  .member:last-child {
    border-bottom: none;
  }

  .member:hover {
    background: var(--hover-bg);
  }

  .member-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .member-user {
    color: var(--text-faint);
  }
</style>
