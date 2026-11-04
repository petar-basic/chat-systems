import { describe, it, expect, afterEach } from 'vitest';
import { createInstance } from 'i18next';
import i18n, { LANGUAGES, applyLanguage, i18nOptions, preferredLanguage, resources } from './i18n';

function keys(tree: object, prefix = ''): string[] {
  return Object.entries(tree).flatMap(([key, value]) =>
    typeof value === 'object' && value !== null ? keys(value, `${prefix}${key}.`) : [`${prefix}${key}`],
  );
}

afterEach(async () => {
  await i18n.changeLanguage('en');
});

describe('preferredLanguage', () => {
  it('takes the first browser language it has, by its base tag', () => {
    expect(preferredLanguage(['ja-JP', 'en-US'])).toBe('ja');
    expect(preferredLanguage(['fr-FR', 'ja'])).toBe('ja');
    expect(preferredLanguage(['JA'])).toBe('ja');
    expect(preferredLanguage(['en-GB', 'ja'])).toBe('en');
  });

  it('falls back to English', () => {
    expect(preferredLanguage(['fr-FR', 'de'])).toBe('en');
    expect(preferredLanguage([])).toBe('en');
  });
});

describe('applyLanguage', () => {
  it('switches to a language it has', async () => {
    await applyLanguage('ja');
    expect(i18n.language).toBe('ja');
  });

  it('follows the browser for anything else', async () => {
    for (const chosen of [
      null,
      undefined,
      '',
      'browser',
      'xx',
      'ja-JP',
      'JA',
      '<b>ja</b>',
      7,
      ['ja'],
      { ja: true },
    ]) {
      await i18n.changeLanguage('ja');
      await applyLanguage(chosen);
      expect(i18n.language, JSON.stringify(chosen)).toBe('en');
    }
  });
});

describe('i18n', () => {
  it('shows English for a string that has no translation yet', async () => {
    const untranslated = createInstance();
    await untranslated.init({ ...i18nOptions, lng: 'ja', resources: { en: resources.en, ja: { auth: {} } } });
    expect(untranslated.t('backToSignIn', { ns: 'auth' })).toBe(resources.en.auth.backToSignIn);
  });

  it('has no translated key that English lacks', () => {
    const english = new Set(Object.values(resources.en).flatMap((ns) => keys(ns)));
    for (const language of LANGUAGES) {
      const stray = Object.values(resources[language])
        .flatMap((ns) => keys(ns))
        .filter((key) => !english.has(key));
      expect(stray, language).toEqual([]);
    }
  });

  it('keeps the page language in step', async () => {
    await i18n.changeLanguage('ja');
    expect(document.documentElement.lang).toBe('ja');
    await i18n.changeLanguage('en');
    expect(document.documentElement.lang).toBe('en');
  });
});
