/**
 * Touch long-press tracker for the entry list.
 *
 * Pointer events bubble, so a `pointerdown` on an entry row also reaches the
 * list's blank-area handler. A tracker that only remembers "the" pending
 * timer therefore loses the first registration: the second one overwrites it
 * and the orphaned timer still fires after the finger lifted, opening the
 * context menu on a plain tap. This tracker keeps a single armed press per
 * gesture and lets the first registration win — the innermost element under
 * the finger owns the gesture.
 */

/** A pointer position in client coordinates. */
export interface PressPoint {
  x: number;
  y: number;
}

export interface LongPressOptions {
  /** Milliseconds a touch must be held before the long press fires. */
  threshold?: number;
  /** Pixels the pointer may travel before the press is abandoned. */
  moveTolerance?: number;
}

export interface LongPressTracker {
  /**
   * Begin a gesture. A mouse never long-presses, so it only invalidates a
   * stale click suppression. Only the first call of a gesture arms the press:
   * later calls (the same event bubbling to an ancestor) are ignored, and the
   * innermost element under the finger keeps the press.
   */
  start(pointerType: string, point: PressPoint, onFire: (point: PressPoint) => void): void;
  /** Feed a pointer move; the press is abandoned past the move tolerance. */
  move(point: PressPoint): void;
  /** End the press without firing (pointer up, pointer cancel). */
  cancel(): void;
  /**
   * Whether the trailing `click` of the current gesture must be swallowed
   * because a long press already opened a menu. Consumes the flag.
   */
  consumeSuppressedClick(): boolean;
}

export function createLongPressTracker(options: LongPressOptions = {}): LongPressTracker {
  const threshold = options.threshold ?? 500;
  const moveTolerance = options.moveTolerance ?? 12;

  let timer: ReturnType<typeof setTimeout> | undefined;
  let origin: PressPoint | null = null;
  /** A long press fired during the current gesture, so the click that
   *  follows it must not also select the row the menu belongs to. */
  let suppressedClick = false;

  function clearTimer(): void {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
  }

  function cancel(): void {
    clearTimer();
    origin = null;
  }

  function start(
    pointerType: string,
    point: PressPoint,
    onFire: (point: PressPoint) => void,
  ): void {
    // A new gesture invalidates a suppression left over from an earlier one,
    // so a stale flag can never swallow an unrelated later tap.
    suppressedClick = false;
    if (pointerType === "mouse") return;
    // Already armed: this is the same gesture bubbling to an ancestor.
    if (timer !== undefined) return;
    origin = point;
    const fired = point;
    timer = setTimeout(() => {
      timer = undefined;
      origin = null;
      suppressedClick = true;
      onFire(fired);
    }, threshold);
  }

  function move(point: PressPoint): void {
    if (!origin) return;
    if (Math.hypot(point.x - origin.x, point.y - origin.y) > moveTolerance) cancel();
  }

  function consumeSuppressedClick(): boolean {
    if (!suppressedClick) return false;
    suppressedClick = false;
    return true;
  }

  return { start, move, cancel, consumeSuppressedClick };
}
