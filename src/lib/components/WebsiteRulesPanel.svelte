<script lang="ts">
  import { tick } from "svelte";
  import { t } from "$lib/i18n";
  import type { Language } from "$lib/types/settings";
  import type { WebsiteRule, WebsiteMode } from "$lib/types/vault";
  import type { WebsiteDraftState } from "$lib/utils/website-rule-draft";
  import Select from "$lib/components/Select.svelte";
  import Toggle from "$lib/components/templates/form/Toggle.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Feedback from "$lib/components/templates/form/Feedback.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    state: WebsiteDraftState;
    lang: Language;
    disabled: boolean;
    conflict: boolean;
    onretry: () => void;
    onclear: () => void;
    onenable: (enabled: boolean) => void;
    onadd: () => void;
    onedit: (index: number, patch: Partial<WebsiteRule>) => void;
    onremove: (index: number) => void;
  }
  let {
    state: draftState,
    lang,
    disabled,
    conflict,
    onretry,
    onclear,
    onenable,
    onadd,
    onedit,
    onremove,
  }: Props = $props();
  let root: HTMLDivElement | undefined = $state();
  const id = $props.id();
  const options = $derived([
    { value: "host" as WebsiteMode, label: t(lang, "editor.websiteHost") },
    { value: "site" as WebsiteMode, label: t(lang, "editor.websiteSite") },
    { value: "exact" as WebsiteMode, label: t(lang, "editor.websiteExact") },
    { value: "never" as WebsiteMode, label: t(lang, "editor.websiteNever") },
  ]);
  const unavailable = $derived(disabled || draftState.loading || !draftState.ready);

  async function add(): Promise<void> {
    const index = draftState.rules.length;
    onadd();
    await tick();
    root?.querySelector<HTMLInputElement>(`[data-rule="${index}"] input`)?.focus();
  }

  async function remove(index: number): Promise<void> {
    onremove(index);
    await tick();
    const next = Math.min(index, draftState.rules.length - 1);
    const target = root?.querySelector<HTMLElement>(
      next >= 0 ? `[data-rule="${next}"] input` : ".website-add button",
    );
    target?.focus();
  }
</script>

<div class="website-panel" bind:this={root} aria-busy={draftState.loading}>
  <div class="website-heading">
    <span>{t(lang, "editor.websiteEnable")}</span>
    <Toggle
      checked={draftState.enabled}
      ariaLabel={t(lang, "editor.websiteEnable")}
      disabled={unavailable}
      onchange={onenable}
    />
  </div>
  <p class="website-hint">{t(lang, "editor.websiteHint")}</p>
  {#if draftState.loading}
    <p class="website-hint" role="status">{t(lang, "editor.websiteLoading")}</p>
  {:else if draftState.error}
    <Feedback inline message={t(lang, "editor.websiteLoadFailed")} />
    <div class="website-actions">
      <Button variant="action" {disabled} onclick={onretry}>{t(lang, "editor.websiteRetry")}</Button
      >
      <Button variant="action" {disabled} onclick={onclear}>{t(lang, "editor.websiteClear")}</Button
      >
    </div>
  {:else if draftState.ready}
    {#if !draftState.enabled}
      <p class="website-hint">{t(lang, "editor.websiteLegacy")}</p>
    {:else}
      {#if draftState.rules.length === 0}
        <p class="website-hint">{t(lang, "editor.websiteEmpty")}</p>
      {/if}
      {#each draftState.rules as rule, index (index)}
        <div class="website-rule" data-rule={index}>
          <div class="website-url">
            <TextField
              id={`${id}-url-${index}`}
              value={rule.url}
              ariaLabel={t(lang, "editor.websiteUrl", { count: index + 1 })}
              placeholder="https://example.com"
              maxlength={4096}
              mono
              size="control"
              autocomplete="off"
              spellcheck={false}
              disabled={unavailable}
              oninput={(event) => onedit(index, { url: event.currentTarget.value })}
            />
          </div>
          <Select
            value={rule.mode}
            {options}
            ariaLabel={t(lang, "editor.websiteMode", { count: index + 1 })}
            disabled={unavailable}
            onchange={(mode) => onedit(index, { mode })}
          />
          <Button
            variant="field"
            ariaLabel={t(lang, "editor.websiteRemove", { count: index + 1 })}
            disabled={unavailable}
            onclick={() => void remove(index)}><AppIcon name="x" size={13} /></Button
          >
        </div>
      {/each}
      <div class="website-actions website-add">
        <Button
          variant="action"
          disabled={unavailable || draftState.rules.length >= 128}
          onclick={() => void add()}
          ><AppIcon name="plus" size={12} />{t(lang, "editor.websiteAdd")}</Button
        >
        <span class="website-hint"
          >{t(lang, "editor.websiteCount", { count: draftState.rules.length })}</span
        >
      </div>
      <p class="website-hint">{t(lang, "editor.websiteModesHint")}</p>
    {/if}
    {#if conflict}<Feedback inline message={t(lang, "editor.websiteOverrideConflict")} />{/if}
  {/if}
</div>

<style>
  .website-panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .website-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
  }
  .website-hint {
    margin: 0;
    color: var(--text-muted);
    font-size: var(--font-size-secondary, 11px);
    line-height: 1.5;
    overflow-wrap: anywhere;
  }
  .website-rule {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .website-url {
    flex: 1;
    min-width: 0;
  }
  .website-actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
  }
  @media (max-width: 480px) {
    .website-rule {
      flex-wrap: wrap;
    }
    .website-url {
      flex-basis: 100%;
    }
  }
</style>
