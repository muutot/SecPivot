import assert from "node:assert/strict";
import test from "node:test";
import { shortestMatchable } from "../src/lib/utils/match-url.ts";

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
