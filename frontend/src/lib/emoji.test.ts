import { describe, it, expect } from 'vitest';
import { splitEmojiText, countEmojiOnly, isJumboEmoji } from './emoji';
import type { CustomEmoji } from '@/hooks/queries/useCustomEmoji';

const shipit: CustomEmoji = {
  id: '1',
  name: 'shipit',
  url: 'https://files.test/shipit.png',
  created_by: 'u1',
  created_at: '2026-01-01T00:00:00Z',
};

const byName = new Map([[shipit.name, shipit]]);
const noEmoji = new Map<string, CustomEmoji>();

describe('splitEmojiText', () => {
  it('leaves plain text in one span', () => {
    expect(splitEmojiText('ship it today')).toEqual([{ text: 'ship it today', isEmoji: false }]);
  });

  it('splits an emoji out of a sentence', () => {
    expect(splitEmojiText('nice 🎉 work')).toEqual([
      { text: 'nice ', isEmoji: false },
      { text: '🎉', isEmoji: true },
      { text: ' work', isEmoji: false },
    ]);
  });

  it('keeps a zwj sequence and a skin tone as one emoji', () => {
    expect(splitEmojiText('👩🏽‍💻')).toEqual([{ text: '👩🏽‍💻', isEmoji: true }]);
  });

  it('merges adjacent emoji into one span', () => {
    expect(splitEmojiText('🎉🎉')).toEqual([{ text: '🎉🎉', isEmoji: true }]);
  });

  it('leaves text-presentation pictographs alone', () => {
    expect(splitEmojiText('© 2026')).toEqual([{ text: '© 2026', isEmoji: false }]);
  });
});

describe('countEmojiOnly', () => {
  it('counts an emoji-only message', () => {
    expect(countEmojiOnly('🎉🎉 🔥', noEmoji)).toBe(3);
  });

  it('is zero when the message has words in it', () => {
    expect(countEmojiOnly('nice 🎉', noEmoji)).toBe(0);
  });

  it('is zero for an empty message', () => {
    expect(countEmojiOnly('   ', noEmoji)).toBe(0);
  });

  it('counts custom emoji the workspace defines', () => {
    expect(countEmojiOnly(':shipit: 🚀', byName)).toBe(2);
  });

  it('is zero when a shortcode is not a known emoji', () => {
    expect(countEmojiOnly(':unknown:', byName)).toBe(0);
  });
});

describe('isJumboEmoji', () => {
  it('holds for a handful of emoji', () => {
    expect(isJumboEmoji('🎉🔥', noEmoji)).toBe(true);
  });

  it('stops past the limit, where a wall of emoji is the message', () => {
    expect(isJumboEmoji('🎉'.repeat(24), noEmoji)).toBe(false);
  });

  it('does not hold for text', () => {
    expect(isJumboEmoji('hello', noEmoji)).toBe(false);
  });
});
