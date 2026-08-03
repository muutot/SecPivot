<script lang="ts">
  import { onMount } from "svelte";
  import "../app.css";
  import { appSettings } from "$lib/services/settings";
  import { applySettingsToDocument, syncCompactShellClass } from "$lib/services/settings-bootstrap";
  import { installAutoLock, installFocusLock, lockVault } from "$lib/services/security";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import BridgeApprovalPrompt from "$lib/components/BridgeApprovalPrompt.svelte";
  import RpcSideChannelPrompt from "$lib/components/RpcSideChannelPrompt.svelte";

  let { children } = $props();

  /** The TCATO overlay window loads this SPA with a `#/tcato` hash. It must
   * not install idle/focus locks: blurring the overlay while typing into the
   * target app would lock the vault out from under the injection. */
  const isTcatoOverlay =
    typeof window !== "undefined" && window.location.hash.startsWith("#/tcato");

  if (typeof document !== "undefined") {
    applySettingsToDocument();
  }

  onMount(() => {
    const unsubscribe = appSettings.subscribe((s) => {
      applySettingsToDocument();
      syncCompactShellClass(s.general.compactMode);
    });
    // Auto-lock lives here (not in +page) so it survives navigation to
    // /settings; focus-lock already lived here.
    const stopFocusLock = isTcatoOverlay ? () => {} : installFocusLock();
    const stopAutoLock = isTcatoOverlay ? () => {} : installAutoLock();
    // System tray "锁定数据库" action.
    let stopTrayLock: UnlistenFn | undefined;
    if (!isTcatoOverlay && typeof window !== "undefined" && "__TAURI_INTERNALS__" in window) {
      void listen("tray-lock", () => void lockVault()).then((fn) => {
        stopTrayLock = fn;
      });
    }
    return () => {
      unsubscribe();
      stopFocusLock();
      stopAutoLock();
      stopTrayLock?.();
    };
  });
</script>

{@render children()}

{#if !isTcatoOverlay}
  <BridgeApprovalPrompt />
  <RpcSideChannelPrompt />
{/if}
