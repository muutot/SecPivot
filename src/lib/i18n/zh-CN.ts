/** Simplified-Chinese dictionary: the source of truth for i18n keys.
 *  Keys are flat dotted paths grouped by surface area. English
 *  (`en.ts`) must define the exact same key set (type-checked). */

export const zhCN = {
  "common.ok": "确定",
  "common.cancel": "取消",
  "common.close": "关闭",
  "common.save": "保存",
  "common.delete": "删除",
  "common.retry": "重试",
  "common.confirm": "确认",
  "common.back": "返回",
} as const;

export type I18nKey = keyof typeof zhCN;
