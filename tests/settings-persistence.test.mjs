import assert from "node:assert/strict";
import test from "node:test";

import { LatestPersistQueue } from "../src/lib/utils/latest-persist-queue.ts";

function deferred() {
  let resolve;
  let reject;
  const promise = new Promise((done, fail) => {
    resolve = done;
    reject = fail;
  });
  return { promise, resolve, reject };
}

test("older persistence acknowledgements never roll back newer local edits", async () => {
  const queue = new LatestPersistQueue();
  const writes = [];
  let current = { first: true, second: false, third: false };
  const write = (value) => {
    const gate = deferred();
    writes.push({ value, gate });
    return gate.promise;
  };
  const apply = (saved) => {
    current = saved;
  };

  queue.enqueue({ ...current });
  const draining = queue.drain(write, apply);
  await Promise.resolve();

  current = { ...current, second: true };
  queue.enqueue({ ...current });
  writes[0].gate.resolve(writes[0].value);
  await Promise.resolve();
  await Promise.resolve();
  assert.deepEqual(current, { first: true, second: true, third: false });
  assert.equal(writes.length, 2);

  current = { ...current, third: true };
  queue.enqueue({ ...current });
  writes[1].gate.resolve(writes[1].value);
  await Promise.resolve();
  await Promise.resolve();
  assert.deepEqual(current, { first: true, second: true, third: true });
  assert.equal(writes.length, 3);

  writes[2].gate.resolve(writes[2].value);
  await draining;
  assert.deepEqual(current, { first: true, second: true, third: true });
});

test("a failed older write continues directly to the newer queued value", async () => {
  const queue = new LatestPersistQueue();
  const writes = [];
  let applied = null;
  const write = (value) => {
    const gate = deferred();
    writes.push({ value, gate });
    return gate.promise;
  };

  queue.enqueue({ revision: 1 });
  const draining = queue.drain(write, (saved) => {
    applied = saved;
  });
  await Promise.resolve();
  queue.enqueue({ revision: 2 });

  writes[0].gate.reject(new Error("temporary failure"));
  await Promise.resolve();
  await Promise.resolve();
  assert.equal(writes.length, 2);
  assert.deepEqual(writes[1].value, { revision: 2 });

  writes[1].gate.resolve(writes[1].value);
  await draining;
  assert.deepEqual(applied, { revision: 2 });
  assert.equal(queue.hasPending, false);
});

test("drain waits for an already in-flight write even with no pending value", async () => {
  const queue = new LatestPersistQueue();
  const gate = deferred();
  let settled = false;

  queue.enqueue("settings");
  void queue.drain(
    () => gate.promise,
    () => {},
  );
  await Promise.resolve();
  const flush = queue
    .drain(
      () => Promise.resolve("unused"),
      () => {},
    )
    .then(() => {
      settled = true;
    });

  await Promise.resolve();
  assert.equal(settled, false);
  gate.resolve("settings");
  await flush;
  assert.equal(settled, true);
});
