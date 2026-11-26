import assert from "node:assert/strict";
import { register } from "node:module";
import { get } from "svelte/store";
import test from "node:test";
register("./helpers/lib-alias-loader.mjs", import.meta.url);
const { KdfOpenPromptController } = await import("../src/lib/services/kdf-open-prompt.ts");
const a = { sessionId: "a", epoch: Symbol("a") };
const b = { sessionId: "b", epoch: Symbol("b") };
const offer = {
  token: "synthetic-lifetime",
  fileName: "synthetic.kdbx",
  parameters: { algorithm: "Aes", rounds: "1" },
  recommended: { algorithm: "Aes", rounds: "600000" },
  readOnly: false,
};
const flush = () => new Promise((resolve) => setImmediate(resolve));
function fixture() {
  const reads = [],
    writes = [];
  let enabled = true;
  const controller = new KdfOpenPromptController({
    enabled: () => enabled,
    inspect: (id) => new Promise((resolve, reject) => reads.push({ id, resolve, reject })),
    upgrade: (id, offer) =>
      new Promise((resolve, reject) => writes.push({ id, offer, resolve, reject })),
    error: (error) => String(error),
  });
  return {
    controller,
    reads,
    writes,
    setEnabled: (value) => (enabled = value),
    prompt: () => get(controller.state),
  };
}

test("opening a weak lifetime only proposes; dismissing stays acknowledged across tab visits", async () => {
  const f = fixture();
  f.controller.activate(a);
  assert.equal(f.prompt(), null);
  assert.equal(f.reads[0].id, "a");
  f.reads[0].resolve(offer);
  await flush();
  assert.equal(f.prompt().offer, offer);
  assert.deepEqual(f.writes, []);
  f.controller.dismiss();
  f.controller.activate(b);
  f.reads[1].resolve({ ...offer, recommended: null });
  await flush();
  f.controller.activate(a);
  assert.equal(f.prompt(), null);
  assert.equal(f.reads.length, 2);
  assert.deepEqual(f.writes, []);
});

test("disabled preference and strong costs never present an upgrade or mutate", async () => {
  const f = fixture();
  f.setEnabled(false);
  f.controller.activate(a);
  assert.deepEqual(f.reads, []);
  f.setEnabled(true);
  f.controller.activate(b);
  f.reads[0].resolve({ ...offer, recommended: null });
  await flush();
  f.controller.activate(a);
  await f.controller.upgrade();
  assert.equal(f.prompt(), null);
  assert.deepEqual(f.writes, []);
});

test("A-B-A and detached inspections cannot publish an obsolete recommendation", async () => {
  const f = fixture();
  f.controller.activate(a);
  f.controller.activate(b);
  f.controller.activate(a);
  assert.equal(f.reads.length, 3);
  f.reads[0].resolve({ ...offer, token: "obsolete-a" });
  f.reads[1].resolve({ ...offer, token: "obsolete-b" });
  await flush();
  assert.equal(f.prompt(), null);
  f.reads[2].resolve({ ...offer, token: "current-a" });
  await flush();
  assert.equal(f.prompt().offer.token, "current-a");
  f.controller.activate(b);
  f.controller.dispose();
  f.reads[3].resolve(offer);
  await flush();
  assert.equal(f.prompt(), null);
});

test("explicit upgrade uses the reviewed origin and permits one pending request", async () => {
  const f = fixture();
  f.controller.activate(a);
  f.reads[0].resolve(offer);
  await flush();
  const run = f.controller.upgrade();
  assert.equal(f.prompt().busy, true);
  await f.controller.upgrade();
  f.controller.dismiss();
  assert.equal(f.writes.length, 1);
  assert.equal(f.writes[0].id, "a");
  assert.equal(f.writes[0].offer, offer);
  f.writes[0].resolve();
  await run;
  assert.equal(f.prompt(), null);
});

test("read-only proposals stay visible but cannot request a mutation", async () => {
  const f = fixture();
  f.controller.activate(a);
  f.reads[0].resolve({ ...offer, readOnly: true });
  await flush();
  await f.controller.upgrade();
  assert.deepEqual(f.writes, []);
  assert.equal(f.prompt().offer.readOnly, true);
  f.controller.dismiss();
  assert.equal(f.prompt(), null);
});

test("late upgrade success/failure cannot close a new tab's prompt", async () => {
  for (const failure of [false, true]) {
    const f = fixture();
    f.controller.activate(a);
    f.reads[0].resolve(offer);
    await flush();
    const run = f.controller.upgrade();
    f.controller.activate(b);
    f.reads[1].resolve({ ...offer, token: "b-lifetime" });
    await flush();
    if (failure) f.writes[0].reject(new Error("old failure"));
    else f.writes[0].resolve();
    await run;
    assert.equal(f.prompt().offer.token, "b-lifetime");
    assert.equal(f.prompt().busy, false);
    assert.equal(f.prompt().error, "");
  }
});

test("failed upgrades retain a retryable proposal; replaced epochs require fresh inspection", async () => {
  const f = fixture();
  f.controller.activate(a);
  f.reads[0].resolve(offer);
  await flush();
  const run = f.controller.upgrade();
  f.writes[0].reject(new Error("KDF_UPGRADE_CHANGED:"));
  await run;
  assert.equal(f.prompt().busy, false);
  assert.match(f.prompt().error, /KDF_UPGRADE_CHANGED/);
  f.controller.activate({ sessionId: "a", epoch: Symbol("replacement") });
  assert.equal(f.prompt(), null);
  assert.equal(f.reads.length, 2);
  f.reads[1].resolve({ ...offer, token: "replacement" });
  await flush();
  assert.equal(f.prompt().offer.token, "replacement");
});

test("inspection rejection does not fail unlock, acknowledge unseen costs or write", async () => {
  const f = fixture();
  f.controller.activate(a);
  f.reads[0].reject(new Error("inspection unavailable"));
  await flush();
  assert.equal(f.prompt(), null);
  f.controller.activate(null);
  f.controller.activate(a);
  assert.equal(f.reads.length, 2);
  f.reads[1].resolve(offer);
  await flush();
  assert.equal(f.prompt().offer, offer);
  assert.deepEqual(f.writes, []);
});

test("pending inspections and approvals recheck the current preference", async () => {
  const f = fixture();
  f.controller.activate(a);
  f.setEnabled(false);
  f.reads[0].resolve(offer);
  await flush();
  assert.equal(f.prompt(), null);
  f.controller.activate(b);
  f.setEnabled(true);
  f.controller.activate({ sessionId: "c", epoch: Symbol("c") });
  f.reads[1].resolve(offer);
  await flush();
  f.setEnabled(false);
  await f.controller.upgrade();
  assert.deepEqual(f.writes, []);
  assert.equal(f.prompt(), null);
});
