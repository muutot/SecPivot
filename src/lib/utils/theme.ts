import type { ThemeColors } from "$lib/types/theme";
import { DARK_THEME_COLORS } from "$lib/types/theme";

export const COLOR_CSS_MAP: Record<keyof ThemeColors, string> = {
  bg: "--bg-app",
  settingsBg: "--bg-settings",
  accent: "--accent",
  textPrimary: "--text-primary",
  textMuted: "--text-muted",
  border: "--border-color",
  cardBg: "--card-bg",
  surfaceBg: "--surface-bg",
  statusBarBg: "--statusbar-bg",
  hoverBg: "--hover-bg",
  inputBg: "--input-bg",
  textSecondary: "--text-secondary",
  textFaint: "--text-faint",
  placeholderColor: "--placeholder-color",
  borderSubtle: "--border-subtle",
  selectionColor: "--selection-color",
  successColor: "--success-color",
  dangerColor: "--danger-color",
  warningColor: "--warning-color",
  scrollbarColor: "--scrollbar-color",
};

export function applyThemeColors(colors: ThemeColors = DARK_THEME_COLORS): void {
  for (const key of Object.keys(COLOR_CSS_MAP) as (keyof ThemeColors)[]) {
    document.documentElement.style.setProperty(COLOR_CSS_MAP[key], colors[key]);
  }
}
