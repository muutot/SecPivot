import assert from "node:assert/strict";
import test from "node:test";

import { classifyContact, detectContacts } from "../src/lib/utils/contact.ts";

test("classifyContact recognizes whole-value URLs with and without scheme", () => {
  assert.equal(classifyContact("https://example.com"), "url");
  assert.equal(classifyContact("http://example.com/a/b?q=1#x"), "url");
  assert.equal(classifyContact("www.example.com"), "url");
  assert.equal(classifyContact("example.com"), "url");
  assert.equal(classifyContact("sub.example.co.uk:8080/path"), "url");
  assert.equal(classifyContact("example.com/path?x=1"), "url");
});

test("classifyContact recognizes whole-value emails and phones", () => {
  assert.equal(classifyContact("alice@example.com"), "email");
  assert.equal(classifyContact("a.b+tag@sub.example.co.uk"), "email");
  assert.equal(classifyContact("13800138000"), "phone");
  assert.equal(classifyContact("+86 138 0013 8000"), "phone");
  assert.equal(classifyContact("010-12345678"), "phone");
  assert.equal(classifyContact("123-456-7890"), "phone");
});

test("classifyContact rejects plain text, dates, and near-misses", () => {
  assert.equal(classifyContact(""), null);
  assert.equal(classifyContact("   "), null);
  assert.equal(classifyContact("hello world"), null);
  assert.equal(classifyContact("2024-12-31"), null);
  assert.equal(classifyContact("ZX-42"), null);
  assert.equal(classifyContact("not an email @example.com"), null);
});

test("detectContacts extracts URLs, emails, and phones from free text", () => {
  const found = detectContacts(
    "联系 alice@example.com 或 13800138000,站点 https://example.com/page?x=1 欢迎访问.",
  );
  const byKind = (kind) => found.filter((m) => m.kind === kind).map((m) => m.value);
  assert.deepEqual(byKind("email"), ["alice@example.com"]);
  assert.deepEqual(byKind("phone"), ["13800138000"]);
  assert.deepEqual(byKind("url"), ["https://example.com/page?x=1"]);
});

test("detectContacts strips trailing punctuation from URLs and dedupes", () => {
  const found = detectContacts("见 https://a.example.com,以及 https://a.example.com.");
  const urls = found.filter((m) => m.kind === "url").map((m) => m.value);
  assert.deepEqual(urls, ["https://a.example.com"]);
});

test("detectContacts does not treat a date as a phone", () => {
  const found = detectContacts("截止日期 2024-12-31 前提交");
  assert.equal(
    found.some((m) => m.kind === "phone"),
    false,
  );
});
