import assert from "node:assert/strict";
import { register } from "node:module";
import test from "node:test";
register("./lib-alias-loader.mjs", new URL("./helpers/", import.meta.url));
const { GROUP_DRAG_MIME, createGroupDrag, planGroupDrop, validateGroupMove, groupDropPlacement } =
  await import("../src/lib/utils/group-drag.ts");
const g = (uuid, parentUuid, children = [], isRecycleBin = false) => ({
  uuid,
  parentUuid,
  name: uuid,
  children,
  entries: [],
  isRecycleBin,
});
const fixture = () =>
  g("root", null, [
    g("a", "root", [g("child", "a")]),
    g("b", "root"),
    g("bin", "root", [g("deleted", "bin")], true),
  ]);
test("group drop plans preserve visual sibling order and reject cycles/bin/invalid anchors", () => {
  const root = fixture();
  assert.deepEqual(planGroupDrop(root, "a", "b", "after"), {
    uuid: "a",
    parentUuid: "root",
    beforeUuid: "bin",
  });
  assert.deepEqual(planGroupDrop(root, "b", "a", "before"), {
    uuid: "b",
    parentUuid: "root",
    beforeUuid: "a",
  });
  assert.deepEqual(planGroupDrop(root, "a", "b", "inside"), {
    uuid: "a",
    parentUuid: "b",
    beforeUuid: null,
  });
  assert.deepEqual(planGroupDrop(root, "child", "root", "inside"), {
    uuid: "child",
    parentUuid: "root",
    beforeUuid: null,
  });
  for (const [source, target] of [
    ["a", "a"],
    ["a", "child"],
    ["a", "bin"],
    ["a", "deleted"],
    ["deleted", "b"],
    ["bin", "b"],
    ["root", "b"],
    ["missing", "b"],
  ]) {
    assert.equal(planGroupDrop(root, source, target, "inside"), null);
  }
  assert.equal(validateGroupMove(root, { uuid: "a", parentUuid: "b", beforeUuid: "child" }), false);
});
function event() {
  const data = new Map();
  return {
    clientY: 50,
    currentTarget: { getBoundingClientRect: () => ({ top: 40, height: 40 }) },
    prevented: false,
    preventDefault() {
      this.prevented = true;
    },
    dataTransfer: {
      types: [],
      setData(type, value) {
        data.set(type, value);
        this.types.push(type);
      },
      getData(type) {
        return data.get(type) ?? "";
      },
    },
  };
}
test("drag owner validates session, lifetime, writable state and actual transfer identity", () => {
  let root = fixture(),
    sessionId = "session-a",
    readOnly = false,
    target;
  const moves = [];
  const drag = createGroupDrag({
    root: () => root,
    sessionId: () => sessionId,
    readOnly: () => readOnly,
    target: (value) => {
      target = value;
    },
    move: (value) => moves.push(value),
  });
  let e = event();
  drag.start("a", e);
  drag.over("b", "inside", e);
  assert.equal(e.dataTransfer.effectAllowed, "move");
  assert.equal(e.dataTransfer.dropEffect, "move");
  assert.deepEqual(target, { uuid: "b", placement: "inside" });
  drag.drop("b", "inside", e);
  assert.equal(moves.length, 1);
  assert.equal(target, null);
  for (const invalidate of [
    () => {
      sessionId = "session-b";
    },
    () => {
      root = fixture();
    },
    () => {
      readOnly = true;
    },
  ]) {
    readOnly = false;
    e = event();
    drag.start("a", e);
    invalidate();
    drag.over("b", "inside", e);
    drag.drop("b", "inside", e);
    assert.equal(target, null);
    assert.equal(moves.length, 1);
  }
  readOnly = false;
  e = event();
  drag.start("a", e);
  e.dataTransfer.setData(GROUP_DRAG_MIME, JSON.stringify({ uuid: "a", sessionId: "foreign" }));
  drag.drop("b", "inside", e);
  assert.equal(moves.length, 1);
  e = event();
  drag.start("a", e);
  e.dataTransfer.setData(GROUP_DRAG_MIME, "{broken");
  drag.drop("b", "inside", e);
  assert.equal(moves.length, 1);
  drag.over("b", "inside", e);
  assert.equal(target, null);
});
test("row edge quarters select insertion and middle selects nesting", () => {
  const e = event();
  for (const [y, placement] of [
    [41, "before"],
    [50, "inside"],
    [70, "inside"],
    [79, "after"],
  ]) {
    e.clientY = y;
    assert.equal(groupDropPlacement(e), placement);
  }
});
