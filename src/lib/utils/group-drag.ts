import type { VaultGroup } from "$lib/types/vault";
import { buildVaultTreeIndex, orderedChildren } from "$lib/utils/tree";

export const GROUP_DRAG_MIME = "application/x-secpivot-group";
export type GroupPlacement = "before" | "inside" | "after";
export interface GroupMove {
  uuid: string;
  parentUuid: string;
  beforeUuid: string | null;
}

/** Validate first; callers may mutate only after this complete plan succeeds. */
export function validateGroupMove(root: VaultGroup, move: GroupMove): boolean {
  const index = buildVaultTreeIndex(root);
  const source = index.groupByUuid.get(move.uuid);
  const parent = index.groupByUuid.get(move.parentUuid);
  if (!source || source === root || !parent || source === parent) return false;
  const inBin = (uuid: string): boolean => {
    let cursor: string | undefined = uuid;
    while (cursor) {
      if (index.groupByUuid.get(cursor)?.isRecycleBin) return true;
      cursor = index.groupByUuid.get(cursor)?.parentUuid ?? undefined;
    }
    return false;
  };
  if (inBin(source.uuid) || inBin(parent.uuid)) return false;
  let cursor: string | undefined = parent.uuid;
  while (cursor) {
    if (cursor === source.uuid) return false;
    cursor = index.groupByUuid.get(cursor)?.parentUuid ?? undefined;
  }
  return (
    move.beforeUuid === null ||
    (move.beforeUuid !== source.uuid && parent.children.some((g) => g.uuid === move.beforeUuid))
  );
}

export function planGroupDrop(
  root: VaultGroup,
  uuid: string,
  targetUuid: string,
  placement: GroupPlacement,
): GroupMove | null {
  const index = buildVaultTreeIndex(root);
  const target = index.groupByUuid.get(targetUuid);
  if (!target || targetUuid === uuid || target.isRecycleBin) return null;
  let parent = target;
  let beforeUuid: string | null = null;
  if (placement !== "inside") {
    const parentId = target.parentUuid;
    if (!parentId) return null;
    parent = index.groupByUuid.get(parentId)!;
    const siblings = orderedChildren(parent).filter((g) => g.uuid !== uuid);
    const position = siblings.findIndex((g) => g.uuid === targetUuid);
    beforeUuid = placement === "before" ? targetUuid : (siblings[position + 1]?.uuid ?? null);
  }
  const move = { uuid, parentUuid: parent.uuid, beforeUuid };
  return validateGroupMove(root, move) ? move : null;
}

export function groupDropPlacement(event: DragEvent): GroupPlacement {
  const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
  const ratio = (event.clientY - rect.top) / rect.height;
  return ratio < 0.25 ? "before" : ratio > 0.75 ? "after" : "inside";
}

/** A local owner is required because dragover cannot read protected transfer data. */
export function createGroupDrag(host: {
  root: () => VaultGroup;
  sessionId: () => string | null;
  readOnly: () => boolean;
  target: (target: { uuid: string; placement: GroupPlacement } | null) => void;
  move: (move: GroupMove) => void;
}) {
  let owner: { uuid: string; sessionId: string; root: VaultGroup } | null = null;
  const end = () => {
    owner = null;
    host.target(null);
  };
  const current = () =>
    owner && !host.readOnly() && owner.sessionId === host.sessionId() && owner.root === host.root();
  return {
    end,
    leave() {
      host.target(null);
    },
    start(uuid: string, event: DragEvent) {
      end();
      const sessionId = host.sessionId();
      if (
        !sessionId ||
        host.readOnly() ||
        !event.dataTransfer ||
        !validateGroupMove(host.root(), { uuid, parentUuid: host.root().uuid, beforeUuid: null })
      ) {
        event.preventDefault();
        return;
      }
      owner = { uuid, sessionId, root: host.root() };
      event.dataTransfer.setData(GROUP_DRAG_MIME, JSON.stringify({ uuid, sessionId }));
      event.dataTransfer.effectAllowed = "move";
    },
    over(uuid: string, placement: GroupPlacement, event: DragEvent) {
      host.target(null);
      if (
        !current() ||
        !event.dataTransfer?.types.includes(GROUP_DRAG_MIME) ||
        !planGroupDrop(host.root(), owner!.uuid, uuid, placement)
      )
        return;
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
      host.target({ uuid, placement });
    },
    drop(uuid: string, placement: GroupPlacement, event: DragEvent) {
      try {
        if (!current() || !event.dataTransfer) return;
        const payload = JSON.parse(event.dataTransfer.getData(GROUP_DRAG_MIME));
        if (payload.uuid !== owner!.uuid || payload.sessionId !== owner!.sessionId) return;
        const move = planGroupDrop(host.root(), payload.uuid, uuid, placement);
        if (!move) return;
        event.preventDefault();
        host.move(move);
      } catch {
        /* Ignore foreign or damaged drag data. */
      } finally {
        end();
      }
    },
  };
}

export type GroupDrag = ReturnType<typeof createGroupDrag>;
