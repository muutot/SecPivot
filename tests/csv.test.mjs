import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { parseCsv, parseCsvRows, buildCsv } from "../src/lib/utils/csv.ts";

describe("parseCsvRows", () => {
  it("maps LastPass export headers via aliases", () => {
    const raw = parseCsv(
      "Name,Username,Password,URL,Extra,Grouping\nBank,bob,s3cret,https://b.example,a note,Finance\n",
    );
    const rows = parseCsvRows(raw);
    assert.equal(rows.length, 1);
    assert.equal(rows[0].title, "Bank");
    assert.equal(rows[0].username, "bob");
    assert.equal(rows[0].password, "s3cret");
    assert.equal(rows[0].notes, "a note");
    assert.equal(rows[0].group, "Finance");
  });

  it("maps canonical headers as before", () => {
    const raw = parseCsv("Group,Title,Username,Password,URL,Notes\nG,T,u,p,https://u.example,n\n");
    const rows = parseCsvRows(raw);
    assert.equal(rows.length, 1);
    assert.equal(rows[0].title, "T");
    assert.equal(rows[0].group, "G");
  });

  it("keeps rows that have content but no title or password", () => {
    const raw = parseCsv(
      "Group,Title,Username,Password,URL,Notes\nG,,user1,,https://u.example,note-only\n",
    );
    const rows = parseCsvRows(raw);
    assert.equal(rows.length, 1);
    assert.equal(rows[0].username, "user1");
    assert.equal(rows[0].url, "https://u.example");
  });

  it("keeps a positional first row whose title happens to match a header name", () => {
    const raw = parseCsv("Finance,url,bob,s3cret,https://u.example,note\n");
    const rows = parseCsvRows(raw);
    assert.equal(rows.length, 1);
    assert.equal(rows[0].group, "Finance");
    assert.equal(rows[0].title, "url");
  });

  it("neutralizes spreadsheet formula prefixes on export", () => {
    const csv = buildCsv([
      {
        group: "",
        title: "=1+1",
        username: "+2",
        password: "-3",
        url: "@cmd",
        notes: "",
        totp: "",
        favorite: false,
      },
    ]);
    assert.ok(csv.includes("'=1+1"));
    assert.ok(csv.includes("'+2"));
    assert.ok(csv.includes("'-3"));
    assert.ok(csv.includes("'@cmd"));
  });
});
