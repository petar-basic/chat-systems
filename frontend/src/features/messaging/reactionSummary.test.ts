import { describe, expect, it } from 'vitest';
import { joinReactors, reactorNames } from './reactionSummary';

const NAMES: Record<string, string> = { a: 'Marko Markovic', b: 'Nikola Nikolic', c: 'Ana Anic' };
const nameOf = (id: string) => NAMES[id];

describe('reactorNames', () => {
  it('puts the current user first and calls them You', () => {
    expect(reactorNames(['a', 'me', 'b'], 'me', nameOf)).toEqual(['You', 'Marko Markovic', 'Nikola Nikolic']);
  });

  it('keeps reaction order when the current user is not among them', () => {
    expect(reactorNames(['b', 'a'], 'me', nameOf)).toEqual(['Nikola Nikolic', 'Marko Markovic']);
  });

  it('falls back for a reactor the cache has never seen', () => {
    expect(reactorNames(['ghost'], 'me', nameOf)).toEqual(['Someone']);
  });
});

describe('joinReactors', () => {
  it('renders one, two and three names', () => {
    expect(joinReactors(['Marko Markovic'])).toBe('Marko Markovic');
    expect(joinReactors(['Marko Markovic', 'Ana Anic'])).toBe('Marko Markovic and Ana Anic');
    expect(joinReactors(['Marko Markovic', 'Ana Anic', 'Nikola Nikolic'])).toBe(
      'Marko Markovic, Ana Anic and Nikola Nikolic',
    );
  });

  it('counts the overflow past three names', () => {
    expect(joinReactors(['A', 'B', 'C', 'D'])).toBe('A, B, C and 1 other');
    expect(joinReactors(['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H'])).toBe('A, B, C and 5 others');
  });

  it('is empty when nobody reacted', () => {
    expect(joinReactors([])).toBe('');
  });
});
