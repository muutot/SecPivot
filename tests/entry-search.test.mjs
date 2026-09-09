import assert from "node:assert/strict";
import test, { describe, it } from "node:test";

import { matchesAdvancedSearch, prepareAdvancedSearch } from "../src/lib/utils/entry-search.ts";

function entry(overrides = {}) {
  return {
    uuid: "entry-1",
    groupUuid: "group-1",
    title: "GitHub Production",
    username: "Alice.Dev",
    url: "https://github.com/acme",
    notes: "Primary deployment account",
    tags: "Work, DevOps，Critical",
    expired: true,
    favorite: true,
    qualityCheck: true,
    hasTotp: false,
    customFields: [
      { name: "Account ID", value: "ZX-42", protected: false },
      { name: "Protected Hint", value: "", protected: true },
    ],
    ...overrides,
  };
}

function query(overrides = {}) {
  return {
    text: "",
    field: "all",
    regex: false,
    exclude: false,
    ...overrides,
  };
}

test("field scopes search only their intended values", () => {
  const target = entry();
  for (const [field, text] of [
    ["title", "github"],
    ["username", "alice.dev"],
    ["url", "github.com/acme"],
    ["notes", "deployment"],
    ["tags", "critical"],
    ["custom", "account id:zx-42"],
    ["all", "zx-42"],
  ]) {
    assert.equal(matchesAdvancedSearch(target, query({ field, text })), true, `${field} match`);
  }

  assert.equal(matchesAdvancedSearch(target, query({ field: "title", text: "Alice.Dev" })), false);
  assert.equal(
    matchesAdvancedSearch(target, query({ field: "custom", text: "deployment" })),
    false,
  );
});

test("plain text and regex matching are case-insensitive", () => {
  const target = entry();
  assert.equal(matchesAdvancedSearch(target, query({ text: "GITHUB PRODUCTION" })), true);
  assert.equal(
    matchesAdvancedSearch(target, query({ text: "^alice\\.dev$", field: "username", regex: true })),
    true,
  );
  assert.equal(
    matchesAdvancedSearch(target, query({ text: "[", field: "title", regex: true })),
    false,
  );
});

test("exclude inverts the text predicate without disabling other filters", () => {
  const target = entry();
  assert.equal(matchesAdvancedSearch(target, query({ text: "github", exclude: true })), false);
  assert.equal(matchesAdvancedSearch(target, query({ text: "gitlab", exclude: true })), true);
  assert.equal(
    matchesAdvancedSearch(
      entry({ favorite: false }),
      query({ text: "gitlab", exclude: true, onlyFavorites: true }),
    ),
    false,
  );
  assert.equal(matchesAdvancedSearch(target, query({ text: "", exclude: true })), true);
});

test("expired, favorite, and quality filters compose", () => {
  const combined = query({ onlyExpired: true, onlyFavorites: true, requireQualityCheck: true });
  assert.equal(matchesAdvancedSearch(entry(), combined), true);
  assert.equal(matchesAdvancedSearch(entry({ expired: false }), combined), false);
  assert.equal(matchesAdvancedSearch(entry({ favorite: false }), combined), false);
  assert.equal(matchesAdvancedSearch(entry({ qualityCheck: false }), combined), false);
  assert.equal(matchesAdvancedSearch(entry({ qualityCheck: undefined }), combined), true);
});

test("tag filters accept comma, Chinese comma, and whitespace separators and require all tags", () => {
  const target = entry();
  assert.equal(matchesAdvancedSearch(target, query({ tags: "Work DevOps，Critical" })), true);
  assert.equal(matchesAdvancedSearch(target, query({ tags: "Work,Missing" })), false);
  assert.equal(matchesAdvancedSearch(target, query({ tags: "  " })), true);
});

describe("prepareAdvancedSearch", () => {
  const cases = [
    query({ text: "github" }),
    query({ text: "^GITHUB PRODUCTION$", regex: true }),
    query({ text: "[", regex: true }),
    query({ text: "[", regex: true, exclude: true }),
    query({ text: "github", exclude: true }),
    query({ text: "", exclude: true }),
    query({ tags: "Work,Missing" }),
    query({ text: "zx-42", field: "custom" }),
    query({ text: "ALICE.DEV", field: "username" }),
    query({ onlyExpired: true, onlyFavorites: true, requireQualityCheck: true }),
  ];
  const targets = [
    entry(),
    entry({ favorite: false }),
    entry({ expired: false }),
    entry({ qualityCheck: false }),
    entry({ qualityCheck: undefined }),
  ];

  it("agrees with matchesAdvancedSearch on every case and entry", () => {
    for (const q of cases) {
      const prepared = prepareAdvancedSearch(q);
      for (const target of targets) {
        assert.equal(prepared.test(target), matchesAdvancedSearch(target, q), JSON.stringify(q));
      }
    }
  });

  it("treats invalid regex as never-match (inverted by exclude)", () => {
    assert.equal(prepareAdvancedSearch(query({ text: "[", regex: true })).test(entry()), false);
    assert.equal(
      prepareAdvancedSearch(query({ text: "[", regex: true, exclude: true })).test(entry()),
      true,
    );
  });
});
