import assert from "node:assert/strict";
import { readFileSync, mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { execFileSync, spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { resolve, join } from "node:path";
import { register, createRequire } from "node:module";
import { runInNewContext } from "node:vm";
import { after, test } from "node:test";

register("./helpers/lib-alias-loader.mjs", import.meta.url);
globalThis.window = globalThis;
let nativeConfig = {};
globalThis.__TAURI_INTERNALS__ = {
  invoke: async (command, args) => {
    if (command === "get_config") return structuredClone(nativeConfig);
    if (command === "set_config") return (nativeConfig = structuredClone(args.config));
    if (command === "remote_secret_present") return { accessKey: false, secretKey: false };
    throw new Error("Unexpected synthetic command " + command);
  },
};
const { appSettings, normalizeSettings, DEFAULT_GENERAL_SETTINGS } =
  await import("../src/lib/services/settings.ts");
after(() => appSettings.destroy());
const require = createRequire(import.meta.url);
const ts = require("typescript");
const pagePath = "src/routes/+page.svelte";
const page = process.env.SECPIVOT_ESCAPE_COMPARE_REF
  ? execFileSync("git", ["show", `${process.env.SECPIVOT_ESCAPE_COMPARE_REF}:${pagePath}`], {
      encoding: "utf8",
    })
  : readFileSync(pagePath, "utf8");
const script = page.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
const ast = ts.createSourceFile("page.ts", script, ts.ScriptTarget.Latest, true);
const names = [
  "handleLock",
  "isCommandSurfaceOpen",
  "handleEscapeAction",
  "handleEscapeKeydown",
  "handleShortcutKeydown",
];
const bodies = ast.statements.filter(
  (node) => ts.isFunctionDeclaration(node) && names.includes(node.name?.text),
);
const js = ts.transpileModule(bodies.map((node) => node.getText(ast)).join("\n"), {
  compilerOptions: { target: ts.ScriptTarget.ES2022 },
}).outputText;
const surfaces = [
  "groupModalOpen",
  "groupIconDialogUuid",
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
  "largeOpen",
  "timelineOpen",
  "hibpOpen",
  "faviconDialog",
];
function fixture(action = "lock") {
  const calls = [],
    flashes = [],
    errors = [];
  const context = {
    settings: { general: { escapeAction: action, language: "en", focusSearchOnType: false } },
    currentVault: { dirty: false },
    isTcatoOverlay: false,
    $activeContextMenu: null,
    $fileReloadApplying: false,
    busy: false,
    escapeActionPending: false,
    groupIconSaving: false,
    editor: { editorOpen: false },
    ...Object.fromEntries(surfaces.map((key) => [key, null])),
    modal: false,
    inline: false,
    closing: false,
    desktop: true,
    mobile: false,
    confirm: true,
    isAnyModalOpen: () => context.modal,
    isFileReloadBlocked: () => context.inline,
    isVaultClosePending: () => context.closing,
    isTauriRuntime: () => context.desktop,
    isMobile: () => context.mobile,
    lockVault: async () => {
      calls.push("lock");
      await context.wait;
      if (context.failure) throw new Error("Synthetic cancellation");
    },
    invoke: async (command, args) => {
      calls.push([command, args.action]);
      await context.wait;
      if (context.failure) throw new Error("Synthetic failure");
    },
    flash: (message, kind) => flashes.push([message, kind]),
    reportCloseError: (error) => errors.push(error.message),
    t: (_lang, key) => key,
    window: { confirm: () => context.confirm },
  };
  runInNewContext(js, context);
  const fire = (extra = {}) => {
    const event = {
      key: "Escape",
      defaultPrevented: false,
      repeat: false,
      isComposing: false,
      ctrlKey: false,
      shiftKey: false,
      altKey: false,
      metaKey: false,
      target: null,
      preventDefault() {
        this.defaultPrevented = true;
      },
      ...extra,
    };
    context.handleShortcutKeydown(event);
    context.handleEscapeKeydown?.(event);
    return event;
  };
  const settled = async () => {
    await new Promise((resolve) => setImmediate(resolve));
  };
  return { context, calls, flashes, errors, fire, settled };
}
test("unhandled plain Escape locks via the actual manual lock flow; None consumes without acting", async () => {
  const f = fixture();
  assert.equal(
    f.fire({ target: { tagName: "INPUT" } }).defaultPrevented,
    true,
    "main quick search permits Escape locking",
  );
  await f.settled();
  assert.deepEqual(f.calls, ["lock"]);
  f.context.settings.general.escapeAction = "none";
  f.fire();
  await f.settled();
  assert.deepEqual(f.calls, ["lock"]);
});
test("all desktop actions dispatch explicit native requests even without an open database", async () => {
  for (const action of ["minimize", "minimizeToTray", "exit"]) {
    const f = fixture(action);
    f.context.currentVault = null;
    f.fire();
    await f.settled();
    assert.deepEqual(f.calls, [["perform_window_action", action]]);
    f.fire({ target: { tagName: "INPUT" } });
    await f.settled();
    assert.equal(f.calls.length, 1, "standalone credential draft retains Escape ownership");
  }
});
test("menus, modal and page surfaces, native close, reload, edits and busy operations retain Escape", async () => {
  for (const key of [
    "modal",
    "inline",
    "closing",
    "busy",
    "$fileReloadApplying",
    "isTcatoOverlay",
    "$activeContextMenu",
    ...surfaces,
  ]) {
    const f = fixture();
    f.context[key] = key.endsWith("Uuid") || key === "$activeContextMenu" ? "synthetic" : true;
    if (key === "groupIconDialogUuid") f.context.groupIconSaving = true;
    f.fire();
    await f.settled();
    assert.deepEqual(f.calls, [], key);
  }
  const f = fixture();
  f.context.editor.editorOpen = true;
  f.fire();
  await f.settled();
  assert.deepEqual(f.calls, []);
});
test("consumed control Escape, key repeat, IME and modifiers never start a workspace action", async () => {
  for (const extra of [
    { defaultPrevented: true },
    { repeat: true },
    { isComposing: true },
    { ctrlKey: true },
    { altKey: true },
    { shiftKey: true },
    { metaKey: true },
  ]) {
    const f = fixture();
    f.fire(extra);
    await f.settled();
    assert.deepEqual(f.calls, [], JSON.stringify(extra));
  }
  const f = fixture();
  f.context.groupIconDialogUuid = "synthetic";
  assert.equal(f.fire().defaultPrevented, true);
  assert.equal(f.context.groupIconDialogUuid, null);
  await f.settled();
  assert.deepEqual(f.calls, [], "icon dismissal cannot cascade into lock");
});
test("one action remains pending across repeated presses and failures permit a later retry", async () => {
  const f = fixture("minimize");
  let finish;
  f.context.wait = new Promise((resolve) => {
    finish = resolve;
  });
  f.fire();
  f.fire();
  assert.equal(f.calls.length, 1);
  f.context.failure = true;
  finish();
  await f.settled();
  assert.deepEqual(f.flashes, [["page.windowActionFailed", "error"]]);
  f.context.failure = false;
  f.fire();
  await f.settled();
  assert.equal(f.calls.length, 2);
});
test("lock cancellation and browser confirmation preserve state; window actions report unavailable", async () => {
  const f = fixture();
  f.context.desktop = false;
  f.context.currentVault.dirty = true;
  f.context.confirm = false;
  f.fire();
  await f.settled();
  assert.deepEqual(f.calls, []);
  f.context.confirm = true;
  f.context.failure = true;
  f.fire();
  await f.settled();
  assert.deepEqual(f.calls, ["lock"]);
  assert.equal(f.errors.length, 1);
  for (const mobile of [true, false]) {
    const w = fixture("exit");
    w.context.mobile = mobile;
    w.context.desktop = mobile;
    w.fire();
    await w.settled();
    assert.deepEqual(w.calls, []);
    assert.deepEqual(w.flashes, [["page.windowActionDesktop", "error"]]);
  }
});
test("strict five-value settings preserve real publication/reload and default to official Lock", async () => {
  assert.equal(DEFAULT_GENERAL_SETTINGS.escapeAction, "lock");
  for (const value of [undefined, null, true, 1, {}, "unknown", "Lock"]) {
    assert.equal(
      normalizeSettings({ general: { escapeAction: value } }).general.escapeAction,
      "lock",
    );
  }
  for (const value of ["none", "lock", "minimize", "minimizeToTray", "exit"]) {
    appSettings.updateGeneral("escapeAction", value);
    await appSettings.flush();
    await appSettings.initialize();
    assert.equal(nativeConfig.general.escapeAction, value);
    let actual;
    const unsubscribe = appSettings.subscribe((s) => (actual = s.general.escapeAction));
    unsubscribe();
    assert.equal(actual, value);
  }
});

test("actual group rename Escape discards the draft before blur; Enter and ordinary blur commit once", () => {
  const source = readFileSync("src/lib/components/GroupNode.svelte", "utf8");
  const script = source.match(/<script lang="ts">([\s\S]*?)<\/script>/)[1];
  const ast = ts.createSourceFile("group.ts", script, ts.ScriptTarget.Latest, true);
  const commit = ast.statements.find(
    (node) => ts.isFunctionDeclaration(node) && node.name?.text === "commitRename",
  );
  const callback = source.match(/onkeydown=\{([\s\S]*?)\}\s+onblur=\{commitRename\}/)[1];
  const code = ts.transpileModule(commit.getText(ast) + "\nconst keydown = " + callback + ";", {
    compilerOptions: { target: ts.ScriptTarget.ES2022 },
  }).outputText;
  for (const mode of ["Escape", "Enter", "blur"]) {
    const calls = [],
      context = {
        renaming: true,
        nameInput: "  Synthetic new name  ",
        group: { uuid: "synthetic", name: "Original" },
        onrename: (...args) => calls.push(args),
      };
    runInNewContext(code + "\nglobalThis.keydown = keydown;", context);
    const event = {
      key: mode,
      prevented: false,
      preventDefault() {
        this.prevented = true;
      },
    };
    if (mode !== "blur") context.keydown(event);
    context.commitRename();
    context.commitRename();
    assert.deepEqual(calls, mode === "Escape" ? [] : [["synthetic", "Synthetic new name"]]);
    assert.equal(context.renaming, false);
    if (mode === "Escape") assert.equal(event.prevented, true);
  }
});

test("actual native command and tray helper honor target, hide/lock failures and explicit exit", () => {
  const source = readFileSync("src-tauri/src/commands/window.rs", "utf8");
  const command = source.match(/pub\(crate\) fn perform_window_action[\s\S]*?\n\}/)[0];
  const lib = readFileSync("src-tauri/src/lib.rs", "utf8");
  const helper = lib.match(/pub\(crate\) fn minimize_main_to_tray[\s\S]*?\n\}/)[0];
  const root = mkdtempSync(join(tmpdir(), "secpivot-window-actions-"));
  const rust = `
use std::{rc::Rc, cell::RefCell};
const TRAY_ID: &str = "main"; const TRAY_LOCK_EVENT: &str = "tray-lock";
#[derive(Default)] struct Data { events: Vec<String>, tray: bool, window: bool, lock: bool, config_error: bool, hide_error: bool, emit_error: bool, minimize_error: bool }
#[derive(Clone)] struct ConfigStore(Rc<RefCell<Data>>);
struct Config {security: Security} struct Security {lock_on_minimize_to_tray: bool}
impl ConfigStore {fn get(&self)->Result<Config,String>{let d=self.0.borrow();if d.config_error {Err("config".into())}else{Ok(Config{security:Security{lock_on_minimize_to_tray:d.lock}})}}}
mod tauri {
 use super::*;
 #[derive(Clone)] pub(crate) struct AppHandle(pub Rc<RefCell<Data>>);
 #[derive(Clone)] pub(crate) struct WebviewWindow {pub app: AppHandle, pub label: String}
 impl AppHandle {
  pub fn tray_by_id(&self,id:&str)->Option<()>{assert_eq!(id,"main");self.0.borrow().tray.then_some(())}
  pub fn get_webview_window(&self,id:&str)->Option<WebviewWindow>{assert_eq!(id,"main");self.0.borrow().window.then(||WebviewWindow{app:self.clone(),label:"main".into()})}
  pub fn state<T>(&self)->ConfigStore{let _=std::marker::PhantomData::<T>;ConfigStore(self.0.clone())}
  pub fn emit(&self,event:&str,_:())->Result<(),String>{self.0.borrow_mut().events.push(event.into());if self.0.borrow().emit_error{Err("emit".into())}else{Ok(())}}
  pub fn exit(&self,code:i32){self.0.borrow_mut().events.push(format!("exit:{code}"));}
 }
 impl WebviewWindow {
  pub fn label(&self)->&str{&self.label} pub fn app_handle(&self)->&AppHandle{&self.app}
  pub fn minimize(&self)->Result<(),String>{self.app.0.borrow_mut().events.push("minimize".into());if self.app.0.borrow().minimize_error{Err("minimize".into())}else{Ok(())}}
  pub fn hide(&self)->Result<(),String>{self.app.0.borrow_mut().events.push("hide".into());if self.app.0.borrow().hide_error{Err("hide".into())}else{Ok(())}}
  pub fn show(&self)->Result<(),String>{self.app.0.borrow_mut().events.push("show".into());Ok(())}
  pub fn unminimize(&self)->Result<(),String>{self.app.0.borrow_mut().events.push("unminimize".into());Ok(())}
  pub fn set_focus(&self)->Result<(),String>{self.app.0.borrow_mut().events.push("focus".into());Ok(())}
 }
}
${source.match(/pub\(crate\) enum WindowAction[\s\S]*?\n\}/)[0]}
${command}
${helper}
fn fixture()->(tauri::WebviewWindow,Rc<RefCell<Data>>){let data=Rc::new(RefCell::new(Data{tray:true,window:true,..Default::default()}));(tauri::WebviewWindow{app:tauri::AppHandle(data.clone()),label:"main".into()},data)}
#[test] fn explicit_exit_only_requests_exit(){let(w,d)=fixture();perform_window_action(w,WindowAction::Exit).unwrap();assert_eq!(d.borrow().events,["exit:0"]);}
#[test] fn minimize_returns_failures(){let(w,d)=fixture();perform_window_action(w.clone(),WindowAction::Minimize).unwrap();d.borrow_mut().minimize_error=true;assert!(perform_window_action(w,WindowAction::Minimize).is_err());assert_eq!(d.borrow().events,["minimize","minimize"]);}
#[test] fn tray_reads_live_lock_preference(){let(w,d)=fixture();perform_window_action(w.clone(),WindowAction::MinimizeToTray).unwrap();d.borrow_mut().lock=true;perform_window_action(w,WindowAction::MinimizeToTray).unwrap();assert_eq!(d.borrow().events,["hide","hide","tray-lock"]);}
#[test] fn missing_target_tray_and_config_refuse_hide(){for field in 0..3{let(w,d)=fixture();match field{0=>d.borrow_mut().tray=false,1=>d.borrow_mut().window=false,_=>d.borrow_mut().config_error=true}assert!(perform_window_action(w,WindowAction::MinimizeToTray).is_err());assert!(d.borrow().events.is_empty());}}
#[test] fn failed_hide_does_not_emit_and_failed_emit_restores_window(){let(w,d)=fixture();d.borrow_mut().lock=true;d.borrow_mut().hide_error=true;assert!(perform_window_action(w.clone(),WindowAction::MinimizeToTray).is_err());assert_eq!(d.borrow().events,["hide"]);d.borrow_mut().hide_error=false;d.borrow_mut().emit_error=true;d.borrow_mut().events.clear();assert!(perform_window_action(w,WindowAction::MinimizeToTray).is_err());assert_eq!(d.borrow().events,["hide","tray-lock","show","unminimize","focus"]);}
#[test] fn foreign_windows_cannot_dispatch(){for action in [WindowAction::Minimize,WindowAction::MinimizeToTray,WindowAction::Exit]{let(mut w,d)=fixture();w.label="tcato".into();assert!(perform_window_action(w,action).is_err());assert!(d.borrow().events.is_empty());}}
`;
  try {
    const path = join(root, "fixture.rs"),
      executable = join(root, process.platform === "win32" ? "fixture.exe" : "fixture");
    writeFileSync(path, rust);
    const build = spawnSync("rustc", ["--edition=2021", "--test", path, "-o", executable], {
      encoding: "utf8",
    });
    assert.equal(build.status, 0, build.stderr);
    const run = spawnSync(executable, [], { encoding: "utf8" });
    assert.equal(run.status, 0, run.stdout + run.stderr);
    assert.match(run.stdout, /6 passed; 0 failed/);
  } finally {
    const absolute = resolve(root),
      temp = resolve(tmpdir());
    assert.ok(absolute.startsWith(temp + (process.platform === "win32" ? "\\" : "/")));
    rmSync(absolute, { recursive: true, force: true });
  }
});
