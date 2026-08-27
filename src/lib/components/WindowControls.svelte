<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { isMobile, isTauriRuntime } from "$lib/services/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";

  interface Props {
    /** "toolbar": bordered compact buttons matching .icon-action;
     *  "chrome": flat titlebar-style buttons (welcome/lock/settings). */
    variant?: "toolbar" | "chrome";
    showMinimize?: boolean;
    showMaximize?: boolean;
    showClose?: boolean;
  }

  let { variant = "chrome", showMinimize = true, showMaximize = true, showClose = true }: Props = $props();

  const appWindow = isTauriRuntime() ? getCurrentWindow() : null;
  let maximized = $state(false);

  onMount(() => {
    if (!appWindow) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const sync = (): void => {
      appWindow.isMaximized().then((value) => {
        if (!disposed) maximized = value;
      });
    };
    sync();
    appWindow.onResized(sync).then((un) => {
      if (disposed) un();
      else unlisten = un;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  });

  function minimize(): void {
    appWindow?.minimize().catch(() => {});
  }

  function toggleMaximize(): void {
    appWindow?.toggleMaximize().catch(() => {});
  }

  function close(): void {
    appWindow?.close().catch(() => {});
  }
</script>

{#if appWindow && !isMobile()}
  <div
    class="window-controls"
    class:chrome={variant === "chrome"}
    role="group"
    aria-label="窗口控制"
  >
    {#if showMinimize}
      <button class="wc-btn" onclick={minimize} title="最小化" aria-label="最小化">
        <AppIcon name="minimize" size={13} />
      </button>
    {/if}
    {#if showMaximize}
      <button
        class="wc-btn"
        onclick={toggleMaximize}
        title={maximized ? "还原" : "最大化"}
        aria-label={maximized ? "还原" : "最大化"}
      >
        <AppIcon name={maximized ? "restore" : "maximize"} size={12} />
      </button>
    {/if}
    {#if showClose}
      <button class="wc-btn wc-close" onclick={close} title="关闭" aria-label="关闭">
        <AppIcon name="x" size={13} />
      </button>
    {/if}
  </div>
{/if}

<style>
  .window-controls {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .wc-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    flex: 0 0 auto;
    padding: 0;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: var(--card-bg);
    cursor: pointer;
  }

  .wc-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .wc-close:hover {
    color: #ffffff;
    background: var(--danger-color);
    border-color: var(--danger-color);
  }

  .window-controls.chrome .wc-btn {
    width: 32px;
    height: 28px;
    border: 0;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
  }

  .window-controls.chrome .wc-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .window-controls.chrome .wc-close:hover {
    color: #ffffff;
    background: var(--danger-color);
  }

  @media (max-width: 420px) {
    .window-controls {
      display: none;
    }
  }
</style>
