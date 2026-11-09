import assert from "node:assert/strict";
import test from "node:test";
import { register } from "node:module";

register("./helpers/lib-alias-loader.mjs", import.meta.url);

const { buildKeePassXml } = await import("../src/lib/utils/kdbx-xml.ts");

test("XML export refuses unrepresentable characters instead of changing credentials", () => {
  const base = {
    group: "",
    title: "Synthetic",
    username: "",
    password: "",
    url: "",
    notes: "",
    totp: "",
  };
  for (const invalid of [
    "\u0000",
    "\u0001",
    "\u000b",
    "\u001f",
    "\ufffe",
    "\uffff",
    "\ud800",
    "\udfff",
  ]) {
    for (const field of [
      "group",
      "title",
      "username",
      "password",
      "url",
      "notes",
      "totp",
      "expires",
    ]) {
      assert.throws(
        () => buildKeePassXml([{ ...base, [field]: `synthetic${invalid}value` }]),
        /XML/,
      );
    }
    for (const custom of [
      { name: `synthetic${invalid}name`, value: "value", protected: false },
      { name: "PIN", value: `synthetic${invalid}value`, protected: true },
    ]) {
      assert.throws(() => buildKeePassXml([{ ...base, customFields: [custom] }]), /XML/);
    }
  }
});

test("XML export retains valid Unicode and carriage returns", () => {
  const password = " synthetic\r\nline\rtab\t😀\u0085\u2028\ufffd ";
  const xml = buildKeePassXml([
    { group: "", title: "Synthetic", username: "", password, url: "", notes: "", totp: "" },
  ]);
  assert.ok(xml.includes("synthetic&#13;\nline&#13;tab\t😀\u0085\u2028\ufffd"));
  assert.ok(!xml.includes("\r"));
});

test("buildKeePassXml nests groups from A / B paths and escapes text", () => {
  const xml = buildKeePassXml([
    {
      group: "Web <prod> / API",
      title: "Git&Hub",
      username: "alice",
      password: ' 密<&钥"\n ',
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
  // KeePass's official KdbxFile.Write.cs PlainXml branch writes the literal
  // escaped value and ProtectInMemory; Protected denotes encrypted KDBX XML.
  assert.ok(
    xml.includes(
      '<Key>Password</Key><Value ProtectInMemory="True"> 密&lt;&amp;钥&quot;\n </Value>',
    ),
  );
  assert.ok(!xml.includes(' Protected="True"'));
  assert.ok(xml.includes("<Key>otp</Key><Value>JBSWY3DPEHPK3PXP</Value>"));
  // Empty TOTP seeds are omitted entirely.
  const rootEntry = xml.indexOf("Root entry");
  assert.ok(!xml.slice(rootEntry - 400, rootEntry).includes("<Key>otp</Key>"));
  assert.ok(xml.endsWith("</Root></KeePassFile>\n"));
});

test("KeePass XML preserves native OTP fields, protection flags and expiry", () => {
  const xml = buildKeePassXml([
    {
      group: "",
      title: "Synthetic HOTP",
      username: "",
      password: "",
      url: "",
      notes: "",
      totp: "",
      expires: "2030-01-02T03:04:05Z",
      customFields: [
        { name: "HmacOtp", value: "JBSWY3DPEHPK3PXP:6:42", protected: true },
        { name: "PIN", value: " <synthetic>& ", protected: true },
        { name: "Visible", value: "public", protected: false },
        { name: "Password", value: "must not replace standard password", protected: false },
      ],
    },
  ]);
  assert.ok(
    xml.includes('<Key>HmacOtp</Key><Value ProtectInMemory="True">JBSWY3DPEHPK3PXP:6:42</Value>'),
  );
  assert.ok(!xml.includes("<Key>otp</Key>"));
  assert.ok(
    xml.includes('<Key>PIN</Key><Value ProtectInMemory="True"> &lt;synthetic&gt;&amp; </Value>'),
  );
  assert.ok(xml.includes("<Key>Visible</Key><Value>public</Value>"));
  assert.equal(xml.match(/<Key>Password<\/Key>/g).length, 1);
  assert.ok(
    xml.includes(
      "<Times><Expires>True</Expires><ExpiryTime>2030-01-02T03:04:05Z</ExpiryTime></Times>",
    ),
  );
});

test("XML export row mapping carries custom fields and expiry without mutating the source", async () => {
  const { toXmlExportRows } = await import("../src/lib/services/io.ts");
  const field = { name: "SteamOtp", value: "JBSWY3DPEHPK3PXP", protected: true };
  const source = [
    {
      path: "Games",
      entry: { title: "Synthetic Steam", expires: "2030-01-01T00:00:00Z", customFields: [field] },
    },
  ];
  const rows = toXmlExportRows(source);
  assert.equal(rows[0].group, "Games");
  assert.equal(rows[0].expires, source[0].entry.expires);
  assert.deepEqual(rows[0].customFields, [field]);
  rows[0].customFields[0].value = "changed export copy";
  assert.equal(field.value, "JBSWY3DPEHPK3PXP");
});
