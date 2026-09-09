/** Display-state resolution for the HIBP breach-check dialog.
 *
 * A cancelled run must never render the clean bill of health: even with zero
 * collected findings the check is incomplete, so cancellation gets its own
 * terminal states (`cancelled-clean` / `cancelled-hits`).
 */

export interface HibpDialogInput {
  started: boolean;
  running: boolean;
  failed: boolean;
  findingCount: number;
  cancelled: boolean;
}

export type HibpDialogState =
  "idle" | "running" | "error" | "clean" | "cancelled-clean" | "hits" | "cancelled-hits";

export function hibpResultState(input: HibpDialogInput): HibpDialogState {
  if (!input.started) return "idle";
  if (input.running) return "running";
  if (input.failed) return "error";
  if (input.findingCount === 0) return input.cancelled ? "cancelled-clean" : "clean";
  return input.cancelled ? "cancelled-hits" : "hits";
}
