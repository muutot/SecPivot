import assert from "node:assert/strict";
import test from "node:test";

import { buildKeePassXml } from "../src/lib/utils/kdbx-xml.ts";

test("buildKeePassXml nests groups from A / B paths and escapes text", () => {
  const xml = buildKeePassXml([
    {
      group: "Web <prod> / API",
      title: "Git&Hub",
      username: "alice",
      password: "s3cret",
      url: "https://x",
      notes: "line1\nline2",
      totp: "JBSWY3DPEHPK3PXP",
    },
    {
      group: "",
      title: "Root entry",
      username: "bob",
      password: "",
      url: "",
      notes: "",
      totp: "",
    },
  ]);

  assert.ok(xml.startsWith('<?xml version="1.0" encoding="utf-8"'));
  assert.ok(xml.includes("<KeePassFile><Meta><Generator>SecPivot</Generator></Meta><Root>"));
  // Nested group path becomes nested Group elements; text is escaped.
  assert.ok(xml.includes("<Name>Web &lt;prod&gt;</Name>"));
  assert.ok(xml.includes("<Group><Name></Name>"));
  assert.ok(xml.includes("<Key>Title</Key><Value>Git&amp;Hub</Value>"));
  // Passwords use the KeePass Protected + Base64 convention.
  assert.ok(
    xml.includes('<Key>Password</Key><Value Protected="True">' + btoa("s3cret") + "</Value>"),
  );
  assert.ok(xml.includes("<Key>otp</Key><Value>JBSWY3DPEHPK3PXP</Value>"));
  // Empty TOTP seeds are omitted entirely.
  const rootEntry = xml.indexOf("Root entry");
  assert.ok(!xml.slice(rootEntry - 400, rootEntry).includes("<Key>otp</Key>"));
  assert.ok(xml.endsWith("</Root></KeePassFile>\n"));
});
