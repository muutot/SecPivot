<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import type { IconName } from "$lib/components/AppIcon.svelte";

  interface Props {
    icon?: IconName;
    logoSrc?: string;
    title: string;
    subtitle?: string;
    /** Optional footer snippet (e.g., guard toggle, recent files). */
    children?: import("svelte").Snippet;
  }

  let { icon = "key", logoSrc, title, subtitle, children }: Props = $props();
</script>

<div class="standalone-shell">
  <div class="standalone-shell__inner">
    <div class="standalone-shell__header">
      <div class="standalone-shell__logo">
        {#if logoSrc}
          <img class="standalone-shell__logo-img" src={logoSrc} alt={title} />
        {:else}
          <AppIcon name={icon} size={34} />
        {/if}
      </div>
      <div class="standalone-shell__heading">
        <h1 class="standalone-shell__title">{title}</h1>
        {#if subtitle}<p class="standalone-shell__subtitle">{subtitle}</p>{/if}
      </div>
    </div>
    {#if children}{@render children()}{/if}
  </div>
</div>

<style>
  .standalone-shell {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
  }

  .standalone-shell__inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    max-width: 380px;
    padding: 24px;
    text-align: center;
    transform: translateY(10vh);
  }

  .standalone-shell__header {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .standalone-shell__logo {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 56px;
    flex: 0 0 auto;
    overflow: hidden;
    border: 1px solid var(--border-color);
    border-radius: 16px;
    color: var(--warning-color);
    background: var(--card-bg);
  }

  .standalone-shell__logo-img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .standalone-shell__heading {
    min-width: 0;
    text-align: left;
  }

  .standalone-shell__title {
    margin: 0;
    color: var(--text-primary);
    font-size: 24px;
    font-weight: 590;
    letter-spacing: 0.01em;
  }

  .standalone-shell__subtitle {
    margin: 4px 0 0;
    color: var(--text-muted);
    font-size: 12px;
  }
</style>
