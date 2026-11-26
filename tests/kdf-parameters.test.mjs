import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
const { memoryMiB, memoryBytes, readKdfParameters, defaultKdfParameters, isWeakKdfParameters } =
  await import("../src/lib/utils/kdf-parameters.ts");

test("MiB editor round-trips exact bytes, including a one-byte remainder and u64 maximum", () => {
  for (const bytes of [
    "1",
    "65537",
    "67108864",
    "10485760",
    "9007199254740993",
    "18446744073709551615",
  ]) {
    assert.equal(memoryBytes(memoryMiB(bytes)), bytes);
  }
  assert.equal(memoryMiB("10485760"), "10");
  for (const value of ["", "-1", "1e3", "0", "0.0000001", "18446744073709551615"])
    assert.equal(memoryBytes(value), null);
});

test("KeePass AES weak-cost threshold includes zero and preserves u64 precision", () => {
  for (const rounds of ["0", "1", "599999"])
    assert.equal(isWeakKdfParameters({ algorithm: "Aes", rounds }), true);
  for (const rounds of ["600000", "9007199254740993", "18446744073709551615"])
    assert.equal(isWeakKdfParameters({ algorithm: "Aes", rounds }), false);
});

test("KeePass Argon2 warning compares exact work product for both variants and versions", () => {
  for (const algorithm of ["Argon2", "Argon2id"]) {
    for (const version of [16, 19]) {
      for (const [iterations, memoryBytes, weak] of [
        ["2", "67108863", true],
        ["1", "67108864", true],
        ["2", "67108864", false],
        ["1", "134217728", false],
        ["4", "33554432", false],
        ["4294967295", "4398046511103", false],
      ]) {
        assert.equal(
          isWeakKdfParameters({ algorithm, iterations, memoryBytes, parallelism: 2, version }),
          weak,
        );
      }
    }
  }
});

test("AES draft keeps all u64 round bits and legacy zero without using Number", () => {
  for (const rounds of ["0", "9007199254740993", "18446744073709551615"]) {
    assert.deepEqual(readKdfParameters("Aes", rounds, "", "", "", 19), {
      algorithm: "Aes",
      rounds,
    });
  }
  for (const rounds of ["18446744073709551616", "1e6", "-1", "2.5"])
    assert.equal(readKdfParameters("Aes", rounds, "", "", "", 19), null);
});

test("Argon2 draft preserves version and exact memory and rejects truncating parameters", () => {
  assert.deepEqual(readKdfParameters("Argon2", "", "3", memoryMiB("65537"), "2", 16), {
    algorithm: "Argon2",
    iterations: "3",
    memoryBytes: "65537",
    parallelism: 2,
    version: 16,
  });
  for (const [iterations, mib, lanes, version] of [
    ["4294967296", "64", "4", 19],
    ["1", "4194304", "4", 19],
    ["1", "0.0078125", "2", 19],
    ["1", "64", "0", 19],
    ["1", "64", "4", 18],
  ]) {
    assert.equal(readKdfParameters("Argon2id", "", iterations, mib, lanes, version), null);
  }
});

test("switching back restores imported parameters and switching Argon2 variants retains costs", () => {
  const original = {
    algorithm: "Argon2",
    iterations: "23",
    memoryBytes: "65537",
    parallelism: 2,
    version: 16,
  };
  assert.deepEqual(defaultKdfParameters("Argon2", original), original);
  assert.deepEqual(defaultKdfParameters("Argon2id", original), {
    ...original,
    algorithm: "Argon2id",
  });
});

test("new-vault default costs meet the official warning threshold for every supported KDF", () => {
  for (const algorithm of ["Aes", "Argon2", "Argon2id"]) {
    const opposite =
      algorithm === "Aes"
        ? {
            algorithm: "Argon2id",
            iterations: "1",
            memoryBytes: "8192",
            parallelism: 1,
            version: 19,
          }
        : { algorithm: "Aes", rounds: "1" };
    assert.equal(isWeakKdfParameters(defaultKdfParameters(algorithm, opposite)), false);
  }
});
