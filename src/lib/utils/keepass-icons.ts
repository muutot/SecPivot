/**
 * Mapping of built-in KeePass icon indices (0-68) to the compact AppIcon
 * glyphs available in this app. Indices without a mapping fall back to the
 * caller-provided default.
 */

export const KEEPASS_ICONS: Record<number, string> = {
  0: "key",
  1: "globe",
  2: "sliders",
  3: "folder",
  4: "open",
  5: "folder-plus",
  6: "database",
  7: "widgets",
  8: "widgets",
  9: "link",
  10: "globe",
  11: "unlock",
  12: "lock",
  13: "shield",
  14: "database",
  15: "file",
  16: "keyboard",
  17: "user",
  18: "user",
  19: "user",
  20: "user",
  21: "user",
  22: "mail",
  23: "key",
  24: "star",
  25: "phone",
  26: "phone",
  27: "database",
  28: "database",
  29: "database",
  30: "database",
  31: "file",
  32: "file",
  33: "file",
  34: "info",
  35: "info",
  36: "refresh",
  37: "download",
  38: "upload",
  39: "cloud",
  40: "shield",
  41: "shield",
  42: "filter",
  43: "search",
  44: "copy",
  45: "check",
  46: "edit",
  47: "trash",
  48: "x",
  49: "grid",
  50: "grid",
  51: "palette",
  52: "clock",
  53: "clock",
  54: "clock",
  55: "key",
  56: "star",
  57: "file",
  58: "lock",
  59: "shield",
  60: "mail",
  61: "globe",
  62: "globe",
  63: "globe",
  64: "phone",
  65: "phone",
  66: "keyboard",
  67: "keyboard",
  68: "user",
};

export const ENTRY_DEFAULT_ICON = "key";
export const GROUP_DEFAULT_ICON = "folder";

/** Small curated palette offered by the color picker (KeePass-inspired). */
export const KEEPASS_COLORS: string[] = [
  "#FF0000",
  "#FF6600",
  "#FFFF00",
  "#00FF00",
  "#00BFFF",
  "#0000FF",
  "#8000FF",
  "#FF00FF",
  "#FFC0CB",
  "#808080",
  "#000000",
];

/**
 * Icon indices offered by the built-in picker. `KEEPASS_ICONS` maps several
 * KeePass indices to the same compact glyph, so the picker shows each distinct
 * glyph exactly once (first occurrence wins) instead of a grid full of
 * duplicates.
 */
export const KEEPASS_ICON_CHOICES: number[] = [
  0, 1, 2, 3, 4, 5, 6, 7, 9, 11, 12, 13, 15, 16, 17, 22, 24, 25, 34, 36, 37, 38, 39, 42, 43, 44, 45,
  46, 47, 48, 49, 51, 52,
];
