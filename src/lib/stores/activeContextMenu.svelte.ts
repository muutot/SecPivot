import { writable } from "svelte/store";

/**
 * Single source of truth for which context menu is currently open.
 *
 * SecPivot shows several context menus (entry, blank, toolbar, column on the
 * page, plus the per-group menu inside GroupNode) but at most one may be
 * visible at any moment. Each owner registers a unique id when it opens and
 * clears it when it closes. Opening an owner flips the active id, so every
 * other owner's reactive guard closes its own menu — guaranteeing a single
 * visible context menu across the whole app shell.
 *
 * Backed by a plain `svelte/store` (not runes) so the coordination contract is
 * unit-testable in `node --test` without a Svelte compile step.
 */
export const activeContextMenu = writable<string | null>(null);

/** Mark `owner` as the single open context menu (closing any previous one). */
export function openContextMenu(owner: string): void {
  activeContextMenu.set(owner);
}

/**
 * Clear the active context menu. When `owner` is given the clear is scoped to
 * that owner, so a late close from one menu never stomps a freshly opened one.
 */
export function closeContextMenu(owner?: string): void {
  if (owner === undefined) {
    activeContextMenu.set(null);
    return;
  }
  activeContextMenu.update((current) => (current === owner ? null : current));
}
