<script lang="ts">
  import { vault } from "$lib/services/vault";
  import type { SessionInfo } from "$lib/types/vault";

  let sessions = $state<SessionInfo[]>([]);
  let activeIdValue = $state<string | null>(null);
  let currentDirty = $state(false);
  $effect(() => {
    const unsubTabs = vault.tabs.subscribe((value) => {
      sessions = value;
    });
    const unsubActive = vault.activeId.subscribe((value) => {
      activeIdValue = value;
    });
    const unsubState = vault.subscribe((value) => {
      currentDirty = value?.dirty ?? false;
    });
    return () => {
      unsubTabs();
      unsubActive();
      unsubState();
    };
  });

  function switchTo(sessionId: string): void {
    if (sessionId !== activeIdValue) void vault.setActiveSession(sessionId);
  }
</script>

{#if sessions.length > 1}
  <div class="tab-bar" aria-label="打开的数据库">
    {#each sessions as session (session.sessionId)}
      <div
        class="vault-tab"
        class:active={session.sessionId === activeIdValue}
        title={session.path}
      >
        <button
          type="button"
          class="tab-main"
          onclick={() => switchTo(session.sessionId)}
          aria-label={`切换到 ${session.fileName}`}
        >
          <span class="tab-name">{session.fileName}</span>
          {#if session.sessionId === activeIdValue ? currentDirty : session.dirty}
            <span class="tab-dirty" aria-label="有未保存的更改"></span>
          {/if}
        </button>
        <button
          type="button"
          class="tab-close"
          onclick={() => void vault.closeTab(session.sessionId)}
          aria-label={`关闭 ${session.fileName}`}
        >
          ×
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .tab-bar {
    display: flex;
    gap: 4px;
    padding: 4px 8px;
    overflow-x: auto;
    border-bottom: 1px solid var(--border-subtle);
  }

  .vault-tab {
    display: flex;
    align-items: stretch;
    height: 26px;
    overflow: hidden;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
  }

  .vault-tab.active {
    border-color: color-mix(in srgb, var(--selection-color) 55%, transparent);
    background: color-mix(in srgb, var(--selection-color) 12%, transparent);
  }

  .tab-main {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px;
    color: var(--text-secondary);
    background: transparent;
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .tab-main:hover {
    color: var(--text-primary);
  }

  .vault-tab.active .tab-main {
    color: var(--text-primary);
  }

  .tab-name {
    max-width: 160px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tab-dirty {
    width: 6px;
    height: 6px;
    flex: none;
    border-radius: 50%;
    background: var(--warning-color);
  }

  .tab-close {
    width: 22px;
    border: none;
    border-left: 1px solid var(--border-subtle);
    color: var(--text-faint);
    background: transparent;
    font-size: 12px;
    line-height: 1;
    cursor: pointer;
  }

  .tab-close:hover {
    color: var(--danger-color);
    background: var(--hover-bg);
  }
</style>
