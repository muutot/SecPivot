<script lang="ts">
  import type { Snippet } from "svelte";

  /** Viewport-menu item template (`.menu-item` from viewport-menu-shared).
   *  Owns the base row style; callers supply leading icons/checks via the
   *  `leading` snippet and per-variant styling stays with the caller. */
  interface Props {
    label: string;
    role?: "menuitem" | "menuitemcheckbox";
    checked?: boolean;
    destructive?: boolean;
    disabled?: boolean;
    title?: string;
    onclick?: () => void;
    /** Leading slot (icon / check mark). */
    leading?: Snippet;
  }

  let {
    label,
    role = "menuitem",
    checked = false,
    destructive = false,
    disabled = false,
    title = undefined,
    onclick,
    leading,
  }: Props = $props();
</script>

<button
  type="button"
  class="item"
  class:checked
  class:destructive
  {role}
  aria-checked={role === "menuitemcheckbox" ? checked : undefined}
  {disabled}
  {title}
  {onclick}
>
  {#if leading}<span class="leading">{@render leading()}</span>{/if}
  <span class="label">{label}</span>
</button>

<style>
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 7px 10px;
    border: 0;
    border-radius: 4px;
    background: transparent;
    color: var(--text-secondary);
    font-size: var(--font-size-secondary, 11px);
    text-align: left;
    cursor: pointer;
    transition: background 80ms ease;
  }

  .item:hover:not(:disabled) {
    background: var(--hover-bg);
    color: var(--text-primary);
  }

  .item:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .item.destructive {
    color: var(--danger-color);
  }

  .item.destructive:hover:not(:disabled) {
    background: color-mix(in srgb, var(--danger-color) 12%, transparent);
    color: var(--danger-color);
  }

  .leading {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    flex-shrink: 0;
  }

  :global(.viewport-menu--column-config) .leading {
    width: 14px;
    color: var(--text-faint);
    opacity: 0;
  }

  :global(.viewport-menu--column-config) .item.checked .leading {
    opacity: 1;
    color: var(--selection-color);
  }

  .label {
    flex: 1;
    min-width: 0;
  }
</style>
