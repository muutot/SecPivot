<script lang="ts">
  import { appSettings } from "$lib/services/settings";
  import type { GeneralSettings, WindowEffect } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import Select from "$lib/components/Select.svelte";
  import { DARK_THEME_COLORS, LIGHT_THEME_COLORS, type ThemeColors } from "$lib/types/theme";

  type Section = "appearance" | "display" | "compact";

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

  function sliderPercentage(value: number, min: number, max: number): number {
    return Math.round(((value - min) / (max - min)) * 100);
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

  const customColorFields: { key: keyof ThemeColors; label: string; description: string }[] = [
    { key: "accent", label: "强调色", description: "主按钮与高亮" },
    { key: "selectionColor", label: "选中色", description: "选中项与焦点边框" },
    { key: "bg", label: "背景", description: "窗口与整体底色" },
    { key: "cardBg", label: "卡片", description: "卡片与面板底色" },
    { key: "border", label: "边框", description: "分隔线与控件描边" },
  ];
</script>

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
          {#each customColorFields as field (field.key)}
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
                <input
                  class="settings-input color-hex-input"
                  type="text"
                  spellcheck="false"
                  value={s.general.themeColors[field.key]}
                  placeholder="#RRGGBBAA"
                  oninput={(e) => updateColor(field.key, e.currentTarget.value)}
                />
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}
  {/if}

  {#if section === "display"}
    <section class="setting-card toggle-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="eye" size={17} /></span>
        <div>
          <strong>显示条目描述</strong>
          <p>在列表中展示用户名与备注预览</p>
        </div>
      </div>
      <button
        class="toggle-switch"
        class:active={general.showDescriptions}
        role="switch"
        aria-checked={general.showDescriptions}
        aria-label="显示描述"
        onclick={() => change("showDescriptions", !general.showDescriptions)}
      >
        <span class="toggle-knob"></span>
      </button>
    </section>

    <section class="setting-card toggle-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="eye" size={17} /></span>
        <div>
          <strong>工具栏仅显示图标</strong>
          <p>控制按钮只显示图标，按钮名称在鼠标悬浮时提示</p>
        </div>
      </div>
      <button
        class="toggle-switch"
        class:active={general.iconOnlyButtons}
        role="switch"
        aria-checked={general.iconOnlyButtons}
        aria-label="工具栏仅显示图标"
        onclick={() => change("iconOnlyButtons", !general.iconOnlyButtons)}
      >
        <span class="toggle-knob"></span>
      </button>
    </section>

    <section class="setting-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="sliders" size={17} /></span>
        <div class="heading-inline">
          <div>
            <strong>窗口不透明度</strong>
            <p>调整主窗口的整体透明度</p>
          </div>
          <span class="value-label">{s.general.windowOpacity}%</span>
        </div>
      </div>
      <input
        type="range"
        class="transparency-slider"
        min="40"
        max="100"
        value={s.general.windowOpacity}
        style:--slider-pct={sliderPercentage(s.general.windowOpacity, 40, 100)}
        oninput={(e) => change("windowOpacity", Number(e.currentTarget.value))}
      />
    </section>

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

    <section class="setting-card toggle-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="folder" size={17} /></span>
        <div>
          <strong>记住上次数据库</strong>
          <p>启动时自动加载最近打开的数据库</p>
        </div>
      </div>
      <button
        class="toggle-switch"
        class:active={general.rememberLastDatabase}
        role="switch"
        aria-checked={general.rememberLastDatabase}
        aria-label="记住上次数据库"
        onclick={() => change("rememberLastDatabase", !general.rememberLastDatabase)}
      >
        <span class="toggle-knob"></span>
      </button>
    </section>
  {/if}

  {#if section === "compact"}
    <section class="setting-card toggle-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="grid" size={17} /></span>
        <div>
          <strong>紧凑模式</strong>
          <p>缩小间距，提高单屏信息密度</p>
        </div>
      </div>
      <button
        class="toggle-switch"
        class:active={general.compactMode}
        role="switch"
        aria-checked={general.compactMode}
        aria-label="紧凑模式"
        onclick={() => change("compactMode", !general.compactMode)}
      >
        <span class="toggle-knob"></span>
      </button>
    </section>

    {#each densitySliders as slider (slider.key)}
      <section class="setting-card">
        <div class="setting-heading">
          <span class="setting-icon"><AppIcon name="sliders" size={17} /></span>
          <div class="heading-inline">
            <div>
              <strong>{slider.label}</strong>
              <p>{slider.description}</p>
            </div>
            <span class="value-label">{s.general.density[slider.key]}px</span>
          </div>
        </div>
        <input
          type="range"
          class="transparency-slider"
          min={slider.min}
          max={slider.max}
          value={s.general.density[slider.key]}
          style:--slider-pct={sliderPercentage(
            s.general.density[slider.key],
            slider.min,
            slider.max,
          )}
          oninput={(e) =>
            change("density", {
              ...s.general.density,
              [slider.key]: Number((e.currentTarget as HTMLInputElement).value),
            })}
        />
      </section>
    {/each}

    <section class="setting-card toggle-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="folder" size={17} /></span>
        <div>
          <strong>显示分组图标</strong>
          <p>在分组名称前显示文件夹图标</p>
        </div>
      </div>
      <button
        class="toggle-switch"
        class:active={s.general.density.showGroupIcon}
        role="switch"
        aria-checked={s.general.density.showGroupIcon}
        aria-label="显示分组图标"
        onclick={() =>
          change("density", {
            ...s.general.density,
            showGroupIcon: !s.general.density.showGroupIcon,
          })}
      >
        <span class="toggle-knob"></span>
      </button>
    </section>

    <section class="setting-card toggle-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="chevron-down" size={17} /></span>
        <div>
          <strong>显示折叠箭头</strong>
          <p>在可展开分组前显示折叠箭头</p>
        </div>
      </div>
      <button
        class="toggle-switch"
        class:active={s.general.density.showGroupChevron}
        role="switch"
        aria-checked={s.general.density.showGroupChevron}
        aria-label="显示折叠箭头"
        onclick={() =>
          change("density", {
            ...s.general.density,
            showGroupChevron: !s.general.density.showGroupChevron,
          })}
      >
        <span class="toggle-knob"></span>
      </button>
    </section>

    {#each fontSliders as slider (slider.key)}
      <section class="setting-card">
        <div class="setting-heading">
          <span class="setting-icon"><AppIcon name="keyboard" size={17} /></span>
          <div class="heading-inline">
            <div>
              <strong>{slider.label}</strong>
              <p>{slider.description}</p>
            </div>
            <span class="value-label">{s.general.fontSizes[slider.key]}px</span>
          </div>
        </div>
        <input
          type="range"
          class="transparency-slider"
          min={slider.min}
          max={slider.max}
          value={s.general.fontSizes[slider.key]}
          style:--slider-pct={sliderPercentage(
            s.general.fontSizes[slider.key],
            slider.min,
            slider.max,
          )}
          oninput={(e) =>
            change("fontSizes", {
              ...s.general.fontSizes,
              [slider.key]: Number(e.currentTarget.value),
            })}
        />
      </section>
    {/each}
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
