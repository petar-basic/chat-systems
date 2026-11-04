import i18n, { type InitOptions } from 'i18next';
import { initReactI18next } from 'react-i18next';
import enAuth from '@/locales/en/auth.json';
import enProfile from '@/locales/en/profile.json';
import jaAuth from '@/locales/ja/auth.json';
import jaProfile from '@/locales/ja/profile.json';

export const LANGUAGES = ['en', 'ja'] as const;
export type Language = (typeof LANGUAGES)[number];

export const LANGUAGE_NAMES: Record<Language, string> = {
  en: 'English',
  ja: '日本語',
};

export const resources = {
  en: { auth: enAuth, profile: enProfile },
  ja: { auth: jaAuth, profile: jaProfile },
} as const;

export function isLanguage(value: unknown): value is Language {
  return LANGUAGES.some((language) => language === value);
}

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
  ns: ['auth', 'profile'],
  initAsync: false,
  interpolation: { escapeValue: false },
} satisfies InitOptions;

void i18n.use(initReactI18next).init({ ...i18nOptions, lng: preferredLanguage(browserLanguages()) });

function markDocument(language: string) {
  if (typeof document !== 'undefined') document.documentElement.lang = language;
}

markDocument(i18n.language);
i18n.on('languageChanged', markDocument);

export function applyLanguage(chosen: unknown): Promise<unknown> {
  const language = isLanguage(chosen) ? chosen : preferredLanguage(browserLanguages());
  if (i18n.language === language) return Promise.resolve();
  return i18n.changeLanguage(language);
}

export default i18n;
