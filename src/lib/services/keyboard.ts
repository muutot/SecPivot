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
];
