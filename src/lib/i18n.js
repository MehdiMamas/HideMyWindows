import { writable, derived, get } from "svelte/store";
import en from "./locales/en.json";
import fr from "./locales/fr.json";
import it from "./locales/it.json";
import ro from "./locales/ro.json";
import pl from "./locales/pl.json";

export const LOCALES = {
  en: { name: "English", data: en },
  fr: { name: "Français", data: fr },
  it: { name: "Italiano", data: it },
  ro: { name: "Română", data: ro },
  pl: { name: "Polski", data: pl },
};

export const locale = writable("en");

/** Resolve a dotted key against an object. */
function lookup(obj, key) {
  return key.split(".").reduce((o, k) => (o == null ? undefined : o[k]), obj);
}

/**
 * Translate `key`, falling back to English and finally the key itself.
 * Supports {placeholder} substitution from `vars`.
 */
export const t = derived(locale, ($locale) => {
  const data = (LOCALES[$locale] || LOCALES.en).data;
  return (key, vars) => {
    let str = lookup(data, key);
    if (str == null) str = lookup(en, key);
    if (str == null) return key;
    if (vars) {
      for (const [k, v] of Object.entries(vars)) {
        str = str.replaceAll(`{${k}}`, v);
      }
    }
    return str;
  };
});

/** Non-reactive translate for use outside components. */
export function tr(key, vars) {
  return get(t)(key, vars);
}

/** Pick the best starting locale from a saved value or the browser. */
export function detectLocale(saved) {
  if (saved && LOCALES[saved]) return saved;
  const nav = (navigator.language || "en").slice(0, 2).toLowerCase();
  return LOCALES[nav] ? nav : "en";
}
