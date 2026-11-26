<script lang="ts">
  import { onMount } from "svelte";
  import { get } from "svelte/store";
  import { vault } from "$lib/services/vault";
  import { appSettings, isTauriRuntime } from "$lib/services/settings";
  import { FileReloadController, isFileReloadBlocked } from "$lib/services/file-reload";
  import { isAnyModalOpen } from "$lib/utils/escape-stack";
  import { showTip } from "$lib/services/tips";
  import { t } from "$lib/i18n";
  onMount(() => {
    let dragging = false;
    const startDrag = () => {
      dragging = true;
    };
    const endDrag = () => {
      dragging = false;
    };
    const controller = new FileReloadController({
      enabled: () => isTauriRuntime() && get(appSettings).general.autoReloadChangedFiles,
      canApply: () =>
        document.visibilityState === "visible" &&
        !isAnyModalOpen() &&
        !dragging &&
        !isFileReloadBlocked(),
      prepare: async (id) => {
        await appSettings.flush();
        return vault.callInSession(id, () => vault.prepareChangedFile());
      },
      apply: (id, token) => vault.callInSession(id, () => vault.applyChangedFile(token)),
      result: (status) =>
        showTip(
          t(
            get(appSettings).general.language,
            status === "merged" ? "reload.merged" : "reload.reloaded",
          ),
        ),
      error: (error) => {
        if (String(error).includes("FILE_RELOAD_CHANGED:")) return;
        const key = String(error).includes("FILE_RELOAD_CONFLICT:")
          ? "reload.conflict"
          : String(error).includes("FILE_RELOAD_INVALID:")
            ? "reload.invalid"
            : "reload.unavailable";
        showTip(t(get(appSettings).general.language, key), "error");
      },
    });
    const activate = () => {
      const id = vault.getActiveSessionId();
      const epoch = id ? vault.getSessionEpoch(id) : undefined;
      if (controller.activate(vault.get() && id && epoch ? { sessionId: id, epoch } : null))
        void controller.check();
    };
    const unsubscribe = vault.subscribe(activate);
    const unsubscribeId = vault.activeId.subscribe(activate);
    const check = () => void controller.check();
    window.addEventListener("focus", check);
    document.addEventListener("dragstart", startDrag, true);
    document.addEventListener("dragend", endDrag, true);
    document.addEventListener("drop", endDrag, true);
    document.addEventListener("visibilitychange", check);
    const timer = setInterval(check, 30000);
    check();
    return () => {
      clearInterval(timer);
      window.removeEventListener("focus", check);
      document.removeEventListener("dragstart", startDrag, true);
      document.removeEventListener("dragend", endDrag, true);
      document.removeEventListener("drop", endDrag, true);
      document.removeEventListener("visibilitychange", check);
      unsubscribe();
      unsubscribeId();
      controller.dispose();
    };
  });
</script>
