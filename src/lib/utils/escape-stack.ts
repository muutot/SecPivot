/**
 * Tracks open modal surfaces so only the topmost one reacts to Escape.
 *
 * Nested modals (e.g. a TOTP picker inside the entry editor) each register
 * their own `window` keydown handler; one Escape press would otherwise fire
 * every handler at once and close the whole stack, destroying unsaved edits
 * beneath the topmost dialog.
 */
const stack: symbol[] = [];

export interface EscapeScope {
  /** Whether this scope is currently the topmost modal surface. */
  isTop: () => boolean;
  /** Remove the scope (idempotent); call when the modal unmounts. */
  release: () => void;
}

/** Register the calling modal as the current top of the Escape stack. */
export function acquireEscapeScope(): EscapeScope {
  const id = Symbol("modal-escape");
  stack.push(id);
  return {
    isTop: () => stack[stack.length - 1] === id,
    release: () => {
      const index = stack.lastIndexOf(id);
      if (index >= 0) stack.splice(index, 1);
    },
  };
}

/** Whether any modal surface is currently registered. */
export function isAnyModalOpen(): boolean {
  return stack.length > 0;
}
