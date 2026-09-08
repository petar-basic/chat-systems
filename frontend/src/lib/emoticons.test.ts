import { describe, expect, it } from 'vitest';
import { EMOTICON_RULE, emoticonToEmoji } from './emoticons';

const match = (typed: string) => typed.match(EMOTICON_RULE);

describe('emoticons', () => {
  it('maps the curated set to emoji', () => {
    expect(emoticonToEmoji('<3')).toBe('❤️');
    expect(emoticonToEmoji('</3')).toBe('💔');
    expect(emoticonToEmoji(':D')).toBe('😃');
    expect(emoticonToEmoji(':-)')).toBe('🙂');
    expect(emoticonToEmoji(':nope:')).toBeUndefined();
  });

  it('fires at the start of a line and after whitespace', () => {
    expect(match('<3')?.[2]).toBe('<3');
    expect(match('love this <3')?.[2]).toBe('<3');
    expect(match('ha :D')?.[2]).toBe(':D');
  });

  it('leaves emoticons glued to a word alone', () => {
    expect(match('https://x.com:D')).toBeNull();
    expect(match('a:D')).toBeNull();
    expect(match('foo<3')).toBeNull();
  });

  it('only fires on the last thing typed', () => {
    expect(match('<3 and more text')).toBeNull();
  });

  it('prefers the longest emoticon when one contains another', () => {
    expect(match('</3')?.[2]).toBe('</3');
    expect(match(':-(')?.[2]).toBe(':-(');
  });

  it('keeps the whitespace that anchored the match', () => {
    expect(match('ha :D')?.[1]).toBe(' ');
    expect(match(':D')?.[1]).toBe('');
  });
});
