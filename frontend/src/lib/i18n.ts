import i18n, { type InitOptions } from 'i18next';
import { initReactI18next } from 'react-i18next';
import enAuth from '@/locales/en/auth.json';
import jaAuth from '@/locales/ja/auth.json';

export const LANGUAGES = ['en', 'ja'] as const;
export type Language = (typeof LANGUAGES)[number];

export const resources = {
  en: { auth: enAuth },
  ja: { auth: jaAuth },
} as const;

export function preferredLanguage(tags: readonly string[]): Language {
  for (const tag of tags) {
    const base = tag.toLowerCase().split('-')[0];
    const match = LANGUAGES.find((language) => language === base);
    if (match) return match;
  }
  return 'en';
}

function browserLanguages(): readonly string[] {
  if (typeof navigator === 'undefined') return [];
  return navigator.languages?.length ? navigator.languages : [navigator.language];
}

export const i18nOptions = {
  resources,
  fallbackLng: 'en',
  supportedLngs: LANGUAGES,
  ns: ['auth'],
  initAsync: false,
  interpolation: { escapeValue: false },
} satisfies InitOptions;

void i18n.use(initReactI18next).init({ ...i18nOptions, lng: preferredLanguage(browserLanguages()) });

function markDocument(language: string) {
  if (typeof document !== 'undefined') document.documentElement.lang = language;
}

markDocument(i18n.language);
i18n.on('languageChanged', markDocument);

export default i18n;
