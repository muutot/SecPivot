import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
const { HistoryRuleView } = await import("../src/lib/utils/history-rule-view.ts");
const policy = {
  version: 1,
  rules: [{ url: "https://synthetic.example/exact?x=1", mode: "exact" }],
};
function fixture() {
  const pending = [],
    states = [];
  let current = true;
  const view = new HistoryRuleView(
    (source) => new Promise((resolve, reject) => pending.push({ source, resolve, reject })),
    (state) => states.push(state),
    () => current,
  );
  return {
    view,
    pending,
    states,
    last: () => states.at(-1),
    obsolete: () => {
      current = false;
    },
  };
}

test("historical policy reads only after explicit load and keeps exact rule bytes", async () => {
  const f = fixture();
  assert.equal(f.pending.length, 0);
  const task = f.view.load("version");
  assert.equal(f.last().loading, true);
  assert.equal(f.last().ready, false);
  assert.equal(f.last().value, null);
  f.pending[0].resolve(policy);
  await task;
  assert.deepEqual(f.last().value, policy);
  assert.notEqual(f.last().value, policy);
  assert.equal(f.last().source, "version");
  assert.equal(f.last().ready, true);
});

test("absence, empty set and failure remain distinct states", async () => {
  for (const value of [null, { version: 1, rules: [] }]) {
    const f = fixture(),
      task = f.view.load("version");
    f.pending[0].resolve(value);
    await task;
    assert.equal(f.last().ready, true);
    assert.equal(f.last().error, false);
    assert.deepEqual(f.last().value, value);
  }
  const f = fixture(),
    task = f.view.load("version");
  f.pending[0].reject(new Error("synthetic invalid policy"));
  await task;
  assert.equal(f.last().error, true);
  assert.equal(f.last().ready, false);
  assert.equal(f.last().value, null);
  const retry = f.view.load("version");
  f.pending[1].resolve(policy);
  await retry;
  assert.equal(f.last().error, false);
  assert.deepEqual(f.last().value, policy);
});

test("switching sides clears previous policy and ignores older completions", async () => {
  const f = fixture(),
    old = f.view.load("version"),
    current = f.view.load("current");
  assert.equal(f.last().source, "current");
  assert.equal(f.last().value, null);
  f.pending[1].resolve({ version: 1, rules: [] });
  await current;
  const count = f.states.length;
  f.pending[0].resolve(policy);
  await old;
  assert.equal(f.states.length, count);
  assert.deepEqual(f.last().value, { version: 1, rules: [] });
});

test("hiding discards loaded values and cancels pending responses", async () => {
  const f = fixture(),
    first = f.view.load("version");
  f.pending[0].resolve(policy);
  await first;
  f.view.hide();
  assert.equal(f.last().visible, false);
  assert.equal(f.last().value, null);
  const task = f.view.load("current");
  f.view.hide();
  const count = f.states.length;
  f.pending[1].resolve(policy);
  await task;
  assert.equal(f.states.length, count);
  assert.equal(f.last().visible, false);
  assert.equal(f.last().ready, false);
});

test("disposed or replaced historical owners cannot publish late reads or failures", async () => {
  for (const obsolete of [false, true])
    for (const fail of [false, true]) {
      const f = fixture(),
        task = f.view.load("version");
      if (obsolete) f.obsolete();
      else f.view.dispose();
      const count = f.states.length;
      if (fail) f.pending[0].reject(new Error("synthetic"));
      else f.pending[0].resolve(policy);
      await task;
      assert.equal(f.states.length, count);
      await f.view.load("current");
      assert.equal(f.pending.length, 1);
    }
});
