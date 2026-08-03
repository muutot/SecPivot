<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import GeneralSettingsPanel from "$lib/components/settings/GeneralSettingsPanel.svelte";
  import SecuritySettingsPanel from "$lib/components/settings/SecuritySettingsPanel.svelte";
  import DatabaseSettingsPanel from "$lib/components/settings/DatabaseSettingsPanel.svelte";
  import RemoteSettingsPanel from "$lib/components/settings/RemoteSettingsPanel.svelte";
  import BridgeSettingsPanel from "$lib/components/settings/BridgeSettingsPanel.svelte";
  import RpcSettingsPanel from "$lib/components/settings/RpcSettingsPanel.svelte";
  import KeyboardSettingsPanel from "$lib/components/settings/KeyboardSettingsPanel.svelte";
  import AboutSettingsPanel from "$lib/components/settings/AboutSettingsPanel.svelte";
  import WindowControls from "$lib/components/WindowControls.svelte";

  type Section =
    "general" | "security" | "keyboard" | "database" | "remote" | "integrations" | "about";
  type GeneralTab = "appearance" | "display" | "compact" | "network";
  type IntegrationsTab = "http" | "rpc";

  interface Props {
    onclose: () => void;
    appVersion?: string;
  }

  let { onclose, appVersion = "0.1.0" }: Props = $props();

  let active: Section = $state("general");
  let generalTab: GeneralTab = $state("appearance");
  let integrationsTab: IntegrationsTab = $state("http");

  const currentTab = $derived(
    active === "general" ? generalTab : active === "integrations" ? integrationsTab : null,
  );

  const sections: {
    id: Section;
    label: string;
    icon: "sliders" | "shield" | "keyboard" | "database" | "cloud" | "plug" | "info";
    description: string;
    tabs?: { id: string; label: string }[];
    title: string;
  }[] = [
    {
      id: "general",
      label: "通用",
      icon: "sliders",
      title: "通用",
      description: "外观、字体与界面密度设置，修改即时生效。",
      tabs: [
        { id: "appearance", label: "外观" },
        { id: "display", label: "显示" },
        { id: "compact", label: "紧凑" },
        { id: "network", label: "网络" },
      ],
    },
    {
      id: "security",
      label: "安全",
      icon: "shield",
      title: "安全",
      description: "自动锁定、剪贴板清理与托盘行为。",
    },
    {
      id: "keyboard",
      label: "快捷键",
      icon: "keyboard",
      title: "快捷键",
      description: "全局自动填充热键与常用操作的窗口内快捷键。",
    },
    {
      id: "database",
      label: "数据库",
      icon: "database",
      title: "数据库",
      description: "新建数据库的加密默认值与密码生成规则。",
    },
    {
      id: "remote",
      label: "远程",
      icon: "cloud",
      title: "远程",
      description: "S3 兼容对象存储的连接、凭据与本地镜像设置。",
    },
    {
      id: "integrations",
      label: "集成",
      icon: "plug",
      title: "集成",
      description: "浏览器桥接（KeePassHttp 与 KeePassRPC 兼容）与授权客户端管理。",
      tabs: [
        { id: "http", label: "KeePassHttp" },
        { id: "rpc", label: "KeePassRPC" },
      ],
    },
    {
      id: "about",
      label: "关于",
      icon: "info",
      title: "关于",
      description: "版本、技术栈与许可证信息。",
    },
  ];

  const activeSection = $derived(sections.find((s) => s.id === active) ?? sections[0]);
</script>

<div class="settings-dialog settings-dialog--standalone" role="dialog" aria-label="设置">
  <aside class="settings-sidebar" data-tauri-drag-region>
    <div class="settings-brand">
      <span class="brand-icon"><AppIcon name="key" size={17} /></span>
      <div class="brand-text">
        <strong>KeyVault</strong>
        <small>v{appVersion}</small>
      </div>
    </div>
    <nav class="settings-primary-nav" aria-label="设置分类">
      {#each sections as section (section.id)}
        <button
          class="settings-nav-item"
          class:active={active === section.id}
          onclick={() => {
            active = section.id;
            if (section.id === "general") generalTab = "appearance";
            if (section.id === "integrations") integrationsTab = "http";
          }}
        >
          <AppIcon name={section.icon} size={16} />
          <span>{section.label}</span>
        </button>
      {/each}
    </nav>
    <div class="sidebar-foot">
      <p class="sidebar-hint">本地加密存储 · 远程同步可选</p>
    </div>
  </aside>

  <div id="settings-content" class="settings-content">
    <section class="settings-section-header">
      <div class="settings-section-heading-row">
        <div class="settings-breadcrumb">{activeSection.title}</div>
        <div class="settings-section-actions">
          <span class="settings-count"
            >{activeSection.tabs?.length ? `${activeSection.tabs.length} 组` : "1 页"}</span
          >
          <WindowControls variant="chrome" />
          <button class="close-button" onclick={onclose} aria-label="关闭">×</button>
        </div>
      </div>

      {#if activeSection.tabs}
        <nav class="settings-subnav" aria-label="{activeSection.title}子分类">
          {#each activeSection.tabs as tab (tab.id)}
            <button
              class="settings-subnav-item"
              class:active={currentTab === tab.id}
              onclick={() => {
                if (active === "general") generalTab = tab.id as GeneralTab;
                if (active === "integrations") integrationsTab = tab.id as IntegrationsTab;
              }}
            >
              {tab.label}
            </button>
          {/each}
        </nav>
      {:else}
        <div class="settings-subnav settings-subnav--single">
          <span class="settings-section-title">{activeSection.title}</span>
        </div>
      {/if}

      <p class="settings-section-description">{activeSection.description}</p>
    </section>

    {#if active === "general"}
      <GeneralSettingsPanel {onclose} showHeader={false} section={generalTab} />
    {:else if active === "security"}
      <SecuritySettingsPanel {onclose} showHeader={false} />
    {:else if active === "keyboard"}
      <KeyboardSettingsPanel {onclose} showHeader={false} />
    {:else if active === "database"}
      <DatabaseSettingsPanel {onclose} showHeader={false} />
    {:else if active === "remote"}
      <RemoteSettingsPanel {onclose} showHeader={false} />
    {:else if active === "integrations"}
      {#if integrationsTab === "rpc"}
        <RpcSettingsPanel {onclose} showHeader={false} />
      {:else}
        <BridgeSettingsPanel {onclose} showHeader={false} />
      {/if}
    {:else if active === "about"}
      <AboutSettingsPanel {onclose} showHeader={false} {appVersion} />
    {/if}
  </div>
</div>

<style>
  .settings-dialog {
    display: grid;
    grid-template-columns: 168px minmax(0, 1fr);
    width: 100%;
    height: 100%;
    color: var(--text-primary);
    background: var(--bg-settings);
  }

  .settings-dialog--standalone {
    border: 0;
    border-radius: 0;
  }

  .settings-sidebar {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--border-subtle);
    background: var(--surface-bg);
  }

  .settings-brand {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 14px 14px 12px;
  }

  .brand-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    flex: 0 0 auto;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-icon-radius, 7px);
    color: var(--warning-color);
    background: var(--hover-bg);
  }

  .brand-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .brand-text strong {
    font-size: 13px;
    font-weight: 590;
    letter-spacing: 0.01em;
  }

  .brand-text small {
    margin-top: 1px;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    font-variant-numeric: tabular-nums;
  }

  .settings-primary-nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px 8px;
  }

  .settings-nav-item {
    display: flex;
    align-items: center;
    gap: 9px;
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border: 1px solid transparent;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: transparent;
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    text-align: left;
    cursor: pointer;
  }

  .settings-nav-item:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .settings-nav-item.active {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--hover-bg));
  }

  .sidebar-foot {
    margin-top: auto;
    padding: 12px 14px;
    border-top: 1px solid var(--border-subtle);
  }

  .sidebar-hint {
    margin: 0;
    color: var(--text-faint);
    font-size: var(--font-size-tiny, 10px);
    line-height: 1.5;
  }

  .settings-content {
    position: relative;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--bg-settings);
  }

  .settings-section-header {
    flex: 0 0 auto;
    padding: 14px 18px 10px;
    border-bottom: 1px solid var(--border-subtle);
  }

  .settings-section-heading-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .settings-breadcrumb {
    color: var(--text-primary);
    font-size: var(--settings-page-title-size, 18px);
    font-weight: 590;
  }

  .settings-section-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .settings-count {
    color: var(--text-faint);
    font-size: var(--settings-note-size, var(--font-size-tiny, 10px));
    font-variant-numeric: tabular-nums;
  }

  .settings-subnav {
    display: flex;
    gap: 2px;
    margin-top: 12px;
  }

  .settings-subnav--single {
    margin-top: 9px;
  }

  .settings-subnav-item {
    padding: 4px 10px;
    border: 1px solid transparent;
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-muted);
    background: transparent;
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    cursor: pointer;
  }

  .settings-subnav-item:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }

  .settings-subnav-item.active {
    border-color: var(--border-color);
    color: var(--text-primary);
    background: var(--card-bg);
  }

  .settings-section-title {
    color: var(--text-muted);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
  }

  .settings-section-description {
    margin: 8px 0 0;
    color: var(--text-muted);
    font-size: var(--settings-description-size, var(--font-size-secondary, 11px));
    line-height: 1.5;
  }
</style>
