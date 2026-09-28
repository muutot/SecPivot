import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { register } from "node:module";

register("./helpers/lib-alias-loader.mjs", import.meta.url);

const { formatEntryText } = await import("../src/lib/utils/entry-copy.ts");

const en = {
  title: "Title",
  username: "Username",
  password: "Password",
  url: "URL",
  notes: "Notes",
};

describe("formatEntryText", () => {
  it("renders the KeePass field order with labels", () => {
    const text = formatEntryText(
      {
        title: "GitHub",
        username: "octocat",
        password: "s3cret",
        url: "https://github.com",
        notes: "work account",
      },
      en,
    );
    assert.equal(
      text,
      [
        "Title: GitHub",
        "Username: octocat",
        "Password: s3cret",
        "URL: https://github.com",
        "Notes: work account",
      ].join("\n"),
    );
  });

  it("omits empty and unresolved fields", () => {
    const text = formatEntryText(
      { title: "Only a title", username: "", password: null, url: "", notes: "" },
      en,
    );
    assert.equal(text, "Title: Only a title");
  });

  it("keeps inner newlines of multi-line values", () => {
    const text = formatEntryText({ title: "T", notes: "line1\nline2" }, en);
    assert.equal(text, "Title: T\nNotes: line1\nline2");
  });

  it("appends custom fields by name and skips protected ones", () => {
    const text = formatEntryText(
      {
        title: "Bank",
        customFields: [
          { name: "Tag", value: "public" },
          { name: "PIN", value: "", protected: true },
          { name: "Empty", value: "" },
        ],
      },
      en,
    );
    assert.equal(text, "Title: Bank\nTag: public");
  });

  it("uses the supplied locale labels", () => {
    const zh = {
      title: "标题",
      username: "用户名",
      password: "密码",
      url: "网址",
      notes: "备注",
    };
    assert.equal(
      formatEntryText({ title: "GitHub", username: "octocat" }, zh),
      "标题: GitHub\n用户名: octocat",
    );
  });
});
