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
    // Kill middle-button autoscroll globally: WebView2/Chromium engages it on
    // mousedown, so a capture-phase preventDefault here beats any element
    // handler (which would otherwise fire too late, on auxclick/mouseup).
    const onMiddleDown = (event: MouseEvent): void => {
      if (event.button === 1) event.preventDefault();
    };
    document.addEventListener("mousedown", onMiddleDown, true);
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
      document.removeEventListener("mousedown", onMiddleDown, true);
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

<svelte:head>
  <meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover" />
</svelte:head>
