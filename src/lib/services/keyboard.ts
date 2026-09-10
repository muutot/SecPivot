import type { IconName } from "$lib/components/AppIcon.svelte";
import type { I18nKey } from "$lib/i18n";

/** One configurable app-window shortcut action. The panel renders these and
 *  `+page.svelte` dispatches them; both sides must agree on the ids.
 *  `label`/`description` are i18n keys so the panel renders the UI locale. */
export interface KeyboardAction {
  id: string;
  label: I18nKey;
  description: I18nKey;
  icon: IconName;
  /** Accelerator assigned when the action has no stored binding. */
  default: string;
}

export const KEYBOARD_ACTIONS: KeyboardAction[] = [
  {
    id: "save",
    label: "shortcuts.save.label",
    description: "shortcuts.save.description",
    icon: "save",
    default: "Ctrl+S",
  },
  {
    id: "lock",
    label: "shortcuts.lock.label",
    description: "shortcuts.lock.description",
    icon: "lock",
    default: "Ctrl+L",
  },
  {
    id: "edit",
    label: "shortcuts.edit.label",
    description: "shortcuts.edit.description",
    icon: "edit",
    default: "Ctrl+E",
  },
  {
    id: "copy-password",
    label: "shortcuts.copy-password.label",
    description: "shortcuts.copy-password.description",
    icon: "copy",
    default: "Ctrl+Shift+C",
  },
  {
    id: "new-entry",
    label: "shortcuts.new-entry.label",
    description: "shortcuts.new-entry.description",
    icon: "plus",
    default: "Ctrl+N",
  },
  {
    id: "focus-search",
    label: "shortcuts.focus-search.label",
    description: "shortcuts.focus-search.description",
    icon: "search",
    default: "Ctrl+K",
  },
  {
    id: "locate-in-tree",
    label: "shortcuts.locate-in-tree.label",
    description: "shortcuts.locate-in-tree.description",
    icon: "folder",
    default: "Ctrl+G",
  },
];

/** Stored bindings merged with action defaults, so unrecorded actions still
 *  dispatch their default accelerator. */
export function effectiveShortcuts(shortcuts: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const action of KEYBOARD_ACTIONS) {
    out[action.id] = shortcuts[action.id] || action.default;
  }
  return out;
}

/** True when the event's pressed modifiers match `combo` ("Ctrl+Shift+C").
 *  Modifier order in the combo is irrelevant; the last non-modifier token is
 *  the key. Single-character keys compare case-insensitively ("Space" is the
 *  canonical name for `" "`). */
export function matchesShortcut(event: KeyboardEvent, combo: string): boolean {
  const mods: [string, boolean][] = [
    ["Ctrl", event.ctrlKey],
    ["Alt", event.altKey],
    ["Shift", event.shiftKey],
    ["Meta", event.metaKey],
  ];
  const parts = combo.split("+").map((p) => p.trim());
  let keyPart = "";
  for (const part of parts) {
    if (part === "Ctrl" || part === "Alt" || part === "Shift" || part === "Meta") continue;
    keyPart = part;
  }
  for (const [name, pressed] of mods) {
    if (parts.includes(name) !== pressed) return false;
  }
  if (!keyPart) return false;
  const eventKey =
    event.key === " " ? "Space" : event.key.length === 1 ? event.key.toUpperCase() : event.key;
  return eventKey === keyPart;
}

/**
 * Dispatch the first matching shortcut to its handler. Iteration order follows
 * `bindings` insertion order; a match consumes the event (`preventDefault`)
 * and stops the search, so later bindings with identical combos never fire.
 * Empty/blank combos are skipped (an action may be unbound).
 */
export function dispatchShortcut(
  event: KeyboardEvent,
  bindings: Record<string, string>,
  handlers: Record<string, () => void>,
): void {
  for (const [actionId, combo] of Object.entries(bindings)) {
    if (!combo || !matchesShortcut(event, combo)) continue;
    event.preventDefault();
    handlers[actionId]?.();
    return;
  }
}
