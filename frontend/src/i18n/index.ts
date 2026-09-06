// SPEC.md FR-4: de / fr / it / en, English as fallback. No string literals in components.
import de from "./de.json";
import en from "./en.json";
import fr from "./fr.json";
import it from "./it.json";

export type Lang = "de" | "fr" | "it" | "en";
const BUNDLES: Record<Lang, Record<string, string>> = { de, fr, it, en };

export function detectLang(): Lang {
  const nav = navigator.language.slice(0, 2).toLowerCase();
  return (["de", "fr", "it", "en"] as const).includes(nav as Lang) ? (nav as Lang) : "en";
}

export function makeT(lang: Lang) {
  const bundle = BUNDLES[lang] ?? BUNDLES.en;
  return (key: string, vars?: Record<string, string | number>): string => {
    const raw = bundle[key] ?? BUNDLES.en[key] ?? key;
    if (!vars) return raw;
    return raw.replace(/\{(\w+)\}/g, (_m, k: string) => String(vars[k] ?? `{${k}}`));
  };
}
export type T = ReturnType<typeof makeT>;
