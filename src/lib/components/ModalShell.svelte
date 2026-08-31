<script lang="ts">
  import type { Snippet } from "svelte";
  import AppIcon from "$lib/components/AppIcon.svelte";

  export type ModalSize = "small" | "confirm" | "medium" | "large" | "report";
  export type ModalTone = "default" | "danger";

  interface Props {
    title: string;
    description?: string;
    ariaLabel?: string;
    size?: ModalSize;
    tone?: ModalTone;
    prompt?: boolean;
    scrollable?: boolean;
    showClose?: boolean;
    closeOnEscape?: boolean;
    onclose?: () => void;
    icon?: Snippet;
    /** Extra header-end actions (icon buttons); rendered between the heading
     *  and the close button so callers can promote dialog actions to the
     *  title bar. */
    headerActions?: Snippet;
    children?: Snippet;
    actions?: Snippet;
  }

  let {
    title,
    description,
    ariaLabel,
    size = "medium",
    tone = "default",
    prompt = false,
    scrollable = false,
    showClose = false,
    closeOnEscape = false,
    onclose,
    icon,
    headerActions,
    children,
    actions,
  }: Props = $props();

  let dialogRef = $state<HTMLDivElement>();

  /** Focusable descendants kept inside the dialog while it is open. */
  const FOCUSABLE_SELECTOR =
    "a[href], button:not([disabled]), textarea:not([disabled]), input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex='-1'])";

  function focusables(): HTMLElement[] {
    if (!dialogRef) return [];
    return Array.from(dialogRef.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));
  }

  function handleKeydown(event: KeyboardEvent): void {
    if (closeOnEscape && event.key === "Escape") {
      event.preventDefault();
      onclose?.();
      return;
    }
    // Focus trap: keyboard navigation must not escape a modal dialog even
    // though background content remains in the DOM behind the backdrop.
    if (event.key !== "Tab" || !dialogRef) return;
    const items = focusables();
    if (items.length === 0) {
      event.preventDefault();
      dialogRef.focus();
      return;
    }
    const index = items.indexOf(document.activeElement as HTMLElement);
    if (index === -1) {
      event.preventDefault();
      (event.shiftKey ? items[items.length - 1] : items[0]).focus();
      return;
    }
    const next = index + (event.shiftKey ? -1 : 1);
    if (next < 0 || next >= items.length) {
      event.preventDefault();
      items[next < 0 ? items.length - 1 : 0].focus();
    }
  }

  // Initial focus goes to the dialog itself; focus returns to the invoker
  // when the modal unmounts so keyboard users are not stranded.
  $effect(() => {
    const previous = document.activeElement as HTMLElement | null;
    dialogRef?.focus();
    return () => previous?.focus();
  });
</script>

<svelte:window onkeydown={handleKeydown} />

<div class="modal-backdrop" class:modal-backdrop--prompt={prompt} role="presentation">
  <div
    bind:this={dialogRef}
    class={`modal-shell modal-shell--${size}`}
    class:modal-shell--scrollable={scrollable}
    role="dialog"
    aria-modal="true"
    aria-label={ariaLabel ?? title}
    tabindex="-1"
  >
    <div class="modal-shell__head">
      {#if icon}
        <span class="modal-shell__icon" class:modal-shell__icon--danger={tone === "danger"}>
          {@render icon()}
        </span>
      {/if}
      <div class="modal-shell__heading">
        <strong>{title}</strong>
        {#if description}<p>{description}</p>{/if}
      </div>
      {#if headerActions}<div class="modal-shell__header-actions">
          {@render headerActions()}
        </div>{/if}
      {#if showClose && onclose}
        <button
          class="modal-shell__close"
          type="button"
          onclick={onclose}
          title="关闭"
          aria-label="关闭"
        >
          <AppIcon name="x" size={14} />
        </button>
      {/if}
    </div>

    {#if children}<div class="modal-shell__body">{@render children()}</div>{/if}
    {#if actions}<footer class="modal-actions">{@render actions()}</footer>{/if}
  </div>
</div>

<style>
  .modal-shell {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 20px;
    border: 1px solid var(--border-color);
    border-radius: 13px;
    background: var(--surface-bg);
    box-shadow: 0 12px 40px color-mix(in srgb, #000 40%, transparent);
  }

  /* The dialog container receives initial focus for the focus trap; the
     programmatic focus ring would duplicate the visible border. */
  .modal-shell:focus {
    outline: none;
  }

  .modal-shell--small {
    width: min(340px, calc(100% - 40px));
  }

  .modal-shell--confirm {
    width: min(380px, calc(100% - 40px));
  }

  .modal-shell--medium {
    width: min(400px, calc(100% - 40px));
  }

  .modal-shell--large {
    width: min(500px, calc(100% - 40px));
  }

  .modal-shell--report {
    width: min(520px, calc(100% - 40px));
    max-height: min(560px, calc(100% - 80px));
  }

  .modal-shell--scrollable {
    max-height: calc(100% - 48px);
    overflow: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--scrollbar-color) transparent;
  }

  .modal-shell__head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 14px;
  }

  .modal-shell__icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-icon-radius, 7px);
    color: var(--selection-color);
    background: var(--card-bg);
  }

  .modal-shell__icon--danger {
    color: var(--danger-color);
  }

  .modal-shell__heading {
    flex: 1;
    min-width: 0;
  }

  .modal-shell__heading strong {
    display: block;
    color: var(--text-primary);
    font-size: 13px;
    font-weight: 560;
  }

  .modal-shell__heading p {
    margin: 2px 0 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
  }

  .modal-shell__header-actions {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex: 0 0 auto;
  }

  .modal-shell__body {
    min-width: 0;
  }

  .modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 18px;
  }

  .modal-shell__close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    flex: 0 0 auto;
    padding: 0;
    border: 0;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
    cursor: pointer;
  }

  .modal-shell__close:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }
</style>
