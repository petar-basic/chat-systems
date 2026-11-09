import i18n, { type InitOptions } from 'i18next';
import { initReactI18next } from 'react-i18next';
import { logger } from '@/lib/logger';
import enAuth from '@/locales/en/auth.json';
import enChannel from '@/locales/en/channel.json';
import enCommon from '@/locales/en/common.json';
import enHuddle from '@/locales/en/huddle.json';
import enNotifications from '@/locales/en/notifications.json';
import enProfile from '@/locales/en/profile.json';
import enSidebar from '@/locales/en/sidebar.json';
import enWorkspace from '@/locales/en/workspace.json';
import jaAuth from '@/locales/ja/auth.json';
import jaChannel from '@/locales/ja/channel.json';
import jaCommon from '@/locales/ja/common.json';
import jaHuddle from '@/locales/ja/huddle.json';
import jaNotifications from '@/locales/ja/notifications.json';
import jaProfile from '@/locales/ja/profile.json';
import jaSidebar from '@/locales/ja/sidebar.json';
import jaWorkspace from '@/locales/ja/workspace.json';

const STORAGE_KEY = 'chat_language';

export const LANGUAGES = ['en', 'ja'] as const;
export type Language = (typeof LANGUAGES)[number];

export const LANGUAGE_NAMES: Record<Language, string> = {
  en: 'English',
  ja: '日本語',
};

export const resources = {
  en: {
    auth: enAuth,
    channel: enChannel,
    common: enCommon,
    huddle: enHuddle,
    notifications: enNotifications,
    profile: enProfile,
    sidebar: enSidebar,
    workspace: enWorkspace,
  },
  ja: {
    auth: jaAuth,
    channel: jaChannel,
    common: jaCommon,
    huddle: jaHuddle,
    notifications: jaNotifications,
    profile: jaProfile,
    sidebar: jaSidebar,
    workspace: jaWorkspace,
  },
} as const;

const ESCAPED: Record<string, string> = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
  "'": '&#39;',
  '/': '&#x2F;',
  '{': '&#123;',
  '}': '&#125;',
};
const UNESCAPED = Object.fromEntries(Object.entries(ESCAPED).map(([char, entity]) => [entity, char]));

export function escapeText(value: unknown): string {
  return String(value).replace(/[&<>"'/{}]/g, (char) => ESCAPED[char]);
}

export function unescapeText(text: string): string {
  return text.replace(/&(?:amp|lt|gt|quot|#39|#x2F|#123|#125);/g, (entity) => UNESCAPED[entity]);
}

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

function storedLanguage(): Language | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return isLanguage(raw) ? raw : null;
  } catch {
    return null;
  }
}

function remember(language: Language | null) {
  try {
    if (language) localStorage.setItem(STORAGE_KEY, language);
    else localStorage.removeItem(STORAGE_KEY);
  } catch (err) {
    logger.error('i18n', 'remember', err);
  }
}

export const i18nOptions = {
  resources,
  fallbackLng: 'en',
  supportedLngs: LANGUAGES,
  ns: ['auth', 'channel', 'common', 'huddle', 'notifications', 'profile', 'sidebar', 'workspace'],
  initAsync: false,
  interpolation: { escapeValue: false },
  react: { unescape: unescapeText },
} satisfies InitOptions;

void i18n
  .use(initReactI18next)
  .init({ ...i18nOptions, lng: storedLanguage() ?? preferredLanguage(browserLanguages()) });

function markDocument(language: string) {
  if (typeof document !== 'undefined') document.documentElement.lang = language;
}

markDocument(i18n.language);
i18n.on('languageChanged', markDocument);

export function applyLanguage(chosen: unknown): Promise<unknown> {
  const kept = isLanguage(chosen) ? chosen : null;
  remember(kept);
  const language = kept ?? preferredLanguage(browserLanguages());
  if (i18n.language === language) return Promise.resolve();
  return i18n.changeLanguage(language);
}

export default i18n;
