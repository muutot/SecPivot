import assert from "node:assert/strict";
import test from "node:test";

import { generatePassword } from "../src/lib/utils/password.ts";

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
    const password = generatePassword(settings());
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
      ),
    /字符池为空/,
  );
  assert.throws(
    () => generatePassword(settings({ customCharset: "A", excludeChars: "A" })),
    /字符池为空/,
  );
  assert.throws(
    () => generatePassword(settings({ length: 2, customCharset: "ABC", requiredChars: "ABC" })),
    /无法容纳/,
  );
  assert.throws(() => generatePassword(settings({ length: 3 })), /无法容纳/);
  assert.throws(
    () => generatePassword(settings({ customCharset: "ABC", requiredChars: "X" })),
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
    );
    assert.equal(password, "A1a-L!");
  }

  const custom = generatePassword(
    settings({ customCharset: "ABC123", pattern: "aa", requiredChars: "A3" }),
  );
  assert.equal(custom.length, 2);
  assert.ok(custom.includes("A"));
  assert.ok(custom.includes("3"));
});

test("pattern generation rejects incompatible required chars and empty categories", () => {
  assert.throws(
    () => generatePassword(settings({ pattern: "uL", requiredChars: "1" })),
    /pattern 无法容纳必含字符 1/,
  );
  assert.throws(
    () => generatePassword(settings({ pattern: "d", excludeChars: "0123456789" })),
    /pattern 类别 d 的字符池为空/,
  );
});
