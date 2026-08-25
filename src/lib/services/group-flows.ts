//! Group/recycle-bin mutation flows shared with `+page.svelte`, following the
//! `IoHost` seam pattern: session staleness guards, busy tokens, toasts and
//! two-step confirmations behave exactly as before extraction — the page owns
//! every piece of dialog `$state` and hands narrow callbacks in (`closeModal`,
//! `resetSelectedGroup`, …). Do not reorder staleness checks: they decide
//! whether a toast is shown and whether a dialog closes.

import { vault } from "$lib/services/vault";
import {
  LatestOperationGuard,
  SessionViewGuard,
  type SessionViewToken,
} from "$lib/utils/session-state";

export type GroupFlowHost = {
  sessionView: SessionViewGuard;
  /** Busy token for the create-group dialog. */
  createOperations: LatestOperationGuard;
  /** Busy token for the group-icon dialog. */
  iconOperations: LatestOperationGuard;
  notify: (message: string) => void;
  /** Register a confirmation dialog; `onconfirm` runs only on approval. */
  ask: (message: string, onconfirm: () => Promise<void>) => void;
};

/** Create a group inside the create-group dialog flow.
 *
 * Branch parity with the original page code:
 * - stale view after add → return without closing the dialog or toasting;
 * - failure → toast only when the view is still current, dialog stays open;
 * - busy flag raises at flow start and resets only when the view is current
 *   AND the started operation is still the latest one (`resetBusy` runs
 *   under that exact condition). */
export async function createGroupFlow(
  host: GroupFlowHost,
  params: {
    view: SessionViewToken;
    sessionId: string;
    parentUuid: string | null;
    name: string;
    iconIndex: number | null;
    closeModal: () => void;
    setBusy: (value: boolean) => void;
    resetBusy: () => void;
  },
): Promise<void> {
  const { view, sessionId } = params;
  const operation = host.createOperations.begin();
  params.setBusy(true);
  try {
    await vault.callInSession(sessionId, () =>
      vault.addGroup({
        parentUuid: params.parentUuid,
        name: params.name,
        icon: params.iconIndex ?? undefined,
      }),
    );
    if (!host.sessionView.isCurrent(view)) return;
    params.closeModal();
    host.notify("已创建分组");
  } catch (e) {
    if (host.sessionView.isCurrent(view)) host.notify(`创建失败：${e}`);
  } finally {
    if (host.sessionView.isCurrent(view) && host.createOperations.isCurrent(operation)) {
      params.resetBusy();
    }
  }
}

/** Rename a group; toasts only for the current view. */
export async function renameGroupFlow(
  host: GroupFlowHost,
  params: { view: SessionViewToken; sessionId: string; uuid: string; name: string },
): Promise<void> {
  const { view, sessionId, uuid, name } = params;
  try {
    await vault.callInSession(sessionId, () => vault.renameGroup(uuid, name));
    if (!host.sessionView.isCurrent(view)) return;
    host.notify("已重命名分组");
  } catch (e) {
    if (host.sessionView.isCurrent(view)) host.notify(`重命名失败：${e}`);
  }
}

/** Persist group meta (notes/tags/search flag).
 *
 * Returns `true` only when the write succeeded AND the view is current AND
 * `stillTarget()` still holds — mirroring the original dialog contract where
 * a stale or re-targeted dialog reports "not saved" without closing. */
export async function saveGroupMetaFlow(
  host: GroupFlowHost,
  params: {
    view: SessionViewToken;
    sessionId: string;
    uuid: string;
    meta: { notes?: string; tags?: string; enableSearching?: boolean };
    stillTarget: () => boolean;
  },
): Promise<boolean> {
  const { view, sessionId, uuid, meta } = params;
  try {
    await vault.callInSession(sessionId, () => vault.updateGroupMeta(uuid, meta));
    if (!host.sessionView.isCurrent(view) || !params.stillTarget()) return false;
    host.notify("已保存分组属性");
    return true;
  } catch (e) {
    if (host.sessionView.isCurrent(view)) host.notify(`保存分组属性失败：${e}`);
    return false;
  }
}

/** Apply a picked group icon inside the icon-dialog flow; same branch parity
 *  as {@link createGroupFlow} (close + toast on success only). */
export async function changeGroupIconFlow(
  host: GroupFlowHost,
  params: {
    view: SessionViewToken;
    sessionId: string;
    uuid: string;
    pick: number | null;
    closeModal: () => void;
    setBusy: (value: boolean) => void;
    resetBusy: () => void;
  },
): Promise<void> {
  const { view, sessionId } = params;
  const operation = host.iconOperations.begin();
  params.setBusy(true);
  try {
    await vault.callInSession(sessionId, () => vault.setGroupIcon(params.uuid, params.pick));
    if (!host.sessionView.isCurrent(view)) return;
    params.closeModal();
    host.notify("已更新分组图标");
  } catch (e) {
    if (host.sessionView.isCurrent(view)) host.notify(`更新图标失败：${e}`);
  } finally {
    if (host.sessionView.isCurrent(view) && host.iconOperations.isCurrent(operation)) {
      params.resetBusy();
    }
  }
}

/** Register the delete-group confirmation. `inBin` picks the permanent vs.
 *  recycle-bin wording; approval clears the selected group when it was the
 *  deleted one (`resetSelectedGroup` encapsulates that comparison). */
export function confirmDeleteGroupFlow(
  host: GroupFlowHost,
  params: {
    view: SessionViewToken;
    sessionId: string;
    uuid: string;
    inBin: boolean;
    resetSelectedGroup: () => void;
  },
): void {
  const { view, sessionId, uuid, inBin } = params;
  host.ask(
    inBin ? "永久删除该分组及其全部内容？此操作无法撤销。" : "删除该分组？其下条目将移动到回收站。",
    async () => {
      if (!host.sessionView.isCurrent(view)) return;
      try {
        await vault.callInSession(sessionId, () => vault.deleteGroup(uuid));
        if (!host.sessionView.isCurrent(view)) return;
        params.resetSelectedGroup();
        host.notify(inBin ? "已永久删除分组" : "已移入回收站");
      } catch (e) {
        if (host.sessionView.isCurrent(view)) host.notify(`删除失败：${e}`);
      }
    },
  );
}

/** Register the empty-recycle-bin confirmation. */
export function confirmEmptyRecycleBinFlow(
  host: GroupFlowHost,
  params: { view: SessionViewToken; sessionId: string },
): void {
  const { view, sessionId } = params;
  host.ask("清空回收站？其中的条目和分组将被永久删除，此操作无法撤销。", async () => {
    if (!host.sessionView.isCurrent(view)) return;
    try {
      await vault.callInSession(sessionId, () => vault.emptyRecycleBin());
      if (!host.sessionView.isCurrent(view)) return;
      host.notify("已清空回收站");
    } catch (e) {
      if (host.sessionView.isCurrent(view)) host.notify(`清空失败：${e}`);
    }
  });
}

/** Restore a group from the recycle bin back to the root. */
export async function restoreGroupFlow(
  host: GroupFlowHost,
  params: { view: SessionViewToken; sessionId: string; uuid: string },
): Promise<void> {
  const { view, sessionId, uuid } = params;
  try {
    await vault.callInSession(sessionId, () => vault.restoreGroup(uuid));
    if (!host.sessionView.isCurrent(view)) return;
    host.notify("已恢复分组");
  } catch (e) {
    if (host.sessionView.isCurrent(view)) host.notify(`恢复失败：${e}`);
  }
}

/** Move entries into a group one by one (original semantics: sequential calls,
 *  a mid-run stale view stops further moves without a toast). */
export async function moveEntriesFlow(
  host: GroupFlowHost,
  params: { view: SessionViewToken; sessionId: string; groupUuid: string; uuids: string[] },
): Promise<void> {
  const { view, sessionId, groupUuid, uuids } = params;
  try {
    for (const uuid of uuids) {
      await vault.callInSession(sessionId, () => vault.moveEntry(uuid, groupUuid));
    }
    if (!host.sessionView.isCurrent(view)) return;
    host.notify(`已移动 ${uuids.length} 个条目`);
  } catch (e) {
    if (host.sessionView.isCurrent(view)) host.notify(`移动失败：${e}`);
  }
}

/** Persist one group's expanded flag (no success toast on purpose). */
export async function setGroupExpandedFlow(
  host: GroupFlowHost,
  params: { view: SessionViewToken; sessionId: string; uuid: string; expanded: boolean },
): Promise<void> {
  const { view, sessionId, uuid, expanded } = params;
  try {
    await vault.callInSession(sessionId, () => vault.setGroupExpanded(uuid, expanded));
  } catch (error) {
    if (host.sessionView.isCurrent(view)) host.notify(`展开分组失败：${error}`);
  }
}

/** Persist expanded flags for several groups at once (no success toast). */
export async function setGroupsExpandedFlow(
  host: GroupFlowHost,
  params: { view: SessionViewToken; sessionId: string; uuids: string[]; expanded: boolean },
): Promise<void> {
  const { view, sessionId, uuids, expanded } = params;
  try {
    await vault.callInSession(sessionId, () => vault.setGroupsExpanded(uuids, expanded));
  } catch (error) {
    if (host.sessionView.isCurrent(view)) host.notify(`展开分组失败：${error}`);
  }
}
