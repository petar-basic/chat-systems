import { describe, it, expect } from 'vitest';
import { wordRanges } from './fold';

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

  it('finds a word inside a sentence with no spaces, down to one character', () => {
    const text = '明日の定例会議は10時からです';
    const marked = (query: string) => wordRanges(text, query).map(([start, end]) => text.slice(start, end));
    expect(marked('会議')).toEqual(['会議']);
    expect(marked('会')).toEqual(['会']);
    expect(marked('定例会議 10時')).toEqual(['定例会議', '10時']);
  });

  it('meets half-width katakana and full-width letters', () => {
    const text = 'ﾐｰﾃｨﾝｸﾞは１０時、ＡＷＳで';
    const marked = (query: string) => wordRanges(text, query).map(([start, end]) => text.slice(start, end));
    expect(marked('ミーティング')).toEqual(['ﾐｰﾃｨﾝｸﾞ']);
    expect(marked('aws 10')).toEqual(['１０', 'ＡＷＳ']);
  });

  it('finds every word of a file name, in any order', () => {
    expect(wordRanges('Q3 budget.xlsx', 'BUDG q3')).toEqual([
      [0, 2],
      [3, 7],
    ]);
  });
});
