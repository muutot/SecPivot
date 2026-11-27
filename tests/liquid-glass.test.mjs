import assert from "node:assert/strict";
import { after, test } from "node:test";
import { register } from "node:module";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.window = globalThis;
let persisted = {};
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    if (command === "get_config") return structuredClone(persisted);
    if (command === "set_config") return (persisted = structuredClone(args.config));
    if (command === "remote_secret_present") return { accessKey: false, secretKey: false };
    throw new Error("Unexpected synthetic command " + command);
  },
};
const { appSettings, normalizeSettings, selectThemeColors } =
  await import("../src/lib/services/settings.ts");
const { DARK_THEME_COLORS, LIGHT_THEME_COLORS } = await import("../src/lib/types/theme.ts");
const { createRimMap, liquidGlass } = await import("../src/lib/utils/liquid-glass.ts");
after(() => appSettings.destroy());

test("all five themes persist independently without replacing custom colors", async () => {
  await appSettings.initialize();
  const custom = { ...DARK_THEME_COLORS, accent: "#abcd12" };
  appSettings.updateGeneral("themeColors", custom);
  for (const theme of ["liquid-dark", "light", "liquid-light", "dark", "custom"]) {
    appSettings.updateGeneral("theme", theme);
    await appSettings.flush();
    const normalized = normalizeSettings(structuredClone(persisted));
    assert.equal(normalized.general.theme, theme);
    assert.deepEqual(normalized.general.themeColors, custom);
    assert.deepEqual(
      selectThemeColors(normalized),
      theme === "custom"
        ? custom
        : theme === "light" || theme === "liquid-light"
          ? LIGHT_THEME_COLORS
          : DARK_THEME_COLORS,
    );
  }
  assert.equal(normalizeSettings({ general: { theme: "neon" } }).general.theme, "dark");
  assert.equal(normalizeSettings({}).general.theme, "dark");
});

test("lens center is neutral and opposed rims bend in opposite directions", () => {
  const width = 80;
  const height = 40;
  const pixels = createRimMap(width, height, 8);
  const pixel = (x, y) => Array.from(pixels.slice((y * width + x) * 4, (y * width + x) * 4 + 4));
  assert.deepEqual(pixel(40, 20), [128, 128, 128, 255]);
  assert.deepEqual(pixel(0, 0), [128, 128, 128, 255]);
  assert.ok(pixel(3, 20)[0] < 128);
  assert.ok(pixel(76, 20)[0] > 128);
  assert.ok(pixel(40, 3)[1] < 128);
  assert.ok(pixel(40, 36)[1] > 128);
  assert.deepEqual(createRimMap(width, height, 8), pixels);
  assert.equal(createRimMap(1, 1, 100).length, 4);
  for (const dimensions of [
    [0, 40, 8],
    [80, 1025, 8],
    [1.5, 40, 8],
    [80, 40, NaN],
  ]) {
    assert.throws(() => createRimMap(...dimensions), RangeError);
  }
});

test("a legacy theme never allocates a lens and action disposal removes observers", () => {
  const prior = Object.fromEntries(
    [
      "document",
      "matchMedia",
      "requestAnimationFrame",
      "cancelAnimationFrame",
      "ResizeObserver",
      "MutationObserver",
    ].map((key) => [key, globalThis[key]]),
  );
  let scheduled;
  let disconnected = 0;
  let removed = 0;
  let listeners = 0;
  globalThis.document = { documentElement: { dataset: { theme: "dark" } } };
  globalThis.matchMedia = () => ({
    matches: false,
    addEventListener() {
      listeners++;
    },
    removeEventListener() {
      listeners--;
    },
  });
  globalThis.requestAnimationFrame = (callback) => {
    scheduled = callback;
    return 1;
  };
  globalThis.cancelAnimationFrame = () => {};
  globalThis.ResizeObserver = globalThis.MutationObserver = class {
    observe() {}
    disconnect() {
      disconnected++;
    }
  };
  try {
    const action = liquidGlass({
      style: {
        removeProperty() {
          removed++;
        },
      },
    });
    scheduled();
    assert.equal(removed, 1);
    action.destroy();
    assert.equal(disconnected, 2);
    assert.equal(listeners, 0);
  } finally {
    for (const [key, value] of Object.entries(prior)) {
      if (value === undefined) delete globalThis[key];
      else globalThis[key] = value;
    }
  }
});
