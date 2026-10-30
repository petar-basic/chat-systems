import { describe, it, expect } from 'vitest';
import {
  matchCustomEmoji,
  pickedEmojiText,
  pickerCategories,
  scopePickerData,
  splitCustomEmoji,
  type PickerData,
} from './customEmoji';
import type { CustomEmoji } from '@/features/workspace/hooks/useCustomEmoji';

const shipit: CustomEmoji = {
  id: '1',
  name: 'shipit',
  url: 'https://files.test/shipit.png',
  created_by: 'u1',
  created_at: '2026-01-01T00:00:00Z',
};

const byName = new Map([[shipit.name, shipit]]);

describe('splitCustomEmoji', () => {
  it('leaves text alone when the workspace has no emoji', () => {
    expect(splitCustomEmoji(':shipit: now', new Map())).toEqual([{ text: ':shipit: now', emoji: null }]);
  });

  it('splits around a shortcode the workspace defines', () => {
    expect(splitCustomEmoji('ship it :shipit: today', byName)).toEqual([
      { text: 'ship it ', emoji: null },
      { text: ':shipit:', emoji: shipit },
      { text: ' today', emoji: null },
    ]);
  });

  it('leaves an unknown shortcode as the text somebody typed', () => {
    expect(splitCustomEmoji('what about :unknown:', byName)).toEqual([
      { text: 'what about :unknown:', emoji: null },
    ]);
  });

  it('handles several in one message', () => {
    const spans = splitCustomEmoji(':shipit::shipit:', byName);
    expect(spans.filter((s) => s.emoji)).toHaveLength(2);
    expect(spans.map((s) => s.text).join('')).toBe(':shipit::shipit:');
  });

  /// A ratio like 1:30:00 is not a shortcode, and neither is a URL scheme.
  it('does not eat colons that are not shortcodes', () => {
    expect(splitCustomEmoji('at 1:30:00 sharp', byName)).toEqual([{ text: 'at 1:30:00 sharp', emoji: null }]);
  });
});

describe('matchCustomEmoji', () => {
  const party: CustomEmoji = { ...shipit, id: '2', name: 'party-parrot' };
  const parrot: CustomEmoji = { ...shipit, id: '3', name: 'parrot' };
  const all = new Map([party, parrot, shipit].map((e) => [e.name, e]));

  it('puts names that start with what was typed before names that only contain it', () => {
    expect(matchCustomEmoji('parrot', all).map((e) => e.name)).toEqual(['parrot', 'party-parrot']);
  });

  it('ignores case and stops at the limit', () => {
    expect(matchCustomEmoji('PAR', all, 1).map((e) => e.name)).toEqual(['party-parrot']);
  });

  it('offers nothing a workspace does not have', () => {
    expect(matchCustomEmoji('test', all)).toEqual([]);
  });
});

describe('the emoji picker', () => {
  it("adds the workspace's emoji as their own category, and none when there are none", () => {
    expect(pickerCategories(byName)).toEqual([
      {
        id: 'workspace',
        name: 'Workspace',
        emojis: [{ id: 'shipit', name: 'shipit', keywords: ['shipit'], skins: [{ src: shipit.url }] }],
      },
    ]);
    expect(pickerCategories(new Map())).toEqual([]);
  });

  it("keeps only the open workspace's emoji in the picker's shared data", () => {
    const elsewhere = { id: 'workspace', name: 'Workspace', emojis: [{ id: 'partyparrot' }] };
    const data: PickerData = {
      emojis: { rocket: {}, partyparrot: {} },
      originalCategories: [{ id: 'frequent', emojis: [] }, { id: 'people', emojis: ['rocket'] }, elsewhere],
    };
    const here = pickerCategories(byName);

    scopePickerData(data, here);

    expect(data.emojis).toEqual({ rocket: {} });
    expect(data.originalCategories).toEqual([
      { id: 'frequent', emojis: [] },
      { id: 'people', emojis: ['rocket'] },
      ...here,
    ]);
  });

  it('leaves the data alone before the picker has loaded it once', () => {
    const fresh: PickerData = { emojis: { partyparrot: {} } };
    scopePickerData(fresh, pickerCategories(byName));
    expect(fresh).toEqual({ emojis: { partyparrot: {} } });
    expect(() => scopePickerData(undefined, [])).not.toThrow();
  });

  it('gives a standard emoji as its character and a custom one as its shortcode', () => {
    expect(pickedEmojiText({ id: 'rocket', native: '🚀' })).toBe('🚀');
    expect(pickedEmojiText({ id: 'shipit' })).toBe(':shipit:');
  });
});
