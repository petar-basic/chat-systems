import { describe, it, expect } from 'vitest';
import { fragmentRange, wordRanges } from './fold';

describe('fragmentRange', () => {
  it('finds a fragment ignoring case and diacritics, in the original text', () => {
    expect(fragmentRange('Q3 budget.xlsx', 'BUDG')).toEqual([3, 7]);
    expect(fragmentRange('Izveštaj.pdf', 'vesta')).toEqual([2, 7]);
    expect(fragmentRange('Izveštaj.pdf', ' ')).toBeNull();
    expect(fragmentRange('Izveštaj.pdf', 'xlsx')).toBeNull();
  });
});

describe('wordRanges', () => {
  it('finds every word of a query, each time it appears, in the original text', () => {
    expect(wordRanges('Ship the release; the release is Friday', 'release friday')).toEqual([
      [9, 16],
      [22, 29],
      [33, 39],
    ]);
  });

  it('ignores case, diacritics, quotes and one-letter words', () => {
    expect(wordRanges('Izveštaj za Ćirila', '"izvestaj" a CIRILA')).toEqual([
      [0, 8],
      [12, 18],
    ]);
  });

  it('merges matches that overlap', () => {
    expect(wordRanges('deployment', 'deploy deployment')).toEqual([[0, 10]]);
  });
});
