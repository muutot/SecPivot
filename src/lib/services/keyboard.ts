import type { IconName } from "$lib/components/AppIcon.svelte";

/** One configurable app-window shortcut action. The panel renders these and
 *  `+page.svelte` dispatches them; both sides must agree on the ids. */
export interface KeyboardAction {
  id: string;
  label: string;
  description: string;
  icon: IconName;
  /** Accelerator assigned when the action has no stored binding. */
  default: string;
}

export const KEYBOARD_ACTIONS: KeyboardAction[] = [
  {
    id: "save",
    label: "保存数据库",
    description: "把当前改动写入数据库文件",
    icon: "save",
    default: "Ctrl+S",
  },
  {
    id: "lock",
    label: "锁定数据库",
    description: "立即锁定当前数据库",
    icon: "lock",
    default: "Ctrl+L",
  },
  {
    id: "edit",
    label: "编辑所选条目",
    description: "打开所选条目的编辑对话框",
    icon: "edit",
    default: "Ctrl+E",
  },
  {
    id: "copy-password",
    label: "复制密码",
    description: "复制所选条目的密码到剪贴板",
    icon: "copy",
    default: "Ctrl+Shift+C",
  },
  {
    id: "new-entry",
    label: "新建条目",
    description: "在当前分组下新建条目",
    icon: "plus",
    default: "Ctrl+N",
  },
  {
    id: "focus-search",
    label: "聚焦搜索",
    description: "把焦点移到条目搜索框",
    icon: "search",
    default: "Ctrl+K",
  },
  {
    id: "locate-in-tree",
    label: "定位到所在分组",
    description: "在左侧分组树中选中并展开所选条目所在的分组",
    icon: "folder",
    default: "Ctrl+G",
  },
];

/** Stored bindings merged with action defaults, so unrecorded actions still
 *  dispatch their default accelerator. */
export function effectiveShortcuts(shortcuts: Record<string, string>): Record<string, string> {
  const out: Record<string, string> = {};
  for (const action of KEYBOARD_ACTIONS) {
    out[action.id] = shortcuts[action.id] || action.default;
  }
  return out;
}

/** True when the event's pressed modifiers match `combo` ("Ctrl+Shift+C").
 *  Modifier order in the combo is irrelevant; the last non-modifier token is
 *  the key. Single-character keys compare case-insensitively ("Space" is the
 *  canonical name for `" "`). */
export function matchesShortcut(event: KeyboardEvent, combo: string): boolean {
  const mods: [string, boolean][] = [
    ["Ctrl", event.ctrlKey],
    ["Alt", event.altKey],
    ["Shift", event.shiftKey],
    ["Meta", event.metaKey],
  ];
  const parts = combo.split("+").map((p) => p.trim());
  let keyPart = "";
  for (const part of parts) {
    if (part === "Ctrl" || part === "Alt" || part === "Shift" || part === "Meta") continue;
    keyPart = part;
  }
  for (const [name, pressed] of mods) {
    if (parts.includes(name) !== pressed) return false;
  }
  if (!keyPart) return false;
  const eventKey =
    event.key === " " ? "Space" : event.key.length === 1 ? event.key.toUpperCase() : event.key;
  return eventKey === keyPart;
}
