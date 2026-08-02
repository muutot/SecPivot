<script lang="ts" generics="T extends string | number">
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    value: T;
    options: { value: T; label: string }[];
    onchange: (value: T) => void;
    id?: string;
    ariaLabel?: string;
    className?: string;
    disabled?: boolean;
  }

  let {
    value,
    options,
    onchange,
    id,
    ariaLabel,
    className = "",
    disabled = false,
  }: Props = $props();

  let open = $state(false);
  let highlight = $state(-1);
  let triggerEl: HTMLButtonElement | null = $state(null);
  let listEl: HTMLDivElement | null = $state(null);
  let pos = $state({ top: 0, left: 0, width: 0 });

  const selectedLabel = $derived(options.find((o) => o.value === value)?.label ?? "");

  function positionMenu(): void {
    if (!triggerEl) return;
    const r = triggerEl.getBoundingClientRect();
    const itemH = 28;
    const pad = 6;
    const listH = Math.min(options.length, 8) * itemH + pad * 2;
    let top = r.bottom + 4;
    if (top + listH > window.innerHeight - 8) {
      top = Math.max(8, r.top - listH - 4);
    }
    pos = { top, left: r.left, width: r.width };
  }

  function openMenu(): void {
    if (disabled) return;
    open = true;
    highlight = Math.max(
      0,
      options.findIndex((o) => o.value === value),
    );
    positionMenu();
    requestAnimationFrame(() => listEl?.focus());
  }

  function closeMenu(): void {
    open = false;
    highlight = -1;
  }

  function choose(index: number): void {
    if (index < 0 || index >= options.length) return;
    closeMenu();
    onchange(options[index].value);
  }

  function onTriggerKeydown(e: KeyboardEvent): void {
    if (e.key === "ArrowDown" || e.key === "ArrowUp" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      if (!open) openMenu();
      else if (e.key === "ArrowDown" || e.key === "ArrowUp") moveHighlight(e.key);
    }
  }

  function moveHighlight(dir: "ArrowDown" | "ArrowUp"): void {
    if (options.length === 0) return;
    const step = dir === "ArrowDown" ? 1 : -1;
    highlight = (highlight + step + options.length) % options.length;
    const active = listEl?.querySelector<HTMLElement>(`[data-index="${highlight}"]`);
    active?.scrollIntoView({ block: "nearest" });
  }

  function onListKeydown(e: KeyboardEvent): void {
    if (e.key === "Escape") {
      e.preventDefault();
      closeMenu();
      triggerEl?.focus();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      moveHighlight(e.key);
    } else if (e.key === "Home") {
      e.preventDefault();
      highlight = 0;
    } else if (e.key === "End") {
      e.preventDefault();
      highlight = options.length - 1;
    } else if (e.key === "Enter") {
      e.preventDefault();
      choose(highlight);
    } else if (e.key === "Tab") {
      closeMenu();
    }
  }

  $effect(() => {
    if (!open) return;
    const onDocDown = (e: MouseEvent): void => {
      const t = e.target as Node;
      if (!triggerEl?.contains(t) && !listEl?.contains(t)) closeMenu();
    };
    const onResize = (): void => closeMenu();
    const onScroll = (): void => closeMenu();
    document.addEventListener("mousedown", onDocDown);
    window.addEventListener("resize", onResize);
    window.addEventListener("scroll", onScroll, true);
    return () => {
      document.removeEventListener("mousedown", onDocDown);
      window.removeEventListener("resize", onResize);
      window.removeEventListener("scroll", onScroll, true);
    };
  });
</script>

<div class="select-root {className}">
  <button
    {id}
    class="select-trigger"
    type="button"
    bind:this={triggerEl}
    aria-label={ariaLabel}
    aria-haspopup="listbox"
    aria-expanded={open}
    aria-controls={open ? `${id ?? "select"}-list` : undefined}
    {disabled}
    onclick={() => (open ? closeMenu() : openMenu())}
    onkeydown={onTriggerKeydown}
  >
    <span class="select-value">{selectedLabel || "\u00A0"}</span>
    <span class="select-chevron">
      <AppIcon name="chevron-down" size={13} />
    </span>
  </button>

  {#if open}
    <div
      id={`${id ?? "select"}-list`}
      class="select-list"
      role="listbox"
      tabindex="-1"
      bind:this={listEl}
      style="top:{pos.top}px; left:{pos.left}px; width:{pos.width}px;"
      onkeydown={onListKeydown}
    >
      {#each options as option, i (option.value)}
        <button
          type="button"
          role="option"
          tabindex="-1"
          data-index={i}
          aria-selected={option.value === value}
          class="select-option"
          class:selected={option.value === value}
          class:highlighted={i === highlight}
          onmousemove={() => (highlight = i)}
          onclick={() => choose(i)}
        >
          <span class="select-option-label">{option.label}</span>
          {#if option.value === value}
            <AppIcon name="check" size={12} />
          {/if}
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .select-root {
    position: relative;
    display: inline-flex;
    flex: 0 0 auto;
    min-width: 96px;
  }

  .select-trigger {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    cursor: pointer;
  }

  .select-trigger:hover:not(:disabled) {
    border-color: var(--text-faint);
  }

  .select-trigger:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .select-trigger:disabled {
    cursor: not-allowed;
    opacity: 0.5;
  }

  .select-value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .select-trigger :global(.select-chevron) {
    color: var(--text-faint);
    transition: transform 120ms ease;
  }

  .select-trigger[aria-expanded="true"] :global(.select-chevron) {
    transform: rotate(180deg);
  }

  .select-list {
    position: fixed;
    z-index: 9999;
    display: flex;
    flex-direction: column;
    gap: 1px;
    max-height: min(calc(28px * 8 + 12px), 60vh);
    overflow-y: auto;
    padding: 6px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--surface-bg);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
    animation: select-pop 120ms ease;
  }

  .select-option {
    display: inline-flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    height: 28px;
    padding: 0 8px;
    border: none;
    border-radius: 4px;
    color: var(--text-secondary);
    background: transparent;
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    text-align: left;
    cursor: pointer;
  }

  .select-option:hover,
  .select-option.highlighted {
    background: var(--hover-bg);
    color: var(--text-primary);
  }

  .select-option.selected {
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 14%, transparent);
  }

  .select-option :global(svg) {
    color: var(--selection-color);
    flex: 0 0 auto;
  }

  .select-option-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (prefers-reduced-motion: reduce) {
    .select-list {
      animation: none;
    }

    .select-trigger :global(.select-chevron) {
      transition: none;
    }
  }

  @keyframes select-pop {
    from {
      opacity: 0;
      transform: translateY(-2px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
