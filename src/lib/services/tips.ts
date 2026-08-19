import { writable } from "svelte/store";

export type TipKind = "success" | "error";

export interface Tip {
  id: number;
  message: string;
  kind: TipKind;
}

export const tips = writable<Tip[]>([]);

let nextId = 1;
const timers = new Map<number, ReturnType<typeof setTimeout>>();

const TIP_DURATION = 2200;

/** Show a transient tip in the global bottom-center tips area. */
export function showTip(message: string, kind: TipKind = "success"): void {
  const id = nextId++;
  tips.update((list) => [...list.slice(-2), { id, message, kind }]);
  timers.set(
    id,
    setTimeout(() => dismissTip(id), TIP_DURATION),
  );
}

export function dismissTip(id: number): void {
  const timer = timers.get(id);
  if (timer) clearTimeout(timer);
  timers.delete(id);
  tips.update((list) => list.filter((tip) => tip.id !== id));
}
