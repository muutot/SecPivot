import assert from "node:assert/strict";
import test from "node:test";

import { classifyContact, detectContacts, linkifyContacts } from "../src/lib/utils/contact.ts";

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

test("linkifyContacts flags inline URL/email/phone and keeps surrounding text", () => {
  const tokens = linkifyContacts(
    "联系 alice@example.com,站点 https://example.com/x?q=1 或 13800138000",
  );
  const texts = tokens.filter((t) => t.kind === "text").map((t) => t.value);
  const contacts = tokens.filter((t) => t.kind !== "text");
  assert.equal(texts.join(""), "联系 ,站点  或 ");
  assert.deepEqual(contacts, [
    { kind: "email", value: "alice@example.com" },
    { kind: "url", value: "https://example.com/x?q=1" },
    { kind: "phone", value: "13800138000" },
  ]);
});

test("linkifyContacts preserves newlines and trailing URL punctuation as text", () => {
  const tokens = linkifyContacts("第一行 https://a.example.com.\n第二行 www.b.example.org/");
  assert.deepEqual(tokens, [
    { kind: "text", value: "第一行 " },
    { kind: "url", value: "https://a.example.com" },
    { kind: "text", value: ".\n第二行 " },
    { kind: "url", value: "www.b.example.org/" },
  ]);
});

test("linkifyContacts never double-links a phone-shaped fragment inside a URL", () => {
  const tokens = linkifyContacts("查 https://x.example/1234567890 末尾");
  const contacts = tokens.filter((t) => t.kind !== "text");
  assert.deepEqual(contacts, [{ kind: "url", value: "https://x.example/1234567890" }]);
});

test("linkifyContacts returns plain text when there are no contacts", () => {
  const tokens = linkifyContacts("纯文本，无链接。\n第二行");
  assert.deepEqual(tokens, [{ kind: "text", value: "纯文本，无链接。\n第二行" }]);
});

test("linkifyContacts returns empty tokens for empty text", () => {
  assert.deepEqual(linkifyContacts(""), []);
});
