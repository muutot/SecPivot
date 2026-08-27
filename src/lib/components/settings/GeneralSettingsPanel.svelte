<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import type { GeneralSettings, WindowEffect } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Select from "$lib/components/Select.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";
  import SettingToggleCard from "$lib/components/settings/SettingToggleCard.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import { DARK_THEME_COLORS, LIGHT_THEME_COLORS, type ThemeColors } from "$lib/types/theme";

  type Section = "appearance" | "display" | "compact" | "toolbar" | "network";

  interface Props {
    onclose: () => void;
    showHeader?: boolean;
    section: Section;
  }

  let { onclose, showHeader = true, section }: Props = $props();

  let s = $state($appSettings);
  $effect(() => {
    const unsubscribe = appSettings.subscribe((value) => {
      s = value;
    });
    return unsubscribe;
  });

  const general = $derived(s.general);

  function change<K extends keyof GeneralSettings>(key: K, value: GeneralSettings[K]): void {
    appSettings.updateGeneral(key, value);
  }

  function updateColor(key: keyof ThemeColors, value: string): void {
    appSettings.updateGeneral("themeColors", { ...s.general.themeColors, [key]: value });
  }

  const fontSliders: {
    key: "base" | "secondary" | "cardTitle";
    label: string;
    description: string;
    min: number;
    max: number;
  }[] = [
    { key: "base", label: "基础字号", description: "全局界面与正文", min: 11, max: 20 },
    { key: "secondary", label: "次级字号", description: "描述与元数据", min: 9, max: 16 },
    { key: "cardTitle", label: "标题字号", description: "条目标题与卡片标题", min: 11, max: 18 },
  ];

  const densitySliders: {
    key: "groupGap" | "groupPaddingY" | "groupIndent" | "groupRadius";
    label: string;
    description: string;
    min: number;
    max: number;
  }[] = [
    { key: "groupGap", label: "分组间距", description: "分组之间的垂直间距", min: 0, max: 16 },
    {
      key: "groupPaddingY",
      label: "分组上下边距",
      description: "分组文字与边框的上下间距",
      min: 0,
      max: 16,
    },
    { key: "groupIndent", label: "分组缩进", description: "每级子分组的缩进距离", min: 4, max: 32 },
    {
      key: "groupRadius",
      label: "分组选中圆角",
      description: "分组选中背景的圆角半径",
      min: 0,
      max: 12,
    },
  ];

  const customColorGroups: {
    label: string;
    fields: { key: keyof ThemeColors; label: string; description: string }[];
  }[] = [
    {
      label: "基础",
      fields: [
        { key: "accent", label: "强调色", description: "主按钮与高亮" },
        { key: "selectionColor", label: "选中色", description: "选中项与焦点边框" },
        { key: "linkColor", label: "链接色", description: "网址与可点击链接" },
      ],
    },
    {
      label: "文本",
      fields: [
        { key: "textPrimary", label: "主要文本", description: "正文与标题" },
        { key: "textSecondary", label: "次要文本", description: "次级内容" },
        { key: "textMuted", label: "弱化文本", description: "描述与元数据" },
        { key: "textFaint", label: "最弱文本", description: "最低强调的文字与图标" },
        { key: "placeholderColor", label: "占位符", description: "输入框占位文字" },
      ],
    },
    {
      label: "表面",
      fields: [
        { key: "bg", label: "背景", description: "窗口与整体底色" },
        { key: "settingsBg", label: "设置背景", description: "设置界面底色" },
        { key: "cardBg", label: "卡片", description: "卡片与面板底色" },
        { key: "surfaceBg", label: "浮层", description: "弹出层与面板" },
        { key: "statusBarBg", label: "状态栏", description: "底部状态栏底色" },
        { key: "hoverBg", label: "悬停背景", description: "悬停与静默选中表面" },
        { key: "inputBg", label: "输入框", description: "输入框与内嵌表面" },
      ],
    },
    {
      label: "边框",
      fields: [
        { key: "border", label: "边框", description: "分隔线与控件描边" },
        { key: "borderSubtle", label: "细分隔线", description: "更安静的分割线" },
        { key: "scrollbarColor", label: "滚动条", description: "滚动条滑块颜色" },
      ],
    },
    {
      label: "状态",
      fields: [
        { key: "successColor", label: "成功", description: "成功状态提示" },
        { key: "dangerColor", label: "危险", description: "删除与错误状态" },
        { key: "warningColor", label: "警告", description: "警示与收藏强调" },
      ],
    },
  ];
</script>

{#snippet presetPaletteCard(title: string, description: string, colors: ThemeColors)}
  <section class="setting-card">
    <div class="setting-heading">
      <span class="setting-icon"><AppIcon name="settings" size={17} /></span>
      <div>
        <strong>{title}</strong>
        <p>{description}</p>
      </div>
    </div>
    <div class="color-list">
      {#each customColorGroups as group (group.label)}
        <div class="color-group-label">{group.label}</div>
        {#each group.fields as field (field.key)}
          <div class="setting-row">
            <div class="setting-heading">
              <span class="setting-icon color-swatch" style:background-color={colors[field.key]}
              ></span>
              <div>
                <strong>{field.label}</strong>
                <p>{field.description}</p>
              </div>
            </div>
            <code class="readonly-hex">{colors[field.key]}</code>
          </div>
        {/each}
      {/each}
    </div>
  </section>
{/snippet}

{#if showHeader}
  <header>
    <div>
      <span class="eyebrow">Settings · 通用</span>
      <h2>通用</h2>
      <p>外观、字体与界面密度设置。</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label="关闭">×</button>
  </header>
{/if}

<div class="settings-scroll">
  {#if section === "appearance"}
    <section class="setting-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="palette" size={17} /></span>
        <div class="heading-inline">
          <div>
            <strong>主题模式</strong>
            <p>选择内置主题或启用自定义配色</p>
          </div>
        </div>
      </div>
      <div class="theme-segmented" role="group" aria-label="主题模式">
        <button
          class="theme-segment"
          class:active={general.theme === "dark"}
          onclick={() => change("theme", "dark")}
        >
          <span class="swatch swatch-dark"></span>深色
        </button>
        <button
          class="theme-segment"
          class:active={general.theme === "light"}
          onclick={() => change("theme", "light")}
        >
          <span class="swatch swatch-light"></span>浅色
        </button>
        <button
          class="theme-segment"
          class:active={general.theme === "custom"}
          onclick={() => change("theme", "custom")}
        >
          <span class="swatch swatch-custom"></span>自定义
        </button>
      </div>
    </section>

    {#if general.theme === "custom"}
      <section class="setting-card">
        <div class="setting-heading">
          <span class="setting-icon"><AppIcon name="settings" size={17} /></span>
          <div class="heading-inline">
            <div>
              <strong>自定义配色</strong>
              <p>直接修改主题语义色，即时预览</p>
            </div>
            <div class="preset-row">
              <button
                class="preset-button"
                onclick={() => change("themeColors", { ...DARK_THEME_COLORS })}>深色预设</button
              >
              <button
                class="preset-button"
                onclick={() => change("themeColors", { ...LIGHT_THEME_COLORS })}>浅色预设</button
              >
              <button
                class="reset-button"
                onclick={() => change("themeColors", { ...DARK_THEME_COLORS })}>恢复默认</button
              >
            </div>
          </div>
        </div>
        <div class="color-list">
          {#each customColorGroups as group (group.label)}
            <div class="color-group-label">{group.label}</div>
            {#each group.fields as field (field.key)}
              <div class="setting-row">
                <div class="setting-heading">
                  <span
                    class="setting-icon color-swatch"
                    style:background-color={s.general.themeColors[field.key]}
                  ></span>
                  <div>
                    <strong>{field.label}</strong>
                    <p>{field.description}</p>
                  </div>
                </div>
                <div class="color-control">
                  <input
                    type="color"
                    class="color-input"
                    value={s.general.themeColors[field.key].slice(0, 7)}
                    oninput={(e) => updateColor(field.key, e.currentTarget.value)}
                  />
                  <div class="color-hex-input">
                    <TextField
                      size="control"
                      spellcheck={false}
                      value={s.general.themeColors[field.key]}
                      placeholder="#RRGGBBAA"
                      oninput={(e) => updateColor(field.key, e.currentTarget.value)}
                    />
                  </div>
                </div>
              </div>
            {/each}
          {/each}
        </div>
      </section>
    {:else if general.theme === "dark"}
      {@render presetPaletteCard("深色配色", "内置默认配色（只读）", DARK_THEME_COLORS)}
    {:else if general.theme === "light"}
      {@render presetPaletteCard("浅色配色", "内置默认配色（只读）", LIGHT_THEME_COLORS)}
    {/if}
  {/if}

  {#if section === "display"}
    <SettingToggleCard
      icon="eye"
      label="显示条目描述"
      description="在列表中展示去除协议后的网址信息"
      checked={general.showDescriptions}
      ariaLabel="显示描述"
      onchange={(checked) => change("showDescriptions", checked)}
    />

    <SettingToggleCard
      icon="eye"
      label="工具栏仅显示图标"
      description="控制按钮只显示图标，按钮名称在鼠标悬浮时提示"
      checked={general.iconOnlyButtons}
      onchange={(checked) => change("iconOnlyButtons", checked)}
    />

    <SettingToggleCard
      icon="folder"
      label="显示分组图标"
      description="在分组名称前显示文件夹图标"
      checked={s.general.density.showGroupIcon}
      onchange={(checked) =>
        change("density", {
          ...s.general.density,
          showGroupIcon: checked,
        })}
    />

    <SettingToggleCard
      icon="chevron-down"
      label="显示折叠箭头"
      description="在可展开分组前显示折叠箭头"
      checked={s.general.density.showGroupChevron}
      onchange={(checked) =>
        change("density", {
          ...s.general.density,
          showGroupChevron: checked,
        })}
    />

    <SettingToggleCard
      icon="grid"
      label="移动端显示列"
      description="窄屏下也按桌面布局渲染条目表格的完整列，可在列配置菜单中自由选择显示哪些列；关闭则使用单列摘要卡片"
      checked={s.general.mobileColumns}
      ariaLabel="移动端显示列"
      onchange={(checked) => change("mobileColumns", checked)}
    />

    <SettingRangeCard
      icon="sliders"
      label="窗口不透明度"
      description="调整主窗口的整体透明度"
      value={s.general.windowOpacity}
      valueLabel={`${s.general.windowOpacity}%`}
      min={40}
      max={100}
      onchange={(value) => change("windowOpacity", value)}
    />

    <section class="setting-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="widgets" size={17} /></span>
        <div class="heading-inline">
          <div>
            <strong>窗口效果</strong>
            <p>Windows 平台背景材质</p>
          </div>
          <Select
            value={general.windowEffect}
            options={[
              { value: "off", label: "关闭" },
              { value: "acrylic", label: "亚克力" },
              { value: "mica", label: "云母" },
            ]}
            onchange={(v) => change("windowEffect", v as WindowEffect)}
          />
        </div>
      </div>
    </section>

    <SettingToggleCard
      icon="folder"
      label="记住上次数据库"
      description="启动时自动加载最近打开的数据库"
      checked={general.rememberLastDatabase}
      onchange={(checked) => change("rememberLastDatabase", checked)}
    />
  {/if}

  {#if section === "compact"}
    <SettingToggleCard
      icon="grid"
      label="紧凑模式"
      description="缩小间距，提高单屏信息密度"
      checked={general.compactMode}
      onchange={(checked) => change("compactMode", checked)}
    />

    {#each densitySliders as slider (slider.key)}
      <SettingRangeCard
        icon="sliders"
        label={slider.label}
        description={slider.description}
        value={s.general.density[slider.key]}
        valueLabel={`${s.general.density[slider.key]}px`}
        min={slider.min}
        max={slider.max}
        onchange={(value) =>
          change("density", {
            ...s.general.density,
            [slider.key]: value,
          })}
      />
    {/each}

    {#each fontSliders as slider (slider.key)}
      <SettingRangeCard
        icon="keyboard"
        label={slider.label}
        description={slider.description}
        value={s.general.fontSizes[slider.key]}
        valueLabel={`${s.general.fontSizes[slider.key]}px`}
        min={slider.min}
        max={slider.max}
        onchange={(value) =>
          change("fontSizes", {
            ...s.general.fontSizes,
            [slider.key]: value,
          })}
      />
    {/each}
  {/if}

  {#if section === "toolbar"}
    <p class="settings-note" style="margin: 0 0 8px; color: var(--text-muted); font-size: var(--settings-description-size);">逐项控制是否在主界面直接显示；关闭则收纳到「更多」菜单（窗口按钮关闭则隐藏）。</p>
    {#each [
      { key: 'saveAs', label: '另存为', desc: '工具栏左侧“另存为”按钮', icon: 'copy' },
      { key: 'toggleDetail', label: '详情面板切换', desc: '工具栏“显示/隐藏详情”按钮', icon: 'eye' },
      { key: 'securityReport', label: '安全报告', desc: '工具栏“安全报告”按钮', icon: 'shield' },
      { key: 'similarPasswords', label: '相似密码检查', desc: '收纳菜单中的“相似密码检查”', icon: 'shield' },
      { key: 'hibpCheck', label: 'HIBP 泄露检查', desc: '收纳菜单中的“HIBP 泄露检查”', icon: 'globe' },
      { key: 'importMenu', label: '导入', desc: '收纳菜单中的“导入”子菜单（CSV/XML/Bitwarden/1Password）', icon: 'upload' },
      { key: 'exportMenu', label: '导出', desc: '工具栏/收纳菜单中的“导出”', icon: 'download' },
      { key: 'expiredEntries', label: '过期条目', desc: '收纳菜单中的“过期条目”', icon: 'clock' },
      { key: 'clearHistory', label: '清理全部历史', desc: '收纳菜单中的“清理全部历史”', icon: 'trash' },
      { key: 'dbSettings', label: '数据库设置', desc: '收纳菜单中的“数据库设置”', icon: 'settings' },
      { key: 'appSettings', label: '设置', desc: '工具栏“设置”按钮', icon: 'settings' },
      { key: 'windowMinimize', label: '窗口：最小化 —', desc: '主窗口标题栏/工具栏“最小化”按钮', icon: 'minimize' },
      { key: 'windowMaximize', label: '窗口：最大化/还原 □', desc: '主窗口“最大化/还原”按钮', icon: 'maximize' },
      { key: 'windowClose', label: '窗口：关闭 ×', desc: '主窗口“关闭”按钮', icon: 'x' },
    ] as item (item.key)}
      <SettingToggleCard
        icon={item.icon as never}
        label={item.label}
        description={item.desc}
        checked={(s.general.toolbarItems as unknown as Record<string, boolean>)[item.key]}
        onchange={(checked) =>
          change("toolbarItems", {
            ...s.general.toolbarItems,
            [item.key]: checked,
          } as never)}
      />
    {/each}
  {/if}

  {#if section === "network"}
    <SettingToggleCard
      icon="save"
      label="下载图标后自动保存"
      description="关闭时图标仅应用到当前会话并标记未保存，需手动保存；开启则下载完成后立即写入数据库"
      checked={s.favicon.autoSave}
      onchange={(checked) => appSettings.updateFavicon("autoSave", checked)}
    />
    <SettingRangeCard
      icon="globe"
      label="图标下载并发数"
      description="下载条目网址图标时同时进行的请求数，默认 8"
      value={s.favicon.concurrency}
      valueLabel={`${s.favicon.concurrency} 个`}
      min={1}
      max={16}
      onchange={(value) => appSettings.updateFavicon("concurrency", value)}
    />
  {/if}

  <p class="auto-save-note">修改即时生效并自动保存</p>
</div>

<style>
  .theme-segmented {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 6px;
    margin-top: 10px;
  }

  .theme-segment {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    height: 30px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--input-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    cursor: pointer;
  }

  .theme-segment.active {
    border-color: var(--selection-color);
    color: var(--text-primary);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--input-bg));
  }

  .swatch {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    border: 1px solid rgba(128, 128, 128, 0.5);
  }

  .swatch-dark {
    background: #111111;
  }

  .swatch-light {
    background: #f5f5f5;
  }

  .swatch-custom {
    background: conic-gradient(var(--accent), var(--selection-color), var(--success-color));
  }

  .preset-row {
    display: flex;
    gap: 6px;
    margin-top: 10px;
  }

  .color-group-label {
    margin-top: 10px;
    color: var(--text-muted);
    font-size: var(--settings-note-size, var(--font-size-tiny, 10px));
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .readonly-hex {
    flex: 0 0 auto;
    padding: 4px 8px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--input-bg);
    font-family: var(--font-mono);
    font-size: var(--settings-note-size, var(--font-size-tiny, 10px));
    font-variant-numeric: tabular-nums;
  }

  .heading-inline .preset-row {
    flex-shrink: 0;
    margin-top: 0;
  }

  .reset-button,
  .preset-button {
    padding: 4px 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    color: var(--text-secondary);
    background: var(--input-bg);
    font-size: var(--settings-control-size, var(--font-size-secondary, 11px));
    cursor: pointer;
  }

  .reset-button:hover,
  .preset-button:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }
</style>
