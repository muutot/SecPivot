import assert from "node:assert/strict";
import test from "node:test";
import { readFileSync } from "node:fs";
import { shortestMatchable, urlHost } from "../src/lib/utils/match-url.ts";

test("Exact suggestions retain every part of the requested URL", () => {
  for (const address of [
    "https://bank.example/Login?tenant=one#form",
    "http://bank.example:8080/Login?tenant=two#other",
    "https://bank.example/Login",
    "bank.example/Login?tenant=one#form",
  ]) {
    assert.equal(shortestMatchable(`  ${address}  `, "Exact"), address);
  }
});

test("broader suggestions retain their existing host scope", () => {
  const address = "https://bank.example/Login?tenant=one#form";
  assert.equal(shortestMatchable(address, "Domain"), "bank.example");
  assert.equal(shortestMatchable(address, "Hostname"), "bank.example");
  assert.equal(shortestMatchable("", "Exact"), null);
});

test("Hostname suggestions preserve explicit ports including defaults and IPv6", () => {
  for (const [address, expected] of [
    ["https://bank.example:8443/Login?tenant=one", "bank.example:8443"],
    ["https://bank.example:443/Login", "bank.example:443"],
    ["http://bank.example:80/Login", "bank.example:80"],
    ["http://[2001:db8::1]:8443/Login", "[2001:db8::1]:8443"],
    ["https://bank.example/Login", "bank.example"],
  ]) {
    assert.equal(shortestMatchable(address, "Hostname"), expected);
  }
});

test("rule suggestions agree with the shared canonical authority corpus", () => {
  const cases = JSON.parse(
    readFileSync(new URL("./fixtures/url-authorities.json", import.meta.url), "utf8"),
  );
  for (const { input, host, port } of cases) {
    assert.equal(urlHost(input), host);
    if (host === null) {
      for (const accuracy of ["Exact", "Hostname", "Domain"]) {
        assert.equal(shortestMatchable(input, accuracy), null);
      }
    } else {
      const urlHost = host.includes(":") ? `[${host}]` : host;
      assert.equal(shortestMatchable(input, "Domain"), urlHost);
      assert.equal(
        shortestMatchable(input, "Hostname"),
        port === null ? urlHost : `${urlHost}:${port}`,
      );
      assert.equal(shortestMatchable(input, "Exact"), input.trim());
    }
  }
});
