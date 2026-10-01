import { useSyncExternalStore } from "react";
import { vi } from "./i18n-vi";

export type Locale = "en" | "vi";
export const locales: { value: Locale; label: string }[] = [
  { value: "en", label: "English" },
  { value: "vi", label: "Tiếng Việt" },
];

const STORAGE_KEY = "knotree_locale";
const listeners = new Set<() => void>();

export function parseLocale(value: string | null | undefined): Locale | null {
  const primary = value?.trim().toLowerCase().split(/[-_]/)[0];
  return primary === "vi" || primary === "en" ? primary : null;
}

/** Saved choice first, then the browser's preferred languages, then English. */
export function detectLocale(): Locale {
  try {
    const saved = parseLocale(window.localStorage.getItem(STORAGE_KEY));
    if (saved) return saved;
  } catch {
    // Storage can be unavailable (private mode); fall through to the browser.
  }
  const preferred = typeof navigator === "undefined" ? [] : [...(navigator.languages ?? []), navigator.language];
  for (const tag of preferred) {
    const locale = parseLocale(tag);
    if (locale) return locale;
  }
  return "en";
}

let current: Locale = typeof window === "undefined" ? "en" : detectLocale();
if (typeof document !== "undefined") document.documentElement.lang = current;

export function getLocale(): Locale {
  return current;
}

export function setLocale(locale: Locale) {
  try {
    window.localStorage.setItem(STORAGE_KEY, locale);
  } catch {
    // Not fatal: the choice still applies for this page and is saved on the server when signed in.
  }
  if (locale === current) return;
  current = locale;
  document.documentElement.lang = locale;
  listeners.forEach((listener) => listener());
}

export function useLocale(): Locale {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
    () => current,
    () => current,
  );
}

/**
 * Translates an English source string into the active locale. `{name}`
 * placeholders are filled from `vars`. Unknown strings fall back to English.
 */
export function t(source: string, vars?: Record<string, string | number>): string {
  const template = current === "vi" ? (vi[source] ?? source) : source;
  if (!vars) return template;
  return template.replace(/\{(\w+)\}/g, (match: string, key: string) => (key in vars ? String(vars[key]) : match));
}

/** BCP 47 tag for Intl formatters. */
export function intlLocale(): string {
  return current === "vi" ? "vi-VN" : "en-US";
}
