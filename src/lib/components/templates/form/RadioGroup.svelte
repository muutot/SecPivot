<script lang="ts" generics="T extends string">
  interface Props {
    value: T;
    options: readonly { value: T; label: string }[];
    ariaLabel: string;
    disabled?: boolean;
    size?: "control" | "compact";
    stretch?: boolean;
    onchange?: (value: T) => void;
  }

  let {
    value = $bindable(),
    options,
    ariaLabel,
    disabled = false,
    size = "control",
    stretch = true,
    onchange,
  }: Props = $props();
  const selectedIndex = $derived(options.findIndex((option) => option.value === value));

  function select(next: T): void {
    value = next;
    onchange?.(next);
  }

  function handleKeydown(event: KeyboardEvent, index: number): void {
    if (
      event.defaultPrevented ||
      disabled ||
      options.length === 0 ||
      event.altKey ||
      event.ctrlKey ||
      event.metaKey
    )
      return;
    let next: number;
    switch (event.key) {
      case "ArrowRight":
      case "ArrowDown":
        next = (index + 1) % options.length;
        break;
      case "ArrowLeft":
      case "ArrowUp":
        next = (index + options.length - 1) % options.length;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = options.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
    select(options[next].value);
    (event.currentTarget as HTMLElement).parentElement
      ?.querySelectorAll<HTMLButtonElement>('[role="radio"]')
      [next]?.focus();
  }
</script>

<div
  class="radio-group"
  class:compact={size === "compact"}
  class:content={!stretch}
  role="radiogroup"
  aria-label={ariaLabel}
  aria-orientation="horizontal"
>
  {#each options as option, index (option.value)}
    <button
      type="button"
      class="radio-choice"
      class:active={value === option.value}
      role="radio"
      aria-checked={value === option.value}
      tabindex={index === (selectedIndex < 0 ? 0 : selectedIndex) ? 0 : -1}
      {disabled}
      onclick={() => select(option.value)}
      onkeydown={(event) => handleKeydown(event, index)}
    >
      {option.label}
    </button>
  {/each}
</div>

<style>
  .radio-group {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .radio-choice {
    flex: 1 0 auto;
    height: 30px;
    padding: 0 8px;
    white-space: nowrap;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--input-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .radio-choice:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .compact .radio-choice {
    height: 28px;
  }

  .content .radio-choice {
    flex: 0 0 auto;
  }

  .radio-choice.active {
    color: var(--selection-color);
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .radio-choice:disabled {
    cursor: default;
    opacity: 0.6;
  }

  @media (forced-colors: none) {
    :global(:root[data-theme="light"]) .radio-choice.active {
      color: var(--card-bg);
      border-color: var(--selection-color);
      background: var(--selection-color);
      box-shadow: 0 2px 5px color-mix(in srgb, var(--selection-color) 18%, transparent);
    }
  }

  @media (max-width: 360px) {
    .radio-choice {
      padding: 0 6px;
    }
  }
</style>
