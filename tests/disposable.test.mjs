import assert from "node:assert/strict";
import test from "node:test";

import { replaceDisposable, settleDisposable } from "../src/lib/utils/disposable.ts";

test("replacing an owned reference disposes the previous value", () => {
  const disposed = [];
  const first = { token: "first" };
  const second = { token: "second" };
  let current = replaceDisposable(null, first, (value) => disposed.push(value.token));

  current = replaceDisposable(current, second, (value) => disposed.push(value.token));

  assert.equal(current, second);
  assert.deepEqual(disposed, ["first"]);
});

test("clearing an owned reference disposes its current value", () => {
  const disposed = [];
  const current = { token: "current" };

  const cleared = replaceDisposable(current, null, (value) => disposed.push(value.token));

  assert.equal(cleared, null);
  assert.deepEqual(disposed, ["current"]);
});

test("reusing the same reference does not dispose it twice", () => {
  const disposed = [];
  const current = { token: "current" };

  const retained = replaceDisposable(current, current, (value) => disposed.push(value.token));

  assert.equal(retained, current);
  assert.deepEqual(disposed, []);
});

test("a failed consumer retains its owned reference for retry", () => {
  const owned = { token: "owned" };

  assert.equal(settleDisposable(owned, owned, false), owned);
});

test("a late consumer cannot forget a newer replacement", () => {
  const owned = { token: "owned" };
  const replacement = { token: "replacement" };

  assert.equal(settleDisposable(replacement, owned, true), replacement);
  assert.equal(settleDisposable(owned, owned, true), null);
});
