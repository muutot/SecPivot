import test, { mock } from "node:test";
import assert from "node:assert/strict";
import { attachMenuKeyboard } from "../src/lib/utils/menu-keyboard.ts";
import { readFileSync } from "node:fs";
import { runInNewContext } from "node:vm";
import ts from "typescript";
import { effectiveShortcuts, dispatchShortcut } from "../src/lib/services/keyboard.ts";

// Deterministic DOM seam for focus ownership/listener lifetime. Real Svelte
// mounting, native button activation and Tab traversal are verified in Edge.
class Element {
  constructor(doc, role = "", parent = null, disabled = false) {
    this.ownerDocument = doc;
    this.role = role;
    this.parent = parent;
    this.disabled = disabled;
    this.isConnected = true;
    this.children = [];
    parent?.children.push(this);
  }
  contains(node) {
    return node === this || this.children.some((child) => child.contains(node));
  }
  closest(selector) {
    if (selector === '[role="menu"]') {
      return this.role === "menu" ? this : (this.parent?.closest(selector) ?? null);
    }
    return this.role.startsWith("menuitem") ? this : (this.parent?.closest(selector) ?? null);
  }
  querySelectorAll() {
    return this.children.flatMap((child) => [
      ...(child.role.startsWith("menuitem") ? [child] : []),
      ...child.querySelectorAll(),
    ]);
  }
  focus() {
    this.ownerDocument.activeElement = this;
    this.ownerDocument.emit("focusin", { target: this });
  }
  scrollIntoView() {
    this.scrolled = true;
  }
}

globalThis.HTMLElement = Element;

function fixture() {
  const listeners = new Map();
  const doc = {
    addEventListener(type, callback) {
      if (!listeners.has(type)) listeners.set(type, new Set());
      listeners.get(type).add(callback);
    },
    removeEventListener(type, callback) {
      listeners.get(type)?.delete(callback);
    },
    emit(type, event) {
      for (const callback of [...(listeners.get(type) ?? [])]) callback(event);
    },
  };
  doc.body = new Element(doc);
  const invoker = new Element(doc, "button", doc.body);
  invoker.focus();
  const root = new Element(doc, "menu", doc.body);
  const first = new Element(doc, "menuitemcheckbox", root);
  new Element(doc, "menuitem", root, true);
  const last = new Element(doc, "menuitem", root);
  const nested = new Element(doc, "menu", root);
  const child = new Element(doc, "menuitem", nested);
  let closes = 0;
  const attach = (navigate) => attachMenuKeyboard(root, () => closes++, navigate);
  function key(key, extras = {}) {
    const event = {
      key,
      target: doc.activeElement,
      defaultPrevented: false,
      stopped: false,
      preventDefault() {
        this.defaultPrevented = true;
      },
      stopPropagation() {
        this.stopped = true;
      },
      ...extras,
    };
    doc.emit("keydown", event);
    return event;
  }
  return { doc, root, invoker, first, last, child, attach, key, closes: () => closes };
}

test("navigation stays in its menu level and skips disabled rows", () => {
  const f = fixture();
  const dispose = f.attach();
  try {
    assert.equal(f.doc.activeElement, f.first);
    f.key("ArrowDown");
    assert.equal(f.doc.activeElement, f.last);
    f.key("ArrowDown");
    assert.equal(f.doc.activeElement, f.first);
    f.key("ArrowUp");
    assert.equal(f.doc.activeElement, f.last);
    f.key("Home");
    assert.equal(f.doc.activeElement, f.first);
    f.key("End");
    assert.equal(f.doc.activeElement, f.last);
    f.child.focus();
    f.key("Home");
    assert.equal(f.doc.activeElement, f.child);
    assert.equal(f.child.scrolled, true);
  } finally {
    dispose();
  }
});

test("Escape restores the invoker without recursively closing on focusin", () => {
  const f = fixture();
  const dispose = f.attach();
  const event = f.key("Escape");
  assert.equal(f.closes(), 1);
  assert.equal(f.doc.activeElement, f.invoker);
  assert.equal(event.defaultPrevented, true);
  assert.equal(event.stopped, true);
  f.doc.emit("click", { target: f.invoker });
  assert.equal(f.closes(), 1);
  dispose();
});

test("Tab leaves native traversal enabled and child Escape owns its layer", () => {
  const f = fixture();
  const dispose = f.attach((event) => event.key === "Escape");
  const childEscape = f.key("Escape");
  assert.equal(childEscape.defaultPrevented, true);
  assert.equal(f.closes(), 0);
  const tab = f.key("Tab", { shiftKey: true });
  assert.equal(tab.defaultPrevented, false);
  assert.equal(tab.stopped, true);
  assert.equal(f.doc.activeElement, f.invoker);
  assert.equal(f.closes(), 1);
  dispose();
});

test("outside focus or a nonfocusable outside click defeats cleanup restoration", () => {
  for (const byFocus of [true, false]) {
    const f = fixture();
    const dispose = f.attach();
    const outside = new Element(f.doc, "button", f.doc.body);
    if (byFocus) outside.focus();
    else f.doc.emit("click", { target: outside });
    dispose();
    assert.equal(f.closes(), 1);
    assert.equal(f.doc.activeElement, byFocus ? outside : f.first);
  }
});

test("unmount restores owned focus, removes listeners and skips a disconnected invoker", () => {
  for (const connected of [true, false]) {
    const f = fixture();
    const dispose = f.attach();
    f.invoker.isConnected = connected;
    dispose();
    assert.equal(f.doc.activeElement, connected ? f.invoker : f.first);
    f.key("Escape");
    f.doc.emit("click", { target: f.doc.body });
    assert.equal(f.closes(), 0);
  }
});

test("empty menus still focus a dismissible surface and respect consumed keys", () => {
  const f = fixture();
  f.root.children = [];
  const dispose = f.attach();
  assert.equal(f.doc.activeElement, f.root);
  f.key("Escape", { defaultPrevented: true });
  assert.equal(f.closes(), 0);
  f.key("ArrowDown");
  f.key("Escape");
  assert.equal(f.closes(), 1);
  assert.equal(f.doc.activeElement, f.invoker);
  dispose();
});

test("leaf activation keys keep native defaults but cannot reach page bubble shortcuts", () => {
  const f = fixture();
  const dispose = f.attach();
  for (const key of ["Enter", " "]) {
    const event = f.key(key);
    assert.equal(event.defaultPrevented, false);
    assert.equal(event.stopped, true);
  }
  dispose();
});

test("page capture shortcuts respect menus owned by recursive group components", () => {
  const source = readFileSync(new URL("../src/routes/+page.svelte", import.meta.url), "utf8");
  const script = source.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast = ts.createSourceFile("page.ts", script, ts.ScriptTarget.Latest, true);
  const handler = ast.statements.find(
    (node) => ts.isFunctionDeclaration(node) && node.name?.text === "handleShortcutKeydown",
  );
  assert.ok(handler);
  const js = ts.transpileModule(handler.getText(ast), {
    compilerOptions: { target: ts.ScriptTarget.ES2022 },
  }).outputText;
  const context = {
    isTcatoOverlay: false,
    currentVault: {},
    $activeContextMenu: "group-fixture",
    busy: false,
    groupIconDialogUuid: null,
    isAnyModalOpen: () => false,
    editor: { editorOpen: false },
    ...Object.fromEntries(
      [
        "groupModalOpen",
        "groupAutoTypeUuid",
        "groupMetaUuid",
        "reportOpen",
        "confirmState",
        "entryMenu",
        "blankMenu",
        "toolbarMenu",
        "columnMenu",
        "advancedSearchOpen",
        "autotypePick",
        "remoteConflict",
        "emergencyExportOpen",
        "dbMetaOpen",
        "dbSettingsOpen",
        "similarOpen",
        "expiredOpen",
        "timelineOpen",
        "hibpOpen",
        "faviconDialog",
      ].map((key) => [key, null]),
    ),
    effectiveShortcuts,
    dispatchShortcut,
    appSettings: { keyboard: { shortcuts: {} } },
    get: (value) => value,
    settings: { general: { focusSearchOnType: false } },
    saves: 0,
    handleSave: () => context.saves++,
    locateSelectedInTree: () => {},
  };
  const event = {
    key: "s",
    ctrlKey: true,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    target: null,
    preventDefault() {},
  };
  runInNewContext(js, context);
  context.handleShortcutKeydown(event);
  assert.equal(context.saves, 0, "a group menu must block global Save before document capture");
  context.$activeContextMenu = null;
  context.handleShortcutKeydown(event);
  assert.equal(context.saves, 1, "Save must work again after menu dismissal");
});

test("consumed menu keys refresh the actual idle deadline and cleanup removes the activity listener", () => {
  const source = readFileSync(new URL("../src/lib/services/security.ts", import.meta.url), "utf8");
  const ast = ts.createSourceFile("security.ts", source, ts.ScriptTarget.Latest, true);
  const names = ["clearIdleTimer", "armIdleLock", "onActivity", "installAutoLock"];
  const functions = names.map((name) => {
    const node = ast.statements.find(
      (node) => ts.isFunctionDeclaration(node) && node.name?.text === name,
    );
    assert.ok(node);
    return node.getText(ast).replace(/^export /, "");
  });
  const js = ts.transpileModule(
    "let idleTimer = null; let idleDeadline = 0;\n" + functions.join("\n"),
    { compilerOptions: { target: ts.ScriptTarget.ES2022 } },
  ).outputText;
  const listeners = [];
  const capture = (options) => options === true || options?.capture === true;
  mock.timers.enable({ apis: ["setTimeout", "Date"], now: 1000 });
  const context = {
    Date,
    setTimeout,
    clearTimeout,
    window: {
      addEventListener(type, callback, options) {
        listeners.push({ type, callback, capture: capture(options) });
      },
      removeEventListener(type, callback, options) {
        const index = listeners.findIndex(
          (item) =>
            item.type === type && item.callback === callback && item.capture === capture(options),
        );
        if (index >= 0) listeners.splice(index, 1);
      },
    },
    appSettings: {
      security: { autoLockMinutes: 1 },
      subscribe(callback) {
        callback(this);
        return () => {};
      },
    },
    get: (value) => value,
    vault: { get: () => ({}) },
    locks: 0,
    lockVault: () => context.locks++,
  };
  const f = fixture();
  const disposeMenu = f.attach();
  let disposeActivity;
  try {
    runInNewContext(js, context);
    disposeActivity = context.installAutoLock();
    mock.timers.tick(45_000);
    // Window capture precedes document capture; stopped events never bubble.
    for (const item of listeners.filter((item) => item.type === "keydown" && item.capture))
      item.callback();
    const event = f.key("ArrowDown");
    if (!event.stopped)
      for (const item of listeners.filter((item) => item.type === "keydown" && !item.capture))
        item.callback();
    mock.timers.tick(15_001);
    assert.equal(context.locks, 0, "menu activity must defeat the old idle deadline");
    mock.timers.tick(45_000);
    assert.equal(context.locks, 1, "inactivity still locks at the refreshed deadline");
    disposeActivity();
    assert.equal(listeners.length, 0);
  } finally {
    disposeActivity?.();
    disposeMenu();
    mock.timers.reset();
  }
});
