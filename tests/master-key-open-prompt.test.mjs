import assert from "node:assert/strict";
import { register } from "node:module";
import { get } from "svelte/store";
import test from "node:test";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
const { MasterKeyOpenPromptController } =
  await import("../src/lib/services/master-key-open-prompt.ts");
const flush = () => new Promise((resolve) => setImmediate(resolve));
function fixture() {
  const inspections = [],
    closes = [];
  let failure;
  const controller = new MasterKeyOpenPromptController({
    enabled: () => true,
    inspect: (id) => new Promise((resolve, reject) => inspections.push({ id, resolve, reject })),
    close: async (owner) => {
      closes.push(owner);
      if (failure) throw failure;
    },
    error: String,
  });
  return {
    controller,
    inspections,
    closes,
    state: () => get(controller.state),
    ready: () => get(controller.ready),
    failClose: () => {
      failure = new Error("synthetic close cancelled");
    },
  };
}
const offer = (patch = {}) => ({
  token: "synthetic-native-lifetime",
  fileName: "synthetic.kdbx",
  changedAt: "2000-01-01T00:00:00Z",
  recommended: false,
  required: false,
  forceOnce: false,
  readOnly: false,
  invalidRules: [],
  ...patch,
});
const owner = (id = "a") => ({ sessionId: id, epoch: Symbol(id) });

test("required renewal blocks decline; back preserves requirement and only successful change releases KDF", async () => {
  const f = fixture(),
    a = owner();
  f.controller.activate(a);
  assert.equal(f.state().phase, "inspect");
  assert.equal(f.ready(), null);
  f.inspections.shift().resolve(offer({ required: true, forceOnce: true }));
  await flush();
  f.controller.decline();
  assert.equal(f.state().phase, "review");
  assert.equal(f.ready(), null);
  f.controller.change();
  f.controller.setBusy(a, true);
  f.controller.back();
  assert.equal(f.state().phase, "change");
  f.controller.setBusy(a, false);
  f.controller.back();
  assert.equal(f.state().phase, "review");
  assert.equal(f.ready(), null);
  f.controller.change();
  f.controller.changed(owner());
  assert.equal(f.ready(), null);
  f.controller.changed(a);
  assert.equal(f.state(), null);
  assert.deepEqual(f.ready(), a);
  f.controller.activate(owner("b"));
  f.controller.activate(a);
  assert.deepEqual(f.ready(), a);
});
test("recommended/warning-only decisions can be declined once; readonly force requires closing", async () => {
  for (const patch of [{ recommended: true }, { invalidRules: ["required"] }]) {
    const f = fixture(),
      a = owner();
    f.controller.activate(a);
    f.inspections.shift().resolve(offer(patch));
    await flush();
    f.controller.decline();
    assert.deepEqual(f.ready(), a);
    assert.equal(f.state(), null);
  }
  const f = fixture(),
    a = owner();
  f.controller.activate(a);
  f.inspections.shift().resolve(offer({ required: true, readOnly: true }));
  await flush();
  f.controller.change();
  assert.equal(f.state().phase, "review");
  f.controller.decline();
  assert.equal(f.ready(), null);
  f.failClose();
  await f.controller.close();
  assert.equal(f.state().phase, "review");
  assert.equal(f.state().busy, false);
  assert.equal(f.ready(), null);
  assert.deepEqual(f.closes, [a]);
});
test("failed inspection offers retry and never silently permits an unknown renewal policy", async () => {
  const f = fixture(),
    a = owner();
  f.controller.activate(a);
  f.inspections.shift().reject(new Error("synthetic unavailable"));
  await flush();
  assert.equal(f.state().phase, "error");
  assert.equal(f.ready(), null);
  f.controller.decline();
  assert.equal(f.ready(), null);
  f.controller.retry();
  assert.equal(f.state().phase, "inspect");
  f.inspections.shift().resolve(offer());
  await flush();
  assert.equal(f.state(), null);
  assert.deepEqual(f.ready(), a);
});
test("late A-B-A and disposed replies cannot acknowledge a new lifetime or change form", async () => {
  const f = fixture(),
    a = owner(),
    b = owner("b");
  f.controller.activate(a);
  const late = f.inspections.shift();
  f.controller.activate(b);
  f.controller.activate(a);
  const current = f.inspections.at(-1);
  late.resolve(offer());
  await flush();
  assert.equal(f.ready(), null);
  current.resolve(offer({ required: true }));
  await flush();
  f.controller.change();
  f.controller.activate({ ...a, epoch: Symbol("reopened") });
  f.controller.changed(a);
  f.controller.setBusy(a, true);
  assert.equal(f.state().phase, "inspect");
  assert.equal(f.ready(), null);
  f.controller.dispose();
  f.inspections.at(-1).resolve(offer());
  await flush();
  assert.equal(f.state(), null);
  assert.equal(f.ready(), null);
});
test("identical busy callbacks do not publish an endless reactive feedback loop", async () => {
  const f = fixture(),
    a = owner();
  f.controller.activate(a);
  f.inspections.shift().resolve(offer({ required: true }));
  await flush();
  f.controller.change();
  let publications = 0;
  const off = f.controller.state.subscribe(() => publications++);
  f.controller.setBusy(a, false);
  assert.equal(publications, 1);
  f.controller.setBusy(a, true);
  f.controller.setBusy(a, true);
  assert.equal(publications, 2);
  off();
});
