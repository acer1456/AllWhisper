import { writable, derived } from "svelte/store";
import zhTW from "./zh-TW";
import zhCN from "./zh-CN";
import en from "./en";
import ko from "./ko";
import vi from "./vi";
import ja from "./ja";

export type AppLang = "zh-TW" | "zh-CN" | "en" | "ko" | "vi" | "ja";

// ── Translation dictionaries ──────────────────────────────────────────────────

const dict = {
  "zh-TW": zhTW,
  "zh-CN": zhCN,
  en,
  ko,
  vi,
  ja,
} as const;

export type Translations = typeof dict["zh-TW"];

// ── Stores ────────────────────────────────────────────────────────────────────

export const lang = writable<AppLang>("en");

export const t = derived(lang, ($lang) => dict[$lang] as Translations);
