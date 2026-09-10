import assert from "node:assert/strict";
import test from "node:test";
import { register } from "node:module";

register("./helpers/lib-alias-loader.mjs", import.meta.url);

const { generatePassword } = await import("../src/lib/utils/password.ts");

function settings(overrides = {}) {
  return {
    length: 20,
    includeUpper: true,
    includeLower: true,
    includeDigits: true,
    includeSymbols: true,
    excludeSimilar: false,
    excludeAmbiguous: false,
    ...overrides,
  };
}

test("default generation guarantees every enabled category", () => {
  for (let attempt = 0; attempt < 100; attempt++) {
    const password = generatePassword(settings(), "zh-CN");
    assert.equal(password.length, 20);
    assert.match(password, /[A-Z]/);
    assert.match(password, /[a-z]/);
    assert.match(password, /[0-9]/);
    assert.match(password, /[^A-Za-z0-9]/);
  }
});

test("disabled categories never leak into the output", () => {
  for (let attempt = 0; attempt < 20; attempt++) {
    const password = generatePassword(
      settings({
        length: 32,
        includeUpper: false,
        includeLower: false,
        includeSymbols: false,
      }),
      "zh-CN",
    );
    assert.match(password, /^\d{32}$/);
  }
});

test("custom charset, exclusions, and required characters compose", () => {
  for (let attempt = 0; attempt < 20; attempt++) {
    const password = generatePassword(
      settings({
        length: 12,
        customCharset: "ABC123",
        excludeChars: "B2",
        requiredChars: "A3",
      }),
      "zh-CN",
    );
    assert.match(password, /^[AC13]{12}$/);
    assert.ok(password.includes("A"));
    assert.ok(password.includes("3"));
  }
});

test("impossible pools and capacity constraints fail explicitly", () => {
  assert.throws(
    () =>
      generatePassword(
        settings({
          includeUpper: false,
          includeLower: false,
          includeDigits: false,
          includeSymbols: false,
        }),
        "zh-CN",
      ),
    /字符池为空/,
  );
  assert.throws(
    () => generatePassword(settings({ customCharset: "A", excludeChars: "A" }), "zh-CN"),
    /字符池为空/,
  );
  assert.throws(
    () =>
      generatePassword(
        settings({ length: 2, customCharset: "ABC", requiredChars: "ABC" }),
        "zh-CN",
      ),
    /无法容纳/,
  );
  assert.throws(() => generatePassword(settings({ length: 3 }), "zh-CN"), /无法容纳/);
  assert.throws(
    () => generatePassword(settings({ customCharset: "ABC", requiredChars: "X" }), "zh-CN"),
    /不在字符池中/,
  );
});

test("pattern slots keep their categories and satisfy compatible required characters", () => {
  for (let attempt = 0; attempt < 20; attempt++) {
    const password = generatePassword(
      settings({
        pattern: "udl-Ls",
        requiredChars: "A1a!",
      }),
      "zh-CN",
    );
    assert.equal(password, "A1a-L!");
  }

  const custom = generatePassword(
    settings({ customCharset: "ABC123", pattern: "aa", requiredChars: "A3" }),
    "zh-CN",
  );
  assert.equal(custom.length, 2);
  assert.ok(custom.includes("A"));
  assert.ok(custom.includes("3"));
});

test("pattern generation rejects incompatible required chars and empty categories", () => {
  assert.throws(
    () => generatePassword(settings({ pattern: "uL", requiredChars: "1" }), "zh-CN"),
    /pattern 无法容纳必含字符 1/,
  );
  assert.throws(
    () => generatePassword(settings({ pattern: "d", excludeChars: "0123456789" }), "zh-CN"),
    /pattern 类别 d 的字符池为空/,
  );
});

test("error messages follow the requested locale", () => {
  assert.throws(() => generatePassword(settings({ length: 0 }), "en"), /positive integer/);
  assert.throws(
    () => generatePassword(settings({ pattern: "d", excludeChars: "0123456789" }), "en"),
    /pattern category d has an empty pool/,
  );
});
