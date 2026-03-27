import { Converter } from "opencc-js";

// Lazy-init converters (each takes ~5ms to build, reuse across calls)
let s2tw: ((text: string) => string) | null = null;
let t2s: ((text: string) => string) | null = null;

function getS2TW() {
  if (!s2tw) s2tw = Converter({ from: "cn", to: "twp" }); // Simplified → Traditional Taiwan
  return s2tw;
}

function getT2S() {
  if (!t2s) t2s = Converter({ from: "tw", to: "cn" }); // Traditional → Simplified
  return t2s;
}

/**
 * Convert text based on language code:
 * - "zh-TW": Simplified → Traditional (standard)
 * - "zh-CN": Traditional → Simplified (in case Whisper returned Traditional)
 * - others / null: no conversion
 */
export function convertChineseScript(
  text: string,
  language: string | null
): string {
  if (!language) return text;
  if (language === "zh-TW") return getS2TW()(text);
  if (language === "zh-CN") return getT2S()(text);
  return text;
}
