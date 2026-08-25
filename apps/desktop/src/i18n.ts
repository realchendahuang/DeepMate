// i18n setup. English is the source language; zh is bundled. The active
// language is loaded from config at startup and switched at runtime.

import i18n from "i18next";
import { initReactI18next } from "react-i18next";

import en from "./locales/en.json";
import zh from "./locales/zh.json";

export const SUPPORTED_LANGUAGES = ["en", "zh"] as const;
export type Language = (typeof SUPPORTED_LANGUAGES)[number];

export function resolveLanguage(code: string | null | undefined): Language {
  return SUPPORTED_LANGUAGES.includes(code as Language) ? (code as Language) : "en";
}

i18n.use(initReactI18next).init({
  resources: {
    en: { translation: en },
    zh: { translation: zh },
  },
  lng: resolveLanguage(null),
  fallbackLng: "en",
  interpolation: { escapeValue: false },
});

export default i18n;
