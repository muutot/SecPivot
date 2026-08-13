import assert from "node:assert/strict";
import test from "node:test";

import {
  canToggleSecretReveal,
  commitNewestSessionState,
  LatestOperationGuard,
  resolveListedActiveId,
  SessionViewGuard,
  SessionStateCache,
  SessionSwitchQueue,
  switchSession,
} from "../src/lib/utils/session-state.ts";

function deferred() {
  let resolve;
  const promise = new Promise((done) => {
    resolve = done;
  });
  return { promise, resolve };
}

test("the real switch orchestration validates an uncached tab before swapping", async () => {
  const queue = new SessionSwitchQueue();
  const snapshotGate = deferred();
  const order = [];

  const toB = switchSession({
    queue,
    cached: undefined,
    load: async () => {
      order.push("B:snapshot:start");
      await snapshotGate.promise;
      order.push("B:snapshot:end");
      return { revision: 1, value: "B snapshot" };
    },
    activate: async () => {
      order.push("B:switch");
      return { revision: 1, value: "B active" };
    },
    commit: (state) => state,
    publish: () => order.push("B:publish"),
  });
  const toA = switchSession({
    queue,
    cached: { revision: 1, value: "A cached" },
    load: async () => null,
    activate: async () => {
      order.push("A:switch");
      return { revision: 1, value: "A active" };
    },
    commit: (state) => state,
    publish: () => order.push("A:publish"),
  });

  await Promise.resolve();
  assert.deepEqual(order, ["B:snapshot:start"]);
  snapshotGate.resolve();

  assert.equal((await toB).value, "B active");
  assert.equal((await toA).value, "A active");
  assert.deepEqual(order, [
    "B:snapshot:start",
    "B:snapshot:end",
    "B:switch",
    "B:publish",
    "A:switch",
    "A:publish",
  ]);
});

test("a failed uncached snapshot never performs its backend switch", async () => {
  const queue = new SessionSwitchQueue();
  const order = [];

  const toB = switchSession({
    queue,
    cached: undefined,
    load: async () => {
      order.push("B:snapshot");
      throw new Error("session missing");
    },
    activate: async () => {
      order.push("B:switch");
      return { revision: 1, value: "B" };
    },
    commit: (state) => state,
    publish: () => order.push("B:publish"),
  });
  const toA = switchSession({
    queue,
    cached: { revision: 1, value: "A cached" },
    load: async () => null,
    activate: async () => {
      order.push("A:switch");
      return { revision: 1, value: "A active" };
    },
    commit: (state) => state,
    publish: () => order.push("A:publish"),
  });

  await assert.rejects(toB, /session missing/);
  assert.equal((await toA).value, "A active");
  assert.deepEqual(order, ["B:snapshot", "A:switch", "A:publish"]);
});

test("topology changes and tab switches complete in invocation order", async () => {
  const queue = new SessionSwitchQueue();
  const publishGate = deferred();
  const order = [];

  const switching = switchSession({
    queue,
    cached: { revision: 1, value: "A cached" },
    load: async () => null,
    activate: async () => {
      order.push("switch:backend");
      return { revision: 1, value: "A active" };
    },
    commit: (state) => state,
    publish: async () => {
      order.push("switch:publish:start");
      await publishGate.promise;
      order.push("switch:publish:end");
    },
  });
  const openAfterSwitch = queue.enqueue(async () => {
    order.push("open:backend");
    order.push("open:publish");
    return "opened";
  });

  await Promise.resolve();
  assert.deepEqual(order, ["switch:backend"]);
  await Promise.resolve();
  assert.deepEqual(order, ["switch:backend", "switch:publish:start"]);
  publishGate.resolve();

  assert.equal((await switching).value, "A active");
  assert.equal(await openAfterSwitch, "opened");
  assert.deepEqual(order, [
    "switch:backend",
    "switch:publish:start",
    "switch:publish:end",
    "open:backend",
    "open:publish",
  ]);
});

test("late snapshots cannot replace a newer revision in the same session", () => {
  const states = new Map();
  const newer = { revision: 8, value: "new" };
  const stale = { revision: 7, value: "old" };

  assert.equal(commitNewestSessionState(states, "s1", newer), newer);
  assert.equal(commitNewestSessionState(states, "s1", stale), newer);
  assert.equal(states.get("s1"), newer);

  const equalRevision = { revision: 8, value: "authoritative refresh" };
  assert.equal(commitNewestSessionState(states, "s1", equalRevision), equalRevision);
  assert.equal(states.get("s1"), equalRevision);
});

test("tab refresh keeps only an active id that still exists", () => {
  const sessions = [{ sessionId: "s2" }, { sessionId: "s3" }];
  assert.equal(resolveListedActiveId("s2", sessions), "s2");
  assert.equal(resolveListedActiveId("s1", sessions), "s2");
  assert.equal(resolveListedActiveId(null, sessions), "s2");
  assert.equal(resolveListedActiveId("s1", []), null);
});

test("A to B to A invalidates callbacks from the first A view", () => {
  const view = new SessionViewGuard();
  view.activate("A");
  const oldA = view.capture();
  assert.ok(oldA);
  assert.equal(view.isCurrent(oldA), true);

  view.activate("B");
  view.activate("A");
  const newA = view.capture();
  assert.ok(newA);
  assert.equal(view.isCurrent(oldA), false);
  assert.equal(view.isCurrent(newA), true);
});

test("only the latest operation may clear a shared activity flag", () => {
  const operations = new LatestOperationGuard();
  const oldOperation = operations.begin();
  const newOperation = operations.begin();
  assert.equal(operations.isCurrent(oldOperation), false);
  assert.equal(operations.isCurrent(newOperation), true);
  operations.invalidate();
  assert.equal(operations.isCurrent(newOperation), false);
});

test("secret reveal requires a loaded value for the current session and entry", () => {
  assert.equal(canToggleSecretReveal("secret", "s1", "s1", "u1", "u1"), true);
  assert.equal(canToggleSecretReveal("", "s1", "s1", "u1", "u1"), true);
  assert.equal(canToggleSecretReveal(null, "s1", "s1", "u1", "u1"), false);
  assert.equal(canToggleSecretReveal("secret", "s1", "s2", "u1", "u1"), false);
  assert.equal(canToggleSecretReveal("secret", "s1", "s1", "u1", "u2"), false);
});

test("a wholesale session replacement rejects every response from the old epoch", () => {
  const cache = new SessionStateCache();
  const oldEpoch = cache.capture("s1");
  cache.commit("s1", oldEpoch, { revision: 12, value: "old branch" });

  const replacement = { revision: 3, value: "downloaded remote" };
  assert.equal(cache.replace("s1", replacement), replacement);
  assert.equal(
    cache.commit("s1", oldEpoch, { revision: 13, value: "late old response" }),
    replacement,
  );
  assert.equal(cache.get("s1"), replacement);
});
