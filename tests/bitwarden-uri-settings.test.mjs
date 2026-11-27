import assert from "node:assert/strict";
import test from "node:test";
import { register } from "node:module";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
const { copyBitwardenUriSettings, parseBitwardenUriSettings } =
  await import("../src/lib/utils/bitwarden-uri-settings.ts");

test("manual source choices preserve domain identity and detach every group", () => {
  const settings = parseBitwardenUriSettings(
    5,
    " example.com，EXAMPLE.net\n\n münchen.de, xn--mnchen-3ya.de ",
  );
  assert.deepEqual(settings, {
    defaultMatch: 5,
    equivalentDomains: [
      ["example.com", "EXAMPLE.net"],
      ["münchen.de", "xn--mnchen-3ya.de"],
    ],
  });
  const copy = copyBitwardenUriSettings(settings);
  copy.equivalentDomains[0][0] = "changed";
  assert.equal(settings.equivalentDomains[0][0], "example.com");
  assert.deepEqual(parseBitwardenUriSettings(0, ""), { defaultMatch: 0, equivalentDomains: [] });
});

test("source settings reject missing modes, bad groups, controls and bounded UTF8 size", () => {
  for (const value of [
    null,
    {},
    { defaultMatch: 6, equivalentDomains: [] },
    { defaultMatch: 0.5, equivalentDomains: [] },
    { defaultMatch: 0, equivalentDomains: null },
    ...["has space", "a\u0000b", "a\u0085b", "食".repeat(342)].map((domain) => ({
      defaultMatch: 0,
      equivalentDomains: [[domain]],
    })),
    { defaultMatch: 0, equivalentDomains: Array(129).fill([]) },
    { defaultMatch: 0, equivalentDomains: [Array(129).fill("a")] },
    { defaultMatch: 0, equivalentDomains: [Array(33).fill("a".repeat(1024))] },
  ])
    assert.throws(() => copyBitwardenUriSettings(value));
});
