/**
 * Replace an owned reference and dispose the previous value exactly once.
 * Assign the returned value back to the caller's reactive state.
 */
export function replaceDisposable<T>(
  current: T | null,
  replacement: T | null,
  dispose: (value: T) => unknown,
): T | null {
  if (current !== null && current !== replacement) dispose(current);
  return replacement;
}

/**
 * Forget a reference only when the completed operation still owns it and the
 * external consumer reports that the resource was consumed successfully.
 */
export function settleDisposable<T>(current: T | null, owned: T, consumed: boolean): T | null {
  return consumed && current === owned ? null : current;
}
