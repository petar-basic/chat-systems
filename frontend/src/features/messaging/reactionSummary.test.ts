import { describe, expect, it } from 'vitest';
import i18n from '@/lib/i18n';
import { joinReactors, reactorNames } from './reactionSummary';

const en = i18n.getFixedT('en', 'messages');
const ja = i18n.getFixedT('ja', 'messages');

const NAMES: Record<string, string> = { a: 'Marko Markovic', b: 'Nikola Nikolic', c: 'Ana Anic' };
const nameOf = (id: string) => NAMES[id];

describe('reactorNames', () => {
  it('puts the current user first and calls them You', () => {
    expect(reactorNames(['a', 'me', 'b'], 'me', nameOf, en)).toEqual([
      'You',
      'Marko Markovic',
      'Nikola Nikolic',
    ]);
  });

  it('keeps reaction order when the current user is not among them', () => {
    expect(reactorNames(['b', 'a'], 'me', nameOf, en)).toEqual(['Nikola Nikolic', 'Marko Markovic']);
  });

  it('falls back for a reactor the cache has never seen', () => {
    expect(reactorNames(['ghost'], 'me', nameOf, en)).toEqual(['Someone']);
  });
});

describe('joinReactors', () => {
  it('renders one, two and three names', () => {
    expect(joinReactors(['Marko Markovic'], en)).toBe('Marko Markovic');
    expect(joinReactors(['Marko Markovic', 'Ana Anic'], en)).toBe('Marko Markovic and Ana Anic');
    expect(joinReactors(['Marko Markovic', 'Ana Anic', 'Nikola Nikolic'], en)).toBe(
      'Marko Markovic, Ana Anic and Nikola Nikolic',
    );
  });

  it('counts the overflow past three names', () => {
    expect(joinReactors(['A', 'B', 'C', 'D'], en)).toBe('A, B, C and 1 other');
    expect(joinReactors(['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'], en)).toBe('A, B, C and 5 others');
  });

  it('is empty when nobody reacted', () => {
    expect(joinReactors([], en)).toBe('');
  });
});

describe('reactors in Japanese', () => {
  it('calls you あなた and someone unknown 誰か', () => {
    expect(reactorNames(['me', 'ghost'], 'me', nameOf, ja)).toEqual(['あなた', '誰か']);
  });

  it('joins names the Japanese way, counting the overflow', () => {
    expect(joinReactors(['A', 'B'], ja)).toBe('A と B');
    expect(joinReactors(['A', 'B', 'C'], ja)).toBe('A、B と C');
    expect(joinReactors(['A', 'B', 'C', 'D', 'E'], ja)).toBe('A、B、C ほか2人');
  });

  it('keeps a hostile name as written', () => {
    const name = '<img src=x onerror="alert(1)"> {{name}}';
    expect(joinReactors([name, 'B'], ja)).toBe(`${name} と B`);
    expect(joinReactors([name, 'B', 'C', 'D'], ja)).toBe(`${name}、B、C ほか1人`);
  });
});
