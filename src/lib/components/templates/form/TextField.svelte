<script lang="ts">
  import type { HTMLInputAttributes } from "svelte/elements";

  /** Text field template (`.text-input` / `.settings-input`). Owns every
   *  control style; consumers compose it and pass data + limited variants. */
  interface Props {
    value?: string;
    type?: string;
    placeholder?: string;
    /** Monospace text (seeds, sequences, URLs, code). */
    mono?: boolean;
    /** Multi-line textarea instead of a single-line input. */
    multiline?: boolean;
    rows?: number;
    maxlength?: number;
    disabled?: boolean;
    readonly?: boolean;
    spellcheck?: boolean;
    id?: string;
    autocomplete?: HTMLInputAttributes["autocomplete"];
    /** Hide number spinners (numeric fields). */
    numeric?: boolean;
    /** Red invalid ring (inline validation). */
    invalid?: boolean;
    onkeydown?: (event: KeyboardEvent) => void;
    oninput?: (event: Event) => void;
  }

  let {
    value = $bindable(""),
    type = "text",
    placeholder = undefined,
    mono = false,
    multiline = false,
    rows,
    maxlength = undefined,
    disabled = false,
    readonly = false,
    spellcheck = undefined,
    id = undefined,
    autocomplete = undefined,
    numeric = false,
    invalid = false,
    onkeydown,
    oninput,
  }: Props = $props();
</script>

{#if multiline}
  <textarea
    {id}
    class="field mono"
    bind:value
    {placeholder}
    {rows}
    {maxlength}
    {disabled}
    {readonly}
    {spellcheck}
    {onkeydown}
    {oninput}
  ></textarea>
{:else}
  <input
    {id}
    class="field"
    class:mono
    class:numeric
    class:invalid
    {type}
    bind:value
    {placeholder}
    {maxlength}
    {disabled}
    {readonly}
    {spellcheck}
    {autocomplete}
    {onkeydown}
    {oninput}
  />
{/if}

<style>
  .field {
    width: 100%;
    box-sizing: border-box;
    height: 32px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-primary);
    background: var(--input-bg);
    font-size: 12px;
  }

  .field.mono {
    font-family: var(--font-mono);
  }

  .field:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  textarea.field {
    height: auto;
    padding: 8px 10px;
    line-height: 1.5;
    resize: vertical;
  }

  /* Numeric fields read as textfields; spinners are never shown. */
  .field.numeric {
    -moz-appearance: textfield;
    appearance: textfield;
  }

  .field.numeric::-webkit-inner-spin-button,
  .field.numeric::-webkit-outer-spin-button {
    -webkit-appearance: none;
    margin: 0;
  }

  .field.invalid {
    border-color: color-mix(in srgb, var(--danger-color) 60%, var(--border-color));
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--danger-color) 25%, transparent);
  }
</style>
