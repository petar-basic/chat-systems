import { describe, expect, it } from 'vitest';
import { EMOTICON_RULE, TRAILING_EMOTICON, emoticonToEmoji } from './emoticons';

const match = (typed: string) => typed.match(EMOTICON_RULE);

describe('emoticons', () => {
  it('maps the curated set to emoji', () => {
    expect(emoticonToEmoji('<3')).toBe('❤️');
    expect(emoticonToEmoji('</3')).toBe('💔');
    expect(emoticonToEmoji(':D')).toBe('😃');
    expect(emoticonToEmoji(':-)')).toBe('🙂');
    expect(emoticonToEmoji(':nope:')).toBeUndefined();
  });

  it('fires once a space follows, at the start of a line and after whitespace', () => {
    expect(match('<3 ')?.[2]).toBe('<3');
    expect(match('love this <3 ')?.[2]).toBe('<3');
    expect(match('ha :D ')?.[2]).toBe(':D');
  });

  it('waits while the emoticon may still grow into a shortcode', () => {
    expect(match(':D')).toBeNull();
    expect(match('ha :D')).toBeNull();
    expect(match(':Dasd')).toBeNull();
    expect(match(':Dasd: ')).toBeNull();
  });

  it('leaves emoticons glued to a word alone', () => {
    expect(match('https://x.com:D ')).toBeNull();
    expect(match('a:D ')).toBeNull();
    expect(match('foo<3 ')).toBeNull();
  });

  it('lets Enter through, which the editor runs input rules on as a newline', () => {
    expect(match('thanks :)\n')).toBeNull();
  });

  it('only fires on the last thing typed', () => {
    expect(match('<3 and more text ')).toBeNull();
  });

  it('prefers the longest emoticon when one contains another', () => {
    expect(match('</3 ')?.[2]).toBe('</3');
    expect(match(':-( ')?.[2]).toBe(':-(');
  });

  it('keeps the whitespace around the match', () => {
    expect(match('ha :D ')?.slice(1)).toEqual([' ', ':D', ' ']);
    expect(match(':D ')?.slice(1)).toEqual(['', ':D', ' ']);
  });

  it('finds the emoticon that ends a message about to be sent', () => {
    expect('thanks :)'.match(TRAILING_EMOTICON)?.[2]).toBe(':)');
    expect(':-('.match(TRAILING_EMOTICON)?.[2]).toBe(':-(');
    expect('thanks :) ok'.match(TRAILING_EMOTICON)).toBeNull();
    expect('a:D'.match(TRAILING_EMOTICON)).toBeNull();
  });
});
