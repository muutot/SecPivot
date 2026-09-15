import { describe, it } from "node:test";
import assert from "node:assert/strict";

import { parseCsv, parseCsvRows } from "../src/lib/utils/csv.ts";

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
});
