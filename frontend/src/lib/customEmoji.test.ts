import { describe, it, expect } from 'vitest';
import { matchCustomEmoji, splitCustomEmoji } from './customEmoji';
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
