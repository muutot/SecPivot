<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import type { GeneralSettings, WindowEffect } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Select from "$lib/components/Select.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";
  import SettingToggleCard from "$lib/components/settings/SettingToggleCard.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Toggle from "$lib/components/templates/form/Toggle.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
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

  let draggedId: string | null = $state(null);
  let selectedThemeIdx: number | null = $state(null);

  function change<K extends keyof GeneralSettings>(key: K, value: GeneralSettings[K]): void {
    appSettings.updateGeneral(key, value);
  }

  function updateColor(key: keyof ThemeColors, value: string): void {
    appSettings.updateGeneral("themeColors", { ...s.general.themeColors, [key]: value });
  }

  const displayColors = $derived(
    selectedThemeIdx !== null
      ? (s.general.customThemes[selectedThemeIdx]?.colors ?? s.general.themeColors)
      : s.general.themeColors,
  );

  // A saved theme edits its stored copy; the live palette only follows via 应用.
  function updateDisplayColor(key: keyof ThemeColors, value: string): void {
    if (selectedThemeIdx === null) {
      updateColor(key, value);
      return;
    }
    const themes = s.general.customThemes.map((t) => ({ ...t, colors: { ...t.colors } }));
    const theme = themes[selectedThemeIdx];
    if (!theme) return;
    theme.colors = { ...theme.colors, [key]: value };
    appSettings.updateGeneral("customThemes", themes);
  }

  function applyPreset(preset: ThemeColors): void {
    if (selectedThemeIdx === null) {
      change("themeColors", { ...preset });
      return;
    }
    const themes = s.general.customThemes.map((t) => ({ ...t, colors: { ...t.colors } }));
    const theme = themes[selectedThemeIdx];
    if (!theme) return;
    theme.colors = { ...preset };
    appSettings.updateGeneral("customThemes", themes);
  }

  function saveCurrentAsTheme(): void {
    const base = `自定义 ${s.general.customThemes.length + 1}`;
    let name = base;
    let n = 2;
    const names = new Set(s.general.customThemes.map((t) => t.name));
    while (names.has(name)) {
      name = `${base} ${n}`;
      n++;
    }
    const next = [...s.general.customThemes, { name, colors: { ...displayColors } }];
    appSettings.updateGeneral("customThemes", next);
    selectedThemeIdx = next.length - 1;
  }

  function applyCustomTheme(idx: number): void {
    const t = s.general.customThemes[idx];
    if (!t) return;
    appSettings.updateGeneral("themeColors", { ...t.colors });
    if (s.general.theme !== "custom") appSettings.updateGeneral("theme", "custom");
    selectedThemeIdx = null;
  }

  function deleteCustomTheme(idx: number): void {
    appSettings.updateGeneral(
      "customThemes",
      s.general.customThemes.filter((_, i) => i !== idx),
    );
    selectedThemeIdx = null;
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
              <button class="preset-button" onclick={() => applyPreset(DARK_THEME_COLORS)}
                >深色预设</button
              >
              <button class="preset-button" onclick={() => applyPreset(LIGHT_THEME_COLORS)}
                >浅色预设</button
              >
              <button class="reset-button" onclick={() => applyPreset(DARK_THEME_COLORS)}
                >恢复默认</button
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
                    style:background-color={displayColors[field.key]}
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
                    value={displayColors[field.key].slice(0, 7)}
                    oninput={(e) => updateDisplayColor(field.key, e.currentTarget.value)}
                  />
                  <div class="color-hex-input">
                    <TextField
                      size="control"
                      spellcheck={false}
                      value={displayColors[field.key]}
                      placeholder="#RRGGBBAA"
                      oninput={(e) => updateDisplayColor(field.key, e.currentTarget.value)}
                    />
                  </div>
                </div>
              </div>
            {/each}
          {/each}
        </div>
        <div class="setting-heading multi-theme-heading">
          <span class="setting-icon"><AppIcon name="palette" size={17} /></span>
          <div class="heading-inline">
            <div>
              <strong>多主题配置</strong>
              <p>切换编辑当前配色或已保存主题；另存为保存副本，应用使所选主题生效</p>
            </div>
            <div class="theme-config-actions">
              <Select
                value={selectedThemeIdx === null ? "current" : String(selectedThemeIdx)}
                ariaLabel="选择主题"
                options={[
                  { value: "current", label: "当前配色" },
                  ...s.general.customThemes.map((t, i) => ({ value: String(i), label: t.name })),
                ]}
                onchange={(v) => {
                  selectedThemeIdx = v === "current" ? null : Number(v);
                }}
              />
              <Button variant="plain" onclick={saveCurrentAsTheme}>另存为</Button>
              {#if selectedThemeIdx !== null}
                <Button
                  variant="plain"
                  onclick={() => {
                    if (selectedThemeIdx !== null) applyCustomTheme(selectedThemeIdx);
                  }}>应用</Button
                >
                <Button
                  variant="plain"
                  onclick={() => {
                    if (selectedThemeIdx !== null) deleteCustomTheme(selectedThemeIdx);
                  }}>删除</Button
                >
              {/if}
            </div>
          </div>
        </div>
        {#if selectedThemeIdx !== null}
          <div class="theme-name-row">
            <TextField
              value={s.general.customThemes[selectedThemeIdx].name}
              placeholder="主题名称"
              oninput={(e) => {
                const themes = s.general.customThemes.map((t) => ({
                  ...t,
                  colors: { ...t.colors },
                }));
                if (selectedThemeIdx !== null && themes[selectedThemeIdx]) {
                  themes[selectedThemeIdx].name = e.currentTarget.value;
                  appSettings.updateGeneral("customThemes", themes);
                }
              }}
            />
          </div>
        {/if}
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
    <p
      class="settings-note"
      style="margin: 0 0 8px; color: var(--text-muted); font-size: var(--settings-description-size);"
    >
      所有按钮均可显示/隐藏（除“更多”菜单外）、排序、设置左右位置；在项后打开“|”即添加垂直分割线。
    </p>
    <SettingToggleCard
      icon="eye"
      label="点击条目显示详情"
      description="点击条目时自动打开详情面板；关闭后需手动点击工具栏“详情”按钮打开"
      checked={s.general.showDetailOnSelect ?? true}
      onchange={(checked) => change("showDetailOnSelect" as never, checked as never)}
    />
    {@const fullMeta: Record<string, { label: string; desc: string; icon: string }> = {
      newEntry: { label: '新建条目', desc: '新建条目', icon: 'plus' },
      save: { label: '保存', desc: '保存数据库', icon: 'save' },
      saveAs: { label: '另存为', desc: '另存为', icon: 'copy' },
      lock: { label: '锁定', desc: '锁定数据库', icon: 'lock' },
      toggleDetail: { label: '详情面板切换', desc: '显示/隐藏详情', icon: 'eye' },
      securityReport: { label: '安全报告', desc: '安全报告', icon: 'shield' },
      similarPasswords: { label: '相似密码检查', desc: '相似密码检查', icon: 'shield' },
      hibpCheck: { label: 'HIBP 泄露检查', desc: 'HIBP 泄露检查', icon: 'globe' },
      expiredEntries: { label: '过期条目', desc: '过期条目', icon: 'clock' },
      clearHistory: { label: '清理全部历史', desc: '清理全部历史', icon: 'trash' },
      importMenu: { label: '导入', desc: '导入子菜单', icon: 'upload' },
      exportMenu: { label: '导出', desc: '导出', icon: 'download' },
      dbSettings: { label: '数据库设置', desc: '数据库设置', icon: 'settings' },
      appSettings: { label: '设置', desc: '设置', icon: 'settings' },
      moreMenu: { label: '更多菜单 ```', desc: '溢出菜单（不可隐藏）', icon: 'more-horizontal' },
      windowMinimize: { label: '窗口：最小化 —', desc: '最小化按钮', icon: 'minimize' },
      windowMaximize: { label: '窗口：最大化/还原 □', desc: '最大化/还原按钮', icon: 'maximize' },
      windowClose: { label: '窗口：关闭 ×', desc: '关闭按钮', icon: 'x' },
    }}
    {@const fullOrder: string[] = (s.general as unknown as Record<string, unknown>).toolbarFullOrder as string[] ?? []}
    {@const fullSeparators: string[] = (s.general as unknown as Record<string, unknown>).toolbarFullSeparators as string[] ?? []}
    {@const sides: Record<string, string> = (s.general as unknown as Record<string, unknown>).toolbarSides as Record<string, string> ?? {}}
    <section class="setting-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="sliders" size={17} /></span>
        <div>
          <strong>工具栏排序、左右与分隔</strong>
          <p>拖拽整行或用 ↑↓ 调整全局顺序；“左/右”切换所在分组；“|”为该项后添加分割线</p>
        </div>
      </div>
      <div class="toolbar-order-list" role="list">
        {#each fullOrder as id, idx (id)}
          {@const meta = fullMeta[id] ?? { label: id, desc: "", icon: "settings" }}
          {@const isMore = id === "moreMenu"}
          {@const visible = isMore
            ? true
            : ((s.general.toolbarItems as unknown as Record<string, boolean>)[id] ?? true)}
          {@const hasSep = fullSeparators.includes(id)}
          {@const side =
            sides[id] ?? (["newEntry", "save", "saveAs", "lock"].includes(id) ? "left" : "right")}
          <div
            class="toolbar-order-item"
            class:dragging={draggedId === id}
            draggable="true"
            role="listitem"
            ondragstart={(e) => {
              draggedId = id;
              e.dataTransfer?.setData("text/plain", id);
            }}
            ondragend={() => {
              draggedId = null;
            }}
            ondragover={(e) => e.preventDefault()}
            ondrop={(e) => {
              e.preventDefault();
              const from = draggedId;
              if (!from || from === id) return;
              const next = [...fullOrder];
              const fi = next.indexOf(from);
              const ti = next.indexOf(id);
              if (fi === -1 || ti === -1) return;
              next.splice(fi, 1);
              next.splice(ti, 0, from);
              draggedId = null;
              change("toolbarFullOrder" as never, next as never);
            }}
          >
            <span class="drag-handle" title="拖拽排序"><AppIcon name="menu" size={12} /></span>
            <span class="setting-icon small"><AppIcon name={meta.icon as never} size={14} /></span>
            <div class="order-text">
              <strong>{meta.label}</strong>
              <p>{meta.desc} · {side === "left" ? "左侧" : "右侧"}</p>
            </div>
            <div class="order-actions">
              {#if !isMore}
                <span class="order-action-label">显示</span>
                <Toggle
                  checked={!!visible}
                  ariaLabel={meta.label}
                  onchange={(c) =>
                    change(
                      "toolbarItems" as never,
                      { ...s.general.toolbarItems, [id]: c } as never,
                    )}
                />
              {:else}
                <span class="order-action-label" style="opacity:0.6">固定</span>
                <Toggle checked={true} disabled={true} ariaLabel={meta.label} />
              {/if}
              <span class="order-sep-label" title="在该项后显示分割线">|</span>
              <Toggle
                checked={hasSep}
                ariaLabel="分割线"
                onchange={(c) => {
                  const next = c ? [...fullSeparators, id] : fullSeparators.filter((x) => x !== id);
                  change("toolbarFullSeparators" as never, next as never);
                }}
              />
              <span class="order-sep-label" title="所在侧">侧</span>
              <button
                class="order-side-btn"
                class:active={side === "left"}
                onclick={() => {
                  const next = {
                    ...((s.general as unknown as Record<string, unknown>).toolbarSides as Record<
                      string,
                      string
                    >),
                    [id]: "left",
                  };
                  change("toolbarSides" as never, next as never);
                }}
                title="移至左侧"
                aria-label="左侧">左</button
              >
              <button
                class="order-side-btn"
                class:active={side === "right"}
                onclick={() => {
                  const next = {
                    ...((s.general as unknown as Record<string, unknown>).toolbarSides as Record<
                      string,
                      string
                    >),
                    [id]: "right",
                  };
                  change("toolbarSides" as never, next as never);
                }}
                title="移至右侧"
                aria-label="右侧">右</button
              >
              <div class="order-move">
                <button
                  class="order-move-btn"
                  disabled={idx === 0}
                  onclick={() => {
                    if (idx === 0) return;
                    const next = [...fullOrder];
                    [next[idx - 1], next[idx]] = [next[idx], next[idx - 1]];
                    change("toolbarFullOrder" as never, next as never);
                  }}
                  aria-label="上移">↑</button
                >
                <button
                  class="order-move-btn"
                  disabled={idx === fullOrder.length - 1}
                  onclick={() => {
                    if (idx === fullOrder.length - 1) return;
                    const next = [...fullOrder];
                    [next[idx], next[idx + 1]] = [next[idx + 1], next[idx]];
                    change("toolbarFullOrder" as never, next as never);
                  }}
                  aria-label="下移">↓</button
                >
              </div>
            </div>
          </div>
        {/each}
      </div>
      <div class="toolbar-order-footer">
        <Button
          variant="plain"
          onclick={() => {
            change(
              "toolbarFullOrder" as never,
              [
                ...([
                  "newEntry",
                  "save",
                  "saveAs",
                  "lock",
                  "toggleDetail",
                  "securityReport",
                  "similarPasswords",
                  "hibpCheck",
                  "expiredEntries",
                  "clearHistory",
                  "importMenu",
                  "exportMenu",
                  "dbSettings",
                  "appSettings",
                  "moreMenu",
                  "windowMinimize",
                  "windowMaximize",
                  "windowClose",
                ] as unknown as string[]),
              ] as never,
            );
            change(
              "toolbarSides" as never,
              {
                newEntry: "left",
                save: "left",
                saveAs: "left",
                lock: "left",
                toggleDetail: "right",
                securityReport: "right",
                similarPasswords: "right",
                hibpCheck: "right",
                expiredEntries: "right",
                clearHistory: "right",
                importMenu: "right",
                exportMenu: "right",
                dbSettings: "right",
                appSettings: "right",
                moreMenu: "right",
                windowMinimize: "right",
                windowMaximize: "right",
                windowClose: "right",
              } as never,
            );
          }}>重置排序与左右</Button
        >
        <Button
          variant="plain"
          onclick={() => change("toolbarFullSeparators" as never, [] as never)}
          >清除全部分割线</Button
        >
      </div>
    </section>
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

  .multi-theme-heading {
    margin-top: 14px;
  }

  .theme-config-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .theme-name-row {
    margin-top: 8px;
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

  .toolbar-order-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 10px;
  }
  .toolbar-order-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--border-color);
    border-radius: var(--settings-control-radius, 6px);
    background: var(--card-bg);
  }
  .toolbar-order-item.dragging {
    opacity: 0.5;
  }
  .drag-handle {
    display: inline-flex;
    cursor: grab;
    color: var(--text-faint);
  }
  .drag-handle:active {
    cursor: grabbing;
  }
  .setting-icon.small {
    width: 22px;
    height: 22px;
  }
  .order-text {
    flex: 1;
    min-width: 0;
  }
  .order-text strong {
    display: block;
    font-size: var(--settings-control-size);
    color: var(--text-primary);
  }
  .order-text p {
    margin: 1px 0 0;
    font-size: var(--settings-note-size);
    color: var(--text-muted);
  }
  .order-actions {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .order-action-label,
  .order-sep-label {
    font-size: var(--settings-note-size);
    color: var(--text-muted);
  }
  .order-sep-label {
    margin-left: 4px;
    font-weight: 700;
  }
  .order-move {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-left: 4px;
  }
  .order-move-btn {
    width: 22px;
    height: 16px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid var(--border-color);
    border-radius: 4px;
    background: var(--input-bg);
    color: var(--text-muted);
    font-size: 10px;
    cursor: pointer;
  }
  .order-move-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .order-move-btn:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--hover-bg);
  }
  .toolbar-order-footer {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }
  .order-side-btn {
    min-width: 22px;
    height: 20px;
    padding: 0 4px;
    border: 1px solid var(--border-color);
    border-radius: 4px;
    background: var(--input-bg);
    color: var(--text-muted);
    font-size: 10px;
    cursor: pointer;
  }
  .order-side-btn.active {
    border-color: var(--selection-color);
    color: var(--selection-color);
    background: color-mix(in srgb, var(--selection-color) 15%, var(--input-bg));
  }
  .order-side-btn:hover {
    color: var(--text-primary);
    background: var(--hover-bg);
  }
</style>
