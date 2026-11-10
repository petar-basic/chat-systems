import { describe, it, expect, afterEach } from 'vitest';
import i18n from '@/lib/i18n';
import { displayNameOf, matchesPerson } from './userHelpers';

const cleo = { display_name: 'Cleo Park', email: 'cleo@dev.local', title: 'Head of Finance' };
const bob = { display_name: null, email: 'bob@dev.local', title: null };

describe('matchesPerson', () => {
  it('finds the people who have a title, whatever its case', () => {
    expect([cleo, bob].filter((p) => matchesPerson(p, ' FINANCE '))).toEqual([cleo]);
  });

  it('still finds a person by name, or by email when they have no name', () => {
    expect(matchesPerson(cleo, 'park')).toBe(true);
    expect(matchesPerson(bob, 'bob@')).toBe(true);
    expect(matchesPerson(cleo, 'cleo@')).toBe(false);
  });

  it('lets everyone through an empty search', () => {
    expect([cleo, bob].filter((p) => matchesPerson(p, '  '))).toEqual([cleo, bob]);
  });
});

describe('displayNameOf in Japanese', () => {
  afterEach(async () => {
    await i18n.changeLanguage('en');
  });

  it('calls someone without a name 不明なユーザー', async () => {
    await i18n.changeLanguage('ja');
    expect(displayNameOf('  ')).toBe('不明なユーザー');
  });
});
