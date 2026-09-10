import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { register } from "node:module";

register("./helpers/lib-alias-loader.mjs", import.meta.url);

const { t } = await import("../src/lib/i18n/index.ts");
const { en } = await import("../src/lib/i18n/en.ts");
const { zhCN } = await import("../src/lib/i18n/zh-CN.ts");

describe("i18n dictionaries", () => {
  it("covers the exact same key set in both locales", () => {
    const zhKeys = Object.keys(zhCN).sort();
    const enKeys = Object.keys(en).sort();
    assert.deepEqual(enKeys, zhKeys);
    assert.ok(zhKeys.length > 0, "expected translation keys");
    assert.ok(
      zhKeys.every((key) => /^[\w]+(\.[\w-]+)+$/.test(key)),
      "flat dotted keys only",
    );
  });

  it("has no empty translations", () => {
    for (const [key, value] of Object.entries(zhCN)) {
      assert.ok(value.length > 0, `zh-CN ${key} must not be empty`);
    }
    for (const [key, value] of Object.entries(en)) {
      assert.ok(value.length > 0, `en ${key} must not be empty`);
    }
  });
});

describe("t", () => {
  it("resolves keys per locale", () => {
    assert.equal(t("zh-CN", "common.ok"), "确定");
    assert.equal(t("en", "common.ok"), "OK");
  });

  it("falls back to Chinese, then to the key itself", () => {
    assert.equal(t("fr", "common.ok"), "确定");
    assert.equal(t("en", "missing.key"), "missing.key");
  });
});
