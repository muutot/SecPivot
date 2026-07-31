import { get } from "svelte/store";
import { appSettings, selectThemeColors } from "$lib/services/settings";
import { applyThemeColors } from "$lib/utils/theme";

export function applySettingsToDocument(): void {
  const s = get(appSettings);
  const colors = selectThemeColors(s);
  applyThemeColors(colors);

  const root = document.documentElement;
  root.style.setProperty("--font-size-base", `${s.general.fontSizes.base}px`);
  root.style.setProperty("--font-size-secondary", `${s.general.fontSizes.secondary}px`);
  root.style.setProperty("--font-size-cardTitle", `${s.general.fontSizes.cardTitle}px`);
  root.style.setProperty("--font-size-cardPreview", `${s.general.fontSizes.cardPreview}px`);
  root.style.setProperty("--settings-heading-size", `var(--font-size-cardTitle, 13px)`);
  root.style.setProperty("--settings-description-size", `var(--font-size-secondary, 11px)`);
  root.style.fontSize = `${s.general.fontSizes.base}px`;
  root.dataset.windowEffect = s.general.windowEffect;

  if (s.general.theme === "light") {
    document.body.style.background =
      "radial-gradient(circle at 50% -20%, rgba(255,255,255,0.4), transparent 38%), var(--bg-app)";
  } else {
    document.body.style.background = "";
  }
}

export function syncCompactShellClass(compact: boolean): void {
  const shell = document.querySelector(".app-shell");
  if (shell) shell.classList.toggle("compact", compact);
}
