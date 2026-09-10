<script lang="ts">
  import { appSettings, isHexColor } from "$lib/services/settings";
  import { t } from "$lib/i18n";
  import type { GeneralSettings, WindowEffect } from "$lib/types/settings";
  import AppIcon from "$lib/components/AppIcon.svelte";
  import ModalShell from "$lib/components/ModalShell.svelte";
  import Select from "$lib/components/Select.svelte";
  import SettingRangeCard from "$lib/components/settings/SettingRangeCard.svelte";
  import SettingToggleCard from "$lib/components/settings/SettingToggleCard.svelte";
  import TextField from "$lib/components/templates/form/TextField.svelte";
  import Toggle from "$lib/components/templates/form/Toggle.svelte";
  import Button from "$lib/components/templates/action/Button.svelte";
  import { DARK_THEME_COLORS, LIGHT_THEME_COLORS, type ThemeColors } from "$lib/types/theme";

  type Section = "appearance" | "display" | "layout" | "toolbar" | "network";

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
  const lang = $derived(s.general.language);

  let draggedId: string | null = $state(null);
  let selectedThemeIdx: number | null = $state(null);
  let themeDialogMode: "create" | "rename" | null = $state(null);
  let themeDialogName = $state("");
  let themeDialogError = $state("");
  let headingEl = $state<HTMLDivElement | null>(null);
  let titleEl = $state<HTMLDivElement | null>(null);
  let actionsEl = $state<HTMLDivElement | null>(null);
  let actionsWrapped = $state(false);

  $effect(() => {
    const heading = headingEl;
    const title = titleEl;
    const actions = actionsEl;
    if (!heading || !title || !actions) return;
    const measure = () => {
      actionsWrapped =
        actions.getBoundingClientRect().top > title.getBoundingClientRect().bottom + 2;
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(heading);
    ro.observe(actions);
    return () => ro.disconnect();
  });

  function change<K extends keyof GeneralSettings>(key: K, value: GeneralSettings[K]): void {
    appSettings.updateGeneral(key, value);
  }

  /** Hex draft for the separator color field: invalid text stays visible with
   *  a warning but never reaches the store, so a partial `#ab` can neither
   *  persist nor silently reset on restart. */
  let separatorColorDraft = $state<string | null>(null);
  /** Text shown in the hex field: the draft while editing, else the stored value. */
  const separatorColorShown = $derived(separatorColorDraft ?? general.groupSeparatorColor ?? "");
  const separatorColorInvalid = $derived(
    separatorColorDraft !== null && separatorColorDraft !== "" && !isHexColor(separatorColorDraft),
  );

  /** Keep invalid text in the field but persist only empty/valid values. */
  function commitSeparatorColor(raw: string): void {
    separatorColorDraft = raw;
    if (raw === "" || isHexColor(raw)) {
      change("groupSeparatorColor", raw);
    }
  }

  function clearSeparatorColor(): void {
    separatorColorDraft = null;
    change("groupSeparatorColor", "");
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

  function openSaveThemeDialog(): void {
    const base = `${t(lang, "settings.customTheme")} ${s.general.customThemes.length + 1}`;
    let name = base;
    let n = 2;
    const names = new Set(s.general.customThemes.map((t) => t.name));
    while (names.has(name)) {
      name = `${base} ${n}`;
      n++;
    }
    themeDialogName = name;
    themeDialogError = "";
    themeDialogMode = "create";
  }

  function openRenameThemeDialog(): void {
    if (selectedThemeIdx === null) return;
    const cur = s.general.customThemes[selectedThemeIdx];
    if (!cur) return;
    themeDialogName = cur.name;
    themeDialogError = "";
    themeDialogMode = "rename";
  }

  function confirmThemeDialog(): void {
    const raw = themeDialogName.trim();
    if (themeDialogMode === "create") {
      const base = raw || `${t(lang, "settings.customTheme")} ${s.general.customThemes.length + 1}`;
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
      themeDialogMode = null;
      return;
    }
    if (themeDialogMode === "rename") {
      if (selectedThemeIdx === null) return;
      if (!raw) {
        themeDialogError = t(lang, "settings.themeNameRequired");
        return;
      }
      const themes = s.general.customThemes.map((t) => ({ ...t, colors: { ...t.colors } }));
      const target = themes[selectedThemeIdx];
      if (!target) return;
      let name = raw;
      let n = 2;
      const otherNames = new Set(
        themes.filter((_, i) => i !== selectedThemeIdx).map((t) => t.name),
      );
      while (otherNames.has(name)) {
        name = `${raw} ${n}`;
        n++;
      }
      target.name = name;
      appSettings.updateGeneral("customThemes", themes);
      themeDialogMode = null;
    }
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
  }[] = $derived([
    {
      key: "base",
      label: t(lang, "settings.fontBase"),
      description: t(lang, "settings.fontBaseDesc"),
      min: 11,
      max: 20,
    },
    {
      key: "secondary",
      label: t(lang, "settings.fontSecondary"),
      description: t(lang, "settings.fontSecondaryDesc"),
      min: 9,
      max: 16,
    },
    {
      key: "cardTitle",
      label: t(lang, "settings.fontCardTitle"),
      description: t(lang, "settings.fontCardTitleDesc"),
      min: 11,
      max: 18,
    },
  ]);

  const densitySliders: {
    key: "entryRowHeight" | "groupGap" | "groupPaddingY" | "groupIndent" | "groupRadius";
    label: string;
    description: string;
    min: number;
    max: number;
  }[] = $derived([
    {
      key: "entryRowHeight",
      label: t(lang, "settings.rowHeight"),
      description: t(lang, "settings.rowHeightDesc"),
      min: 24,
      max: 72,
    },
    {
      key: "groupGap",
      label: t(lang, "settings.groupGap"),
      description: t(lang, "settings.groupGapDesc"),
      min: 0,
      max: 16,
    },
    {
      key: "groupPaddingY",
      label: t(lang, "settings.groupPaddingY"),
      description: t(lang, "settings.groupPaddingYDesc"),
      min: 0,
      max: 16,
    },
    {
      key: "groupIndent",
      label: t(lang, "settings.groupIndent"),
      description: t(lang, "settings.groupIndentDesc"),
      min: 4,
      max: 32,
    },
    {
      key: "groupRadius",
      label: t(lang, "settings.groupRadius"),
      description: t(lang, "settings.groupRadiusDesc"),
      min: 0,
      max: 12,
    },
  ]);

  const customColorGroups: {
    label: string;
    fields: { key: keyof ThemeColors; label: string; description: string }[];
  }[] = $derived([
    {
      label: t(lang, "settings.themeBase"),
      fields: [
        {
          key: "accent",
          label: t(lang, "settings.colorAccent"),
          description: t(lang, "settings.colorAccentDesc"),
        },
        {
          key: "selectionColor",
          label: t(lang, "settings.colorSelection"),
          description: t(lang, "settings.colorSelectionDesc"),
        },
        {
          key: "linkColor",
          label: t(lang, "settings.colorLink"),
          description: t(lang, "settings.colorLinkDesc"),
        },
      ],
    },
    {
      label: t(lang, "settings.themeText"),
      fields: [
        {
          key: "textPrimary",
          label: t(lang, "settings.colorTextPrimary"),
          description: t(lang, "settings.colorTextPrimaryDesc"),
        },
        {
          key: "textSecondary",
          label: t(lang, "settings.colorTextSecondary"),
          description: t(lang, "settings.colorTextSecondaryDesc"),
        },
        {
          key: "textMuted",
          label: t(lang, "settings.colorTextMuted"),
          description: t(lang, "settings.colorTextMutedDesc"),
        },
        {
          key: "textFaint",
          label: t(lang, "settings.colorTextFaint"),
          description: t(lang, "settings.colorTextFaintDesc"),
        },
        {
          key: "placeholderColor",
          label: t(lang, "settings.colorPlaceholder"),
          description: t(lang, "settings.colorPlaceholderDesc"),
        },
      ],
    },
    {
      label: t(lang, "settings.themeSurface"),
      fields: [
        {
          key: "bg",
          label: t(lang, "settings.colorBg"),
          description: t(lang, "settings.colorBgDesc"),
        },
        {
          key: "settingsBg",
          label: t(lang, "settings.colorSettingsBg"),
          description: t(lang, "settings.colorSettingsBgDesc"),
        },
        {
          key: "cardBg",
          label: t(lang, "settings.colorCardBg"),
          description: t(lang, "settings.colorCardBgDesc"),
        },
        {
          key: "surfaceBg",
          label: t(lang, "settings.colorSurfaceBg"),
          description: t(lang, "settings.colorSurfaceBgDesc"),
        },
        {
          key: "statusBarBg",
          label: t(lang, "settings.colorStatusBarBg"),
          description: t(lang, "settings.colorStatusBarBgDesc"),
        },
        {
          key: "hoverBg",
          label: t(lang, "settings.colorHoverBg"),
          description: t(lang, "settings.colorHoverBgDesc"),
        },
        {
          key: "inputBg",
          label: t(lang, "settings.colorInputBg"),
          description: t(lang, "settings.colorInputBgDesc"),
        },
      ],
    },
    {
      label: t(lang, "settings.themeBorder"),
      fields: [
        {
          key: "border",
          label: t(lang, "settings.colorBorder"),
          description: t(lang, "settings.colorBorderDesc"),
        },
        {
          key: "borderSubtle",
          label: t(lang, "settings.colorBorderSubtle"),
          description: t(lang, "settings.colorBorderSubtleDesc"),
        },
        {
          key: "scrollbarColor",
          label: t(lang, "settings.colorScrollbar"),
          description: t(lang, "settings.colorScrollbarDesc"),
        },
      ],
    },
    {
      label: t(lang, "settings.themeStatus"),
      fields: [
        {
          key: "successColor",
          label: t(lang, "settings.colorSuccess"),
          description: t(lang, "settings.colorSuccessDesc"),
        },
        {
          key: "dangerColor",
          label: t(lang, "settings.colorDanger"),
          description: t(lang, "settings.colorDangerDesc"),
        },
        {
          key: "warningColor",
          label: t(lang, "settings.colorWarning"),
          description: t(lang, "settings.colorWarningDesc"),
        },
      ],
    },
  ]);
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
      <span class="eyebrow">Settings · {t(lang, "settings.generalTitle")}</span>
      <h2>{t(lang, "settings.generalTitle")}</h2>
      <p>{t(lang, "settings.generalDesc")}</p>
    </div>
    <button class="close-button" onclick={onclose} aria-label={t(lang, "common.close")}>×</button>
  </header>
{/if}

<div class="settings-scroll">
  {#if section === "appearance"}
    <section class="setting-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="palette" size={17} /></span>
        <div class="heading-inline">
          <div>
            <strong>{t(lang, "settings.themeMode")}</strong>
            <p>{t(lang, "settings.themeModeDesc")}</p>
          </div>
        </div>
      </div>
      <div class="theme-segmented" role="group" aria-label={t(lang, "settings.themeMode")}>
        <button
          class="theme-segment"
          class:active={general.theme === "dark"}
          onclick={() => change("theme", "dark")}
        >
          <span class="swatch swatch-dark"></span>{t(lang, "settings.themeDark")}
        </button>
        <button
          class="theme-segment"
          class:active={general.theme === "light"}
          onclick={() => change("theme", "light")}
        >
          <span class="swatch swatch-light"></span>{t(lang, "settings.themeLight")}
        </button>
        <button
          class="theme-segment"
          class:active={general.theme === "custom"}
          onclick={() => change("theme", "custom")}
        >
          <span class="swatch swatch-custom"></span>{t(lang, "settings.themeCustom")}
        </button>
      </div>
    </section>

    <section class="setting-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="globe" size={17} /></span>
        <div class="heading-inline">
          <div>
            <strong>{t(lang, "settings.language.title")}</strong>
            <p>{t(lang, "settings.language.description")}</p>
          </div>
        </div>
      </div>
      <div class="theme-segmented" role="group" aria-label={t(lang, "settings.language.title")}>
        <button
          class="theme-segment"
          class:active={general.language === "zh-CN"}
          onclick={() => change("language", "zh-CN")}
        >
          {t(lang, "settings.language.zh")}
        </button>
        <button
          class="theme-segment"
          class:active={general.language === "en"}
          onclick={() => change("language", "en")}
        >
          {t(lang, "settings.language.en")}
        </button>
      </div>
    </section>

    {#if general.theme === "custom"}
      <section class="setting-card">
        <div class="setting-heading" class:wrapped-heading={actionsWrapped}>
          <span class="setting-icon"><AppIcon name="settings" size={17} /></span>
          <div
            class="heading-inline custom-heading"
            class:wrapped={actionsWrapped}
            bind:this={headingEl}
          >
            <div bind:this={titleEl}>
              <strong>{t(lang, "settings.customPalette")}</strong>
              <p>{t(lang, "settings.customPaletteDesc")}</p>
            </div>
            <div class="custom-actions">
              <div class="preset-row">
                <Button
                  variant="action"
                  title={t(lang, "settings.presetDark")}
                  ariaLabel={t(lang, "settings.presetDark")}
                  onclick={() => applyPreset(DARK_THEME_COLORS)}
                  ><AppIcon name="moon" size={15} /></Button
                >
                <Button
                  variant="action"
                  title={t(lang, "settings.presetLight")}
                  ariaLabel={t(lang, "settings.presetLight")}
                  onclick={() => applyPreset(LIGHT_THEME_COLORS)}
                  ><AppIcon name="sun" size={15} /></Button
                >
                <Button
                  variant="action"
                  title={t(lang, "settings.restoreDefaultColors")}
                  ariaLabel={t(lang, "settings.restoreDefaultColors")}
                  onclick={() => applyPreset(LIGHT_THEME_COLORS)}
                  ><AppIcon name="undo" size={15} /></Button
                >
                <Button
                  variant="action"
                  title={t(lang, "settings.saveTheme")}
                  ariaLabel={t(lang, "settings.saveTheme")}
                  onclick={openSaveThemeDialog}><AppIcon name="save" size={15} /></Button
                >
                {#if selectedThemeIdx !== null}
                  <Button
                    variant="action"
                    title={t(lang, "settings.renameTheme")}
                    ariaLabel={t(lang, "settings.renameTheme")}
                    onclick={openRenameThemeDialog}><AppIcon name="edit" size={15} /></Button
                  >
                  <Button
                    variant="action"
                    title={t(lang, "settings.applyTheme")}
                    ariaLabel={t(lang, "settings.applyTheme")}
                    onclick={() => {
                      if (selectedThemeIdx !== null) applyCustomTheme(selectedThemeIdx);
                    }}><AppIcon name="check" size={15} /></Button
                  >
                  <Button
                    variant="action"
                    title={t(lang, "settings.deleteTheme")}
                    ariaLabel={t(lang, "settings.deleteTheme")}
                    onclick={() => {
                      if (selectedThemeIdx !== null) deleteCustomTheme(selectedThemeIdx);
                    }}><AppIcon name="trash" size={15} /></Button
                  >
                {/if}
              </div>
              <Select
                value={selectedThemeIdx === null ? "current" : String(selectedThemeIdx)}
                ariaLabel={t(lang, "settings.selectTheme")}
                options={[
                  { value: "current", label: t(lang, "settings.currentColors") },
                  ...s.general.customThemes.map((t, i) => ({ value: String(i), label: t.name })),
                ]}
                onchange={(v) => {
                  selectedThemeIdx = v === "current" ? null : Number(v);
                }}
              />
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
      </section>
      {#if themeDialogMode !== null}
        <ModalShell
          title={themeDialogMode === "create"
            ? t(lang, "settings.saveTheme")
            : t(lang, "settings.renameTheme")}
          description={themeDialogMode === "create"
            ? t(lang, "settings.themeDialogCopyHint")
            : (s.general.customThemes[selectedThemeIdx ?? -1]?.name ?? "")}
          size="small"
          closeOnEscape
          onclose={() => {
            themeDialogMode = null;
            themeDialogError = "";
          }}
        >
          {#snippet children()}
            <div class="theme-dialog-body">
              <TextField
                bind:value={themeDialogName}
                placeholder={t(lang, "settings.themeNamePh")}
                autofocus
                oninput={() => {
                  themeDialogError = "";
                }}
                onkeydown={(e) => {
                  if (e.key === "Enter") confirmThemeDialog();
                }}
              />
              {#if themeDialogError}<p class="theme-dialog-error">{themeDialogError}</p>{/if}
            </div>
          {/snippet}
          {#snippet actions()}
            <Button
              variant="plain"
              onclick={() => {
                themeDialogMode = null;
                themeDialogError = "";
              }}>{t(lang, "common.cancel")}</Button
            >
            <Button variant="primary" onclick={confirmThemeDialog}
              >{themeDialogMode === "create"
                ? t(lang, "settings.addTheme")
                : t(lang, "common.save")}</Button
            >
          {/snippet}
        </ModalShell>
      {/if}
    {:else if general.theme === "dark"}
      {@render presetPaletteCard(
        t(lang, "settings.darkPalette"),
        t(lang, "settings.builtinReadonly"),
        DARK_THEME_COLORS,
      )}
    {:else if general.theme === "light"}
      {@render presetPaletteCard(
        t(lang, "settings.lightPalette"),
        t(lang, "settings.builtinReadonly"),
        LIGHT_THEME_COLORS,
      )}
    {/if}

    {#if general.showGroupSeparators ?? true}
      <section class="setting-card">
        <div class="setting-row">
          <div class="setting-heading">
            <span class="setting-icon"><AppIcon name="palette" size={17} /></span>
            <div>
              <strong>{t(lang, "settings.separatorColor")}</strong>
              <p>{t(lang, "settings.separatorColorDesc")}</p>
            </div>
          </div>
          <div class="color-control">
            <input
              type="color"
              class="color-input"
              aria-label={t(lang, "settings.separatorColor")}
              title={t(lang, "settings.separatorColor")}
              value={general.groupSeparatorColor && general.groupSeparatorColor.length >= 7
                ? general.groupSeparatorColor.slice(0, 7)
                : "#999999"}
              oninput={(e) => commitSeparatorColor(e.currentTarget.value)}
            />
            <div class="color-hex-input">
              <TextField
                size="control"
                spellcheck={false}
                value={separatorColorShown}
                placeholder={t(lang, "settings.leaveDefault")}
                ariaLabel={t(lang, "settings.separatorColor")}
                invalid={separatorColorInvalid}
                oninput={(e) => commitSeparatorColor(e.currentTarget.value)}
              />
            </div>
            {#if general.groupSeparatorColor}
              <Button
                variant="action"
                title={t(lang, "settings.clearCustomColor")}
                ariaLabel={t(lang, "common.clear")}
                onclick={clearSeparatorColor}>{t(lang, "common.clear")}</Button
              >
            {/if}
          </div>
        </div>
        {#if separatorColorInvalid}
          <p class="settings-note warn">{t(lang, "settings.invalidHexColor")}</p>
        {/if}
      </section>
    {/if}
  {/if}

  {#if section === "display"}
    <SettingToggleCard
      icon="eye"
      label={t(lang, "settings.showDescriptions")}
      description={t(lang, "settings.showDescriptionsDesc")}
      checked={general.showDescriptions}
      ariaLabel={t(lang, "settings.showDescriptionsAria")}
      onchange={(checked) => change("showDescriptions", checked)}
    />

    <SettingToggleCard
      icon="eye"
      label={t(lang, "settings.iconOnly")}
      description={t(lang, "settings.iconOnlyDesc")}
      checked={general.iconOnlyButtons}
      onchange={(checked) => change("iconOnlyButtons", checked)}
    />

    <SettingToggleCard
      icon="folder"
      label={t(lang, "settings.showGroupIcon")}
      description={t(lang, "settings.showGroupIconDesc")}
      checked={s.general.density.showGroupIcon}
      onchange={(checked) =>
        change("density", {
          ...s.general.density,
          showGroupIcon: checked,
        })}
    />

    <SettingToggleCard
      icon="chevron-down"
      label={t(lang, "settings.showGroupChevron")}
      description={t(lang, "settings.showGroupChevronDesc")}
      checked={s.general.density.showGroupChevron}
      onchange={(checked) =>
        change("density", {
          ...s.general.density,
          showGroupChevron: checked,
        })}
    />

    <SettingToggleCard
      icon="grid"
      label={t(lang, "settings.mobileColumns")}
      description={t(lang, "settings.mobileColumnsDesc")}
      checked={s.general.mobileColumns}
      ariaLabel={t(lang, "settings.mobileColumns")}
      onchange={(checked) => change("mobileColumns", checked)}
    />

    <SettingToggleCard
      icon="grid"
      label={t(lang, "settings.showSeparators")}
      description={t(lang, "settings.showSeparatorsDesc")}
      checked={general.showGroupSeparators ?? true}
      ariaLabel={t(lang, "settings.showSeparators")}
      onchange={(checked) => change("showGroupSeparators", checked)}
    />

    <SettingRangeCard
      icon="sliders"
      label={t(lang, "settings.windowOpacity")}
      description={t(lang, "settings.windowOpacityDesc")}
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
            <strong>{t(lang, "settings.windowEffect")}</strong>
            <p>{t(lang, "settings.windowEffectDesc")}</p>
          </div>
          <Select
            value={general.windowEffect}
            options={[
              { value: "off", label: t(lang, "settings.effectOff") },
              { value: "acrylic", label: t(lang, "settings.effectAcrylic") },
              { value: "mica", label: t(lang, "settings.effectMica") },
            ]}
            onchange={(v) => change("windowEffect", v as WindowEffect)}
          />
        </div>
      </div>
    </section>

    <SettingToggleCard
      icon="folder"
      label={t(lang, "settings.rememberLast")}
      description={t(lang, "settings.rememberLastDesc")}
      checked={general.rememberLastDatabase}
      onchange={(checked) => change("rememberLastDatabase", checked)}
    />
  {/if}

  {#if section === "layout"}
    <p
      class="settings-note"
      style="margin: 0 0 8px; color: var(--text-muted); font-size: var(--settings-description-size);"
    >
      {t(lang, "settings.layoutDensity")}
    </p>

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
      {t(lang, "settings.toolbarHelp")}
    </p>
    <SettingToggleCard
      icon="eye"
      label={t(lang, "settings.detailToggle")}
      description={t(lang, "settings.detailToggleDesc")}
      checked={s.general.showDetailOnSelect ?? true}
      onchange={(checked) => change("showDetailOnSelect", checked)}
    />
    {@const fullMeta: Record<string, { label: string; desc: string; icon: string }> = {
      newEntry: { label: t(lang, "menu.newEntry"), desc: t(lang, "menu.newEntry"), icon: "plus" },
      save: { label: t(lang, "toolbar.saveLabel"), desc: t(lang, "toolbar.save"), icon: "save" },
      saveAs: { label: t(lang, "toolbar.saveAsLabel"), desc: t(lang, "toolbar.saveAs"), icon: "copy" },
      lock: { label: t(lang, "toolbar.lockLabel"), desc: t(lang, "toolbar.lock"), icon: "lock" },
      toggleDetail: {
        label: t(lang, "settings.toggleDetailName"),
        desc: t(lang, "settings.toggleDetailDesc"),
        icon: "eye",
      },
      securityReport: {
        label: t(lang, "menu.securityReport"),
        desc: t(lang, "menu.securityReport"),
        icon: "shield",
      },
      similarPasswords: {
        label: t(lang, "menu.similarPasswords"),
        desc: t(lang, "menu.similarPasswords"),
        icon: "shield",
      },
      hibpCheck: { label: t(lang, "menu.hibp"), desc: t(lang, "menu.hibp"), icon: "globe" },
      expiredEntries: { label: t(lang, "menu.expired"), desc: t(lang, "menu.expired"), icon: "clock" },
      clearHistory: {
        label: t(lang, "menu.clearHistory"),
        desc: t(lang, "menu.clearHistory"),
        icon: "trash",
      },
      importMenu: {
        label: t(lang, "menu.import"),
        desc: t(lang, "settings.importMenuDesc"),
        icon: "upload",
      },
      exportMenu: { label: t(lang, "menu.export"), desc: t(lang, "menu.export"), icon: "download" },
      dbSettings: {
        label: t(lang, "menu.dbSettings"),
        desc: t(lang, "menu.dbSettings"),
        icon: "settings",
      },
      appSettings: { label: t(lang, "menu.settings"), desc: t(lang, "menu.settings"), icon: "settings" },
      moreMenu: {
        label: t(lang, "settings.moreMenuName"),
        desc: t(lang, "settings.moreMenuDesc"),
        icon: "more-horizontal",
      },
      windowMinimize: {
        label: t(lang, "settings.winMinimize"),
        desc: t(lang, "settings.winMinimizeDesc"),
        icon: "minimize",
      },
      windowMaximize: {
        label: t(lang, "settings.winMaximize"),
        desc: t(lang, "settings.winMaximizeDesc"),
        icon: "maximize",
      },
      windowClose: {
        label: t(lang, "settings.winClose"),
        desc: t(lang, "settings.winCloseDesc"),
        icon: "x",
      },
    }}
    {@const fullOrder: string[] = (s.general as unknown as Record<string, unknown>).toolbarFullOrder as string[] ?? []}
    {@const fullSeparators: string[] = (s.general as unknown as Record<string, unknown>).toolbarFullSeparators as string[] ?? []}
    {@const sides: Record<string, string> = (s.general as unknown as Record<string, unknown>).toolbarSides as Record<string, string> ?? {}}
    <section class="setting-card">
      <div class="setting-heading">
        <span class="setting-icon"><AppIcon name="sliders" size={17} /></span>
        <div>
          <strong>{t(lang, "settings.toolbarOrder")}</strong>
          <p>{t(lang, "settings.toolbarOrderDesc")}</p>
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
            <span class="drag-handle" title={t(lang, "settings.dragToSort")}
              ><AppIcon name="menu" size={12} /></span
            >
            <span class="setting-icon small"><AppIcon name={meta.icon as never} size={14} /></span>
            <div class="order-text">
              <strong>{meta.label}</strong>
              <p>
                {meta.desc} · {side === "left"
                  ? t(lang, "settings.sideLeft")
                  : t(lang, "settings.sideRight")}
              </p>
            </div>
            <div class="order-actions">
              {#if !isMore}
                <span class="order-action-label">{t(lang, "settings.shown")}</span>
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
                <span class="order-action-label" style="opacity:0.6"
                  >{t(lang, "settings.pinned")}</span
                >
                <Toggle checked={true} disabled={true} ariaLabel={meta.label} />
              {/if}
              <span class="order-sep-label" title={t(lang, "settings.dividerAfter")}>|</span>
              <Toggle
                checked={hasSep}
                ariaLabel={t(lang, "settings.divider")}
                onchange={(c) => {
                  const next = c ? [...fullSeparators, id] : fullSeparators.filter((x) => x !== id);
                  change("toolbarFullSeparators" as never, next as never);
                }}
              />
              <span class="order-sep-label" title={t(lang, "settings.sideTitle")}
                >{t(lang, "settings.sideAbbr")}</span
              >
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
                title={t(lang, "settings.moveLeft")}
                aria-label={t(lang, "settings.left")}>{t(lang, "settings.left")}</button
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
                title={t(lang, "settings.moveRight")}
                aria-label={t(lang, "settings.right")}>{t(lang, "settings.right")}</button
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
                  aria-label={t(lang, "settings.moveUp")}>↑</button
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
                  aria-label={t(lang, "settings.moveDown")}>↓</button
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
          }}>{t(lang, "settings.resetOrder")}</Button
        >
        <Button
          variant="plain"
          onclick={() => change("toolbarFullSeparators" as never, [] as never)}
          >{t(lang, "settings.clearDividers")}</Button
        >
      </div>
    </section>
  {/if}

  {#if section === "network"}
    <SettingToggleCard
      icon="save"
      label={t(lang, "settings.faviconAutosave")}
      description={t(lang, "settings.faviconAutosaveDesc")}
      checked={s.favicon.autoSave}
      onchange={(checked) => appSettings.updateFavicon("autoSave", checked)}
    />
    <SettingRangeCard
      icon="globe"
      label={t(lang, "settings.faviconConcurrency")}
      description={t(lang, "settings.faviconConcurrencyDesc")}
      value={s.favicon.concurrency}
      valueLabel={t(lang, "settings.countItems", { count: s.favicon.concurrency })}
      min={1}
      max={16}
      onchange={(value) => appSettings.updateFavicon("concurrency", value)}
    />
  {/if}

  <p class="auto-save-note">{t(lang, "settings.autoSaveNote")}</p>
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
    flex-direction: row;
    align-items: center;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 6px;
    min-width: 0;
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

  .custom-heading {
    align-items: center;
    flex-wrap: wrap;
    justify-content: flex-end;
  }

  .custom-heading > div:first-child {
    flex: 1 1 180px;
    min-width: 140px;
  }

  .custom-heading.wrapped > div:first-child {
    flex: 0 1 auto;
    min-width: 0;
    margin-right: auto;
  }

  .custom-heading.wrapped > div:first-child p {
    display: none;
  }

  .setting-heading.wrapped-heading {
    align-items: flex-start;
  }

  .custom-actions {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 6px;
    flex: 0 1 auto;
    flex-wrap: wrap;
    justify-content: flex-end;
    min-width: 0;
  }

  .color-list {
    margin-top: 12px;
    padding-top: 12px;
    border-top: 1px solid var(--border-subtle);
  }

  .theme-dialog-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .theme-dialog-error {
    margin: 0;
    color: var(--danger-color);
    font-size: var(--font-size-secondary, 11px);
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
