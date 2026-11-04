import { describe, it, expect, afterEach, vi } from 'vitest';
import { createInstance } from 'i18next';
import i18n, { LANGUAGES, applyLanguage, i18nOptions, preferredLanguage, resources } from './i18n';

function keys(tree: object, prefix = ''): string[] {
  return Object.entries(tree).flatMap(([key, value]) =>
    typeof value === 'object' && value !== null ? keys(value, `${prefix}${key}.`) : [`${prefix}${key}`],
  );
}

const KEPT = 'chat_language';

const HOSTILE = [
  '',
  'browser',
  'xx',
  'ja-JP',
  'JA',
  ' ja',
  '"ja"',
  '<b>ja</b>',
  "ja'; DROP TABLE users; --",
  'j'.repeat(10_000),
];

afterEach(async () => {
  vi.restoreAllMocks();
  localStorage.clear();
  await i18n.changeLanguage('en');
});

async function startFresh() {
  vi.resetModules();
  return (await import('./i18n')).default;
}

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
      localStorage.setItem(KEPT, 'ja');
      await applyLanguage(chosen);
      expect(i18n.language, JSON.stringify(chosen)).toBe('en');
      expect(localStorage.getItem(KEPT), JSON.stringify(chosen)).toBeNull();
    }
  });

  it('keeps the language it switched to on this device', async () => {
    await applyLanguage('ja');
    expect(localStorage.getItem(KEPT)).toBe('ja');
  });

  it('still switches when this device keeps nothing', async () => {
    vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
      throw new Error('quota');
    });
    vi.spyOn(console, 'error').mockImplementation(() => {});
    await applyLanguage('ja');
    expect(i18n.language).toBe('ja');
  });
});

describe('starting up', () => {
  it('starts in the language this device kept', async () => {
    localStorage.setItem(KEPT, 'ja');
    expect((await startFresh()).language).toBe('ja');
  });

  it('follows the browser when this device kept nothing', async () => {
    expect((await startFresh()).language).toBe('en');
  });

  it('follows the browser for a kept value it does not know', async () => {
    for (const kept of HOSTILE) {
      localStorage.setItem(KEPT, kept);
      expect((await startFresh()).language, kept.slice(0, 20)).toBe('en');
    }
  });

  it('follows the browser when this device cannot be read', async () => {
    vi.spyOn(Storage.prototype, 'getItem').mockImplementation(() => {
      throw new Error('blocked');
    });
    expect((await startFresh()).language).toBe('en');
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
