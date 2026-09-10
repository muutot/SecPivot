import type { Language } from "$lib/types/settings";
import { en } from "./en";
import { zhCN, type I18nKey } from "./zh-CN";

export type { I18nKey };

const dictionaries: Record<Language, Record<I18nKey, string>> = {
  "zh-CN": zhCN as unknown as Record<I18nKey, string>,
  en,
};

function fill(template: string, vars: Record<string, string | number>): string {
  return template.replace(/\{(\w+)\}/g, (match, name: string) =>
    name in vars ? String(vars[name]) : match,
  );
}

/** Translate `key` for `locale`, interpolating `{var}` placeholders.
 *
 *  Singular convention: when `vars.count === 1` and a `<key>_one` entry
 *  exists, it wins (English plurals; Chinese ignores it). Fallback chain
 *  is locale → zh-CN → the key itself, so a missing translation degrades
 *  to Chinese, never to a blank.
 */
export function t(
  locale: Language,
  key: I18nKey,
  vars: Record<string, string | number> = {},
): string {
  const dict = dictionaries[locale] ?? dictionaries["zh-CN"];
  if (vars.count === 1) {
    const singular = dict[`${key}_one` as I18nKey] ?? zhCN[`${key}_one` as I18nKey];
    if (singular !== undefined) return fill(singular, vars);
  }
  const template = dict[key] ?? zhCN[key] ?? key;
  return fill(template, vars);
}
