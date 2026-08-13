import assert from "node:assert/strict";
import test from "node:test";

import { resolveImportGroupPath } from "../src/lib/utils/import-groups.ts";

test("every async group creation keeps the import's captured session", async () => {
  const owners = [];
  let activeSession = "A";
  const resolver = {
    state: { children: new Map() },
    baseUuid: "root",
    groups: new Map(),
  };

  const resolved = await resolveImportGroupPath({
    path: "Parent / Child",
    sessionId: "A",
    resolver,
    createGroup: async (sessionId, parentUuid, name) => {
      owners.push(sessionId);
      const uuid = `${parentUuid}:${name}`;
      resolver.state.children.set(`${parentUuid}/${name}`, uuid);
      activeSession = "B";
      await Promise.resolve();
      return resolver.state;
    },
    findCreatedUuid: (state, parentUuid, name) =>
      state.children.get(`${parentUuid}/${name}`) ?? null,
  });

  assert.equal(activeSession, "B");
  assert.deepEqual(owners, ["A", "A"]);
  assert.equal(resolved, "root:Parent:Child");
});

test("existing path segments are reused without duplicate creates", async () => {
  const resolver = {
    state: {},
    baseUuid: "root",
    groups: new Map([
      ["root", new Map([["Parent", "parent"]])],
      ["parent", new Map([["Child", "child"]])],
    ]),
  };
  let creates = 0;

  const resolved = await resolveImportGroupPath({
    path: "Parent / Child",
    sessionId: "A",
    resolver,
    createGroup: async () => {
      creates += 1;
      return resolver.state;
    },
    findCreatedUuid: () => null,
  });

  assert.equal(resolved, "child");
  assert.equal(creates, 0);
});
