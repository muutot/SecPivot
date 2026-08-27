<script lang="ts">
  import AppIcon from "$lib/components/AppIcon.svelte";
  import WindowControls from "$lib/components/WindowControls.svelte";
  import type { ToolbarItemVisibility } from "$lib/types/settings";

  interface Props {
    /** Two-way bound search text. */
    search?: string;
    iconOnlyButtons: boolean;
    toolbarItems: ToolbarItemVisibility;
    showWindowControls: boolean;
    busy: boolean;
    dirty: boolean;
    readOnly: boolean;
    mobileNavOpen: boolean;
    detailVisible: boolean;
    toolbarMenuOpen: boolean;
    advancedFilterActive: boolean;
    ontogglenav: () => void;
    onsave: () => void;
    onsaveas: () => void;
    onlock: () => void;
    onnewentry: () => void;
    onclearsearch: () => void;
    onadvancedsearch: () => void;
    ontoggledetail: () => void;
    onreport: () => void;
    onexportcsv: () => void;
    onsettings: () => void;
    ontogglemenu: (event: MouseEvent) => void;
    onsimilar?: () => void;
    onhibp?: () => void;
    onexpired?: () => void;
    onclearhistory?: () => void;
    ondbsettings?: () => void;
    onimportcsv?: () => void;
  }

  let {
    search = $bindable(""),
    iconOnlyButtons,
    toolbarItems,
    showWindowControls,
    busy,
    dirty,
    readOnly,
    mobileNavOpen,
    detailVisible,
    toolbarMenuOpen,
    advancedFilterActive,
    ontogglenav,
    onsave,
    onsaveas,
    onlock,
    onnewentry,
    onclearsearch,
    onadvancedsearch,
    ontoggledetail,
    onreport,
    onexportcsv,
    onsettings,
    ontogglemenu,
    onsimilar,
    onhibp,
    onexpired,
    onclearhistory,
    ondbsettings,
    onimportcsv,
  }: Props = $props();

  const hasOverflow = $derived(
    !toolbarItems.saveAs ||
      !toolbarItems.toggleDetail ||
      !toolbarItems.securityReport ||
      !toolbarItems.similarPasswords ||
      !toolbarItems.hibpCheck ||
      !toolbarItems.importMenu ||
      !toolbarItems.exportMenu ||
      !toolbarItems.expiredEntries ||
      !toolbarItems.clearHistory ||
      !toolbarItems.dbSettings ||
      !toolbarItems.appSettings,
  );

  let searchInputEl = $state<HTMLInputElement | null>(null);

  /** Focus hook for the Ctrl+K shortcut (the input lives in this component). */
  export function focusSearch(): void {
    searchInputEl?.focus();
  }
</script>

<div class="toolbar" role="presentation" data-tauri-drag-region>
  <div class="toolbar-left">
    <button
      class="mobile-nav-toggle"
      class:active={mobileNavOpen}
      onclick={ontogglenav}
      title="分组"
      aria-label="切换分组面板"
      aria-expanded={mobileNavOpen}
    >
      <AppIcon name="menu" size={15} />
    </button>
    <button
      class="tool-button primary"
      class:icon-only={iconOnlyButtons}
      onclick={onnewentry}
      title="新建条目 (Ctrl+N)"
    >
      <AppIcon name="plus" size={14} />
      {#if !iconOnlyButtons}<span class="btn-label">条目</span>{/if}
    </button>
    <button
      class="tool-button"
      class:icon-only={iconOnlyButtons}
      onclick={onsave}
      disabled={busy || !dirty || readOnly}
      title="保存数据库 (Ctrl+S)"
    >
      <AppIcon name="save" size={14} />
      {#if !iconOnlyButtons}<span class="btn-label">保存</span>{/if}
    </button>
    {#if toolbarItems.saveAs}
      <button
        class="tool-button"
        class:icon-only={iconOnlyButtons}
        onclick={onsaveas}
        title="另存为数据库副本到新路径"
      >
        <AppIcon name="copy" size={14} />
        {#if !iconOnlyButtons}<span class="btn-label">另存为</span>{/if}
      </button>
    {/if}
    <span class="toolbar-divider" aria-hidden="true"></span>
    <button
      class="tool-button"
      class:icon-only={iconOnlyButtons}
      onclick={onlock}
      title="锁定数据库"
    >
      <AppIcon name="lock" size={14} />
      {#if !iconOnlyButtons}<span class="btn-label">锁定</span>{/if}
    </button>
  </div>

  <div class="toolbar-center">
    <div class="search-box">
      <span class="search-icon"><AppIcon name="search" size={13} /></span>
      <input
        class="search-input"
        type="search"
        placeholder="搜索…"
        bind:value={search}
        bind:this={searchInputEl}
        aria-label="搜索条目"
      />
      {#if search}
        <button class="clear-button" onclick={onclearsearch} aria-label="清除搜索">×</button>
      {/if}
      <button
        class="filter-button"
        class:active={advancedFilterActive}
        onclick={onadvancedsearch}
        title="高级搜索"
        aria-label="高级搜索"
      >
        <AppIcon name="sliders" size={13} />
      </button>
    </div>
  </div>

  <div class="toolbar-right">
    {#if readOnly}
      <span class="readonly-badge" title="连续保存失败，数据库已进入只读模式">只读</span>
    {/if}
    {#if dirty}
      <span class="dirty-badge">未保存</span>
    {/if}
    {#if toolbarItems.toggleDetail}
      <button
        class="icon-action"
        onclick={ontoggledetail}
        title={detailVisible ? "隐藏详情面板" : "显示详情面板"}
        aria-pressed={detailVisible}
      >
        <AppIcon name={detailVisible ? "chevron-right" : "chevron-left"} size={15} />
      </button>
    {/if}
    {#if toolbarItems.securityReport}
      <button class="icon-action" onclick={onreport} title="安全报告">
        <AppIcon name="shield" size={15} />
      </button>
    {/if}
    {#if toolbarItems.similarPasswords}
      <button class="icon-action" onclick={() => onsimilar?.()} title="相似密码检查">
        <AppIcon name="shield" size={15} />
      </button>
    {/if}
    {#if toolbarItems.hibpCheck}
      <button class="icon-action" onclick={() => onhibp?.()} title="HIBP 泄露检查">
        <AppIcon name="globe" size={15} />
      </button>
    {/if}
    {#if toolbarItems.expiredEntries}
      <button class="icon-action" onclick={() => onexpired?.()} title="过期条目">
        <AppIcon name="clock" size={15} />
      </button>
    {/if}
    {#if toolbarItems.clearHistory}
      <button class="icon-action" onclick={() => onclearhistory?.()} title="清理全部历史">
        <AppIcon name="trash" size={15} />
      </button>
    {/if}
    {#if toolbarItems.importMenu}
      <button class="icon-action" onclick={() => onimportcsv?.()} title="导入">
        <AppIcon name="upload" size={15} />
      </button>
    {/if}
    {#if toolbarItems.exportMenu}
      <button class="icon-action" onclick={onexportcsv} title="导出 CSV">
        <AppIcon name="download" size={15} />
      </button>
    {/if}
    {#if toolbarItems.dbSettings}
      <button class="icon-action" onclick={() => ondbsettings?.()} title="数据库设置">
        <AppIcon name="database" size={15} />
      </button>
    {/if}
    {#if toolbarItems.appSettings}
      <button class="icon-action" onclick={onsettings} title="设置">
        <AppIcon name="settings" size={16} />
      </button>
    {/if}
    {#if hasOverflow}
      <button
        class="icon-action"
        class:active={toolbarMenuOpen}
        onclick={ontogglemenu}
        title="更多操作"
        aria-label="更多操作"
        aria-haspopup="menu"
        aria-expanded={toolbarMenuOpen}
      >
        <AppIcon name="more-horizontal" size={16} />
      </button>
    {/if}
    {#if showWindowControls}
      <span class="toolbar-divider" aria-hidden="true"></span>
      <WindowControls
        variant="toolbar"
        showMinimize={toolbarItems.windowMinimize}
        showMaximize={toolbarItems.windowMaximize}
        showClose={toolbarItems.windowClose}
      />
    {/if}
  </div>
</div>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 8px 14px;
    border-top: 1px solid var(--border-subtle);
    border-bottom: 1px solid var(--border-subtle);
  }

  .toolbar-divider {
    width: 1px;
    height: 18px;
    flex: 0 0 auto;
    background: var(--border-subtle);
  }

  .toolbar-left,
  .toolbar-center,
  .toolbar-right {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .toolbar-center {
    flex: 1;
    justify-content: center;
  }

  .search-box {
    display: flex;
    align-items: center;
    gap: 6px;
    width: min(340px, 100%);
    height: 28px;
    padding: 0 9px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--input-bg);
  }

  .search-box:focus-within {
    border-color: var(--selection-color);
  }

  .search-icon {
    display: inline-flex;
    color: var(--text-faint);
  }

  .search-input {
    flex: 1;
    min-width: 0;
    padding: 0;
    border: 0;
    outline: none;
    color: var(--text-primary);
    background: transparent;
    font-size: var(--font-size-secondary, 11px);
  }

  .search-input::placeholder {
    color: var(--placeholder-color);
  }

  .search-input::-webkit-search-cancel-button {
    display: none;
  }

  .clear-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    padding: 0;
    border: 0;
    border-radius: 50%;
    color: var(--text-muted);
    background: transparent;
    font-size: 13px;
    line-height: 1;
    cursor: pointer;
  }

  .clear-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .filter-button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex: 0 0 auto;
    padding: 0;
    border: 0;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-faint);
    background: transparent;
    cursor: pointer;
  }

  .filter-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .filter-button.active {
    color: var(--selection-color);
  }

  .icon-action {
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

  .icon-action:hover,
  .icon-action.active {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .tool-button {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--card-bg);
    font-size: var(--font-size-secondary, 11px);
    cursor: pointer;
  }

  .tool-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .tool-button:disabled {
    cursor: not-allowed;
    opacity: 0.55;
  }

  .tool-button.icon-only {
    justify-content: center;
    width: 28px;
    padding: 0;
  }

  .dirty-badge {
    padding: 2px 7px;
    border: 1px solid color-mix(in srgb, var(--warning-color) 45%, transparent);
    border-radius: 10px;
    color: var(--warning-color);
    font-size: var(--font-size-tiny, 10px);
  }

  .readonly-badge {
    padding: 2px 7px;
    border: 1px solid color-mix(in srgb, var(--danger-color) 45%, transparent);
    border-radius: 10px;
    color: var(--danger-color);
    font-size: var(--font-size-tiny, 10px);
  }

  .mobile-nav-toggle {
    display: none;
  }

  @media (max-width: 720px) {
    .toolbar {
      gap: 6px;
      padding: 6px 8px;
    }

    .tool-button {
      width: 28px;
      padding: 0;
      justify-content: center;
    }

    .tool-button .btn-label {
      display: none;
    }

    .tool-button.primary {
      width: 28px;
      padding: 0;
      justify-content: center;
    }
    .toolbar-center {
      flex: 1;
      justify-content: flex-start;
      min-width: 0;
    }

    .search-box {
      width: 100%;
    }

    .mobile-nav-toggle {
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

    .mobile-nav-toggle:hover,
    .mobile-nav-toggle.active {
      color: var(--text-primary);
      background: var(--hover-bg);
    }
  }

  @media (max-width: 420px) {
    .toolbar {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      grid-template-areas:
        "primary primary"
        "search secondary";
      align-items: center;
    }

    .toolbar-left {
      grid-area: primary;
      justify-self: start;
    }

    .toolbar-center {
      grid-area: search;
      width: 100%;
    }

    .toolbar-right {
      grid-area: secondary;
      justify-self: end;
    }

    .toolbar-divider,
    .dirty-badge {
      display: none;
    }

    .mobile-nav-toggle,
    .tool-button,
    .icon-action {
      width: 32px;
      height: 32px;
    }

    .search-box {
      height: 32px;
    }
  }
</style>
