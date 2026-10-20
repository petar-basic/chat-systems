import { describe, expect, it } from 'vitest';
import { loadShortcodes, shortcodeToEmoji } from './emojiShortcodes';

describe('emoji shortcodes', () => {
  it('resolves ids and aliases from the emoji-mart data set', async () => {
    await loadShortcodes();
    expect(shortcodeToEmoji('smile')).toBe('😄');
    expect(shortcodeToEmoji('+1')).toBe('👍');
    expect(shortcodeToEmoji('thumbsup')).toBe('👍');
    expect(shortcodeToEmoji('SMILE')).toBe('😄');
    expect(shortcodeToEmoji('not_an_emoji_at_all')).toBeUndefined();
  });
});
